use rand::seq::SliceRandom;
use rand::thread_rng;
use shared::proto::{
    game_config::{blank_white_card_config::BlankWhiteCardsAdded, BlankWhiteCardConfig},
    playable_white_card::Card,
    BlankWhiteCard, CustomWhiteCard, DefaultWhiteCard, PlayableWhiteCard,
};
use uuid::Uuid;

pub struct WhiteCardDeck {
    draw_pile: Vec<PlayableWhiteCard>,
    discard_pile: Vec<PlayableWhiteCard>,
}

impl WhiteCardDeck {
    pub fn new(
        custom_white_cards: Vec<CustomWhiteCard>,
        default_white_cards: Vec<DefaultWhiteCard>,
        blank_white_card_config: &BlankWhiteCardConfig,
    ) -> WhiteCardDeck {
        // Add custom white cards.
        let mut cards =
            WhiteCardDeck::convert_custom_white_cards_to_playable_white_cards(custom_white_cards);
        // Add blank white cards.
        cards.append(&mut WhiteCardDeck::create_blank_white_cards(
            WhiteCardDeck::get_blank_white_card_count_to_add(cards.len(), blank_white_card_config),
        ));
        // Add default white cards.
        cards.append(
            &mut WhiteCardDeck::convert_default_white_cards_to_playable_white_cards(
                default_white_cards,
            ),
        );

        let mut deck = WhiteCardDeck {
            draw_pile: cards,
            discard_pile: Vec::new(),
        };

        deck.shuffle_and_reset(); // Performs initial shuffle.
        deck
    }

    fn create_blank_white_cards(size: usize) -> Vec<PlayableWhiteCard> {
        let mut cards = Vec::new();
        for _ in 0..size {
            cards.push(PlayableWhiteCard {
                card: Some(Card::BlankWhiteCard(BlankWhiteCard {
                    id: WhiteCardDeck::generate_blank_white_card_id(),
                    open_text: String::from(""),
                })),
            });
        }
        cards
    }

    fn generate_blank_white_card_id() -> String {
        Uuid::new_v4().to_simple().to_string()
    }

    fn get_blank_white_card_count_to_add(
        non_blank_white_card_count: usize,
        blank_white_card_config: &BlankWhiteCardConfig,
    ) -> usize {
        match &blank_white_card_config.blank_white_cards_added {
            Some(config) => match config {
                BlankWhiteCardsAdded::CardCount(card_count) => {
                    if card_count > &0 {
                        *card_count as usize
                    } else {
                        0
                    }
                }
                BlankWhiteCardsAdded::Percentage(percentage) => {
                    if percentage > &0.0 {
                        ((non_blank_white_card_count as f64 * percentage) / (1.0 - percentage))
                            .floor() as usize
                    } else {
                        0
                    }
                }
            },
            None => 0,
        }
    }

    // A PlayableWhiteCard is a wrapper proto that can either contain a CustomWhiteCard proto, a DefaultWhiteCard proto, or a blank white card.
    // This function takes a list of CustomWhiteCard protos and wraps a PlayableWhiteCard proto around each one.
    fn convert_custom_white_cards_to_playable_white_cards(
        cards: Vec<CustomWhiteCard>,
    ) -> Vec<PlayableWhiteCard> {
        cards
            .into_iter()
            .map(|card| PlayableWhiteCard {
                card: Some(Card::CustomWhiteCard(card)),
            })
            .collect()
    }

    // A PlayableWhiteCard is a wrapper proto that can either contain a WhiteCard proto, a DefaultWhiteCard proto, or a blank white card.
    // This function takes a list of DefaultWhiteCard protos and wraps a PlayableWhiteCard proto around each one.
    fn convert_default_white_cards_to_playable_white_cards(
        cards: Vec<DefaultWhiteCard>,
    ) -> Vec<PlayableWhiteCard> {
        cards
            .into_iter()
            .map(|card| PlayableWhiteCard {
                card: Some(Card::DefaultWhiteCard(card)),
            })
            .collect()
    }

    fn sanitize_card(card: &mut PlayableWhiteCard) {
        match &mut card.card {
            Some(c) => {
                match c {
                    Card::BlankWhiteCard(blank_card) => blank_card.open_text.clear(),
                    _ => {}
                };
            }
            None => {}
        };
    }

    fn draw_one(&mut self) -> Option<PlayableWhiteCard> {
        if self.draw_pile.is_empty() {
            self.shuffle_and_reset();
        }
        self.draw_pile.pop()
    }

    // Returns the exact amount of cards specified, or None if there are not enough.
    pub fn draw_many(&mut self, amount: usize) -> Option<Vec<PlayableWhiteCard>> {
        if self.draw_pile.len() + self.discard_pile.len() < amount {
            return None;
        }
        let mut vec: Vec<PlayableWhiteCard> = Vec::with_capacity(amount);
        for _ in 0..amount {
            // Unwrap is safe here because
            // we already made sure that
            // there are enough total cards
            // to draw in the draw and
            // discard piles.
            vec.push(self.draw_one().unwrap());
        }
        Some(vec)
    }

    pub fn discard_many(&mut self, cards: &mut Vec<PlayableWhiteCard>) {
        for card in cards.iter_mut() {
            WhiteCardDeck::sanitize_card(card);
        }
        self.discard_pile.append(cards);
    }

    pub fn shuffle_and_reset(&mut self) {
        self.draw_pile.append(&mut self.discard_pile);
        self.draw_pile.shuffle(&mut thread_rng());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::proto::game_config::blank_white_card_config::Behavior;

    #[test]
    fn get_blank_white_card_count_to_add() {
        let mut blank_white_card_config = BlankWhiteCardConfig {
            behavior: Behavior::Unspecified.into(),
            blank_white_cards_added: None,
        };

        assert_eq!(
            WhiteCardDeck::get_blank_white_card_count_to_add(100, &blank_white_card_config),
            0
        );

        blank_white_card_config.blank_white_cards_added = Some(BlankWhiteCardsAdded::CardCount(10));
        assert_eq!(
            WhiteCardDeck::get_blank_white_card_count_to_add(100, &blank_white_card_config),
            10
        );
        blank_white_card_config.blank_white_cards_added = Some(BlankWhiteCardsAdded::CardCount(20));
        assert_eq!(
            WhiteCardDeck::get_blank_white_card_count_to_add(500, &blank_white_card_config),
            20
        );

        blank_white_card_config.blank_white_cards_added =
            Some(BlankWhiteCardsAdded::Percentage(0.2));
        assert_eq!(
            WhiteCardDeck::get_blank_white_card_count_to_add(100, &blank_white_card_config),
            25
        );
        blank_white_card_config.blank_white_cards_added =
            Some(BlankWhiteCardsAdded::Percentage(0.5));
        assert_eq!(
            WhiteCardDeck::get_blank_white_card_count_to_add(100, &blank_white_card_config),
            100
        );
        blank_white_card_config.blank_white_cards_added =
            Some(BlankWhiteCardsAdded::Percentage(0.8));
        assert_eq!(
            WhiteCardDeck::get_blank_white_card_count_to_add(100, &blank_white_card_config),
            400
        );
        blank_white_card_config.blank_white_cards_added =
            Some(BlankWhiteCardsAdded::Percentage(0.5));
        assert_eq!(
            WhiteCardDeck::get_blank_white_card_count_to_add(250, &blank_white_card_config),
            250
        );
        blank_white_card_config.blank_white_cards_added =
            Some(BlankWhiteCardsAdded::Percentage(0.555));
        assert_eq!(
            WhiteCardDeck::get_blank_white_card_count_to_add(10000, &blank_white_card_config),
            12471
        );
    }
}
