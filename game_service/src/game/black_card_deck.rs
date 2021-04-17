use rand::seq::SliceRandom;
use rand::thread_rng;
use shared::proto::{
    black_card_in_round::Card, BlackCardInRound, CustomBlackCard, DefaultBlackCard,
};
use tonic::Status;

pub struct BlackCardDeck {
    draw_pile: Vec<BlackCardInRound>,
    discard_pile: Vec<BlackCardInRound>,
}

impl BlackCardDeck {
    pub fn new(
        custom_cards: Vec<CustomBlackCard>,
        default_cards: Vec<DefaultBlackCard>,
    ) -> Result<BlackCardDeck, Status> {
        if custom_cards.is_empty() && default_cards.is_empty() {
            return Err(Status::invalid_argument(
                "Cardpacks must contain at least one black card.",
            ));
        }

        let mut draw_pile = Vec::new();

        for custom_card in custom_cards {
            draw_pile.push(BlackCardInRound {
                card: Some(Card::CustomBlackCard(custom_card)),
            });
        }

        for default_card in default_cards {
            draw_pile.push(BlackCardInRound {
                card: Some(Card::DefaultBlackCard(default_card)),
            });
        }

        let mut deck = BlackCardDeck {
            draw_pile,
            discard_pile: Vec::new(),
        };
        deck.shuffle_and_reset();
        Ok(deck)
    }

    pub fn get_current_black_card(&self) -> &BlackCardInRound {
        // Unwrap is safe here because the constructor guarantees that there is at least
        // one card in the deck, and all mutating methods guarantee that there's always
        // a card in the draw pile.
        &self.draw_pile.last().unwrap()
    }

    pub fn next_card(&mut self) {
        self.discard_pile.push(self.draw_pile.pop().unwrap());
        if self.draw_pile.is_empty() {
            self.shuffle_and_reset();
        }
    }

    pub fn shuffle_and_reset(&mut self) {
        self.draw_pile.append(&mut self.discard_pile);
        self.draw_pile.shuffle(&mut thread_rng());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn create_custom_black_cards(amount: usize) -> Vec<CustomBlackCard> {
        let mut cards = Vec::new();
        for i in 0..amount {
            cards.push(CustomBlackCard {
                name: format!("custom_card_{}", i),
                text: format!("custom_card_{}", i),
                answer_fields: 1,
                create_time: None,
                update_time: None,
                delete_time: None,
            });
        }
        cards
    }

    fn create_default_black_cards(amount: usize) -> Vec<DefaultBlackCard> {
        let mut cards = Vec::new();
        for i in 0..amount {
            cards.push(DefaultBlackCard {
                name: format!("default_card_{}", i),
                text: format!("default_card_{}", i),
                answer_fields: 1,
            });
        }
        cards
    }

    #[test]
    fn create_deck_with_no_cards() {
        let deck_or = BlackCardDeck::new(Vec::new(), Vec::new());
        assert_eq!(deck_or.is_err(), true);
    }

    #[test]
    fn create_deck_with_one_custom_card() {
        let deck_or = BlackCardDeck::new(create_custom_black_cards(1), Vec::new());
        assert_eq!(deck_or.is_ok(), true);
        let mut deck = deck_or.unwrap();
        // TODO - The three top-level match statements in this test are identical. Let's make this more DRY.
        match &deck.get_current_black_card().card {
            Some(c) => {
                match c {
                    Card::CustomBlackCard(custom_card) => {
                        assert_eq!(custom_card.text, "custom_card_0");
                    }
                    _ => panic!(),
                };
            }
            None => panic!(),
        };
        deck.next_card();
        match &deck.get_current_black_card().card {
            Some(c) => {
                match c {
                    Card::CustomBlackCard(custom_card) => {
                        assert_eq!(custom_card.text, "custom_card_0");
                    }
                    _ => panic!(),
                };
            }
            None => panic!(),
        };
        deck.next_card();
        match &deck.get_current_black_card().card {
            Some(c) => {
                match c {
                    Card::CustomBlackCard(custom_card) => {
                        assert_eq!(custom_card.text, "custom_card_0");
                    }
                    _ => panic!(),
                };
            }
            None => panic!(),
        };
    }

    #[test]
    fn create_deck_with_one_default_card() {
        let deck_or = BlackCardDeck::new(Vec::new(), create_default_black_cards(1));
        assert_eq!(deck_or.is_ok(), true);
        let mut deck = deck_or.unwrap();
        // TODO - The three top-level match statements in this test are identical. Let's make this more DRY.
        match &deck.get_current_black_card().card {
            Some(c) => {
                match c {
                    Card::DefaultBlackCard(default_card) => {
                        assert_eq!(default_card.text, "default_card_0");
                    }
                    _ => panic!(),
                };
            }
            None => panic!(),
        };
        deck.next_card();
        match &deck.get_current_black_card().card {
            Some(c) => {
                match c {
                    Card::DefaultBlackCard(default_card) => {
                        assert_eq!(default_card.text, "default_card_0");
                    }
                    _ => panic!(),
                };
            }
            None => panic!(),
        };
        deck.next_card();
        match &deck.get_current_black_card().card {
            Some(c) => {
                match c {
                    Card::DefaultBlackCard(default_card) => {
                        assert_eq!(default_card.text, "default_card_0");
                    }
                    _ => panic!(),
                };
            }
            None => panic!(),
        };
    }

    #[test]
    fn create_deck_with_many_cards() {
        let deck_or = BlackCardDeck::new(
            create_custom_black_cards(10),
            create_default_black_cards(10),
        );
        assert_eq!(deck_or.is_ok(), true);
        let mut deck = deck_or.unwrap();
        let mut custom_card_names_seen: HashSet<String> = HashSet::new();
        let mut default_card_names_seen: HashSet<String> = HashSet::new();
        for _ in 0..20 {
            match &deck.get_current_black_card().card {
                Some(c) => {
                    match c {
                        Card::CustomBlackCard(custom_black_card) => {
                            custom_card_names_seen.insert(String::from(&custom_black_card.text));
                        }
                        Card::DefaultBlackCard(default_black_card) => {
                            default_card_names_seen.insert(String::from(&default_black_card.text));
                        }
                    };
                }
                None => {}
            };
            deck.next_card();
        }
        assert_eq!(custom_card_names_seen.len(), 10);
        assert_eq!(default_card_names_seen.len(), 10);
        for _ in 0..20 {
            match &deck.get_current_black_card().card {
                Some(c) => {
                    match c {
                        Card::CustomBlackCard(custom_black_card) => {
                            custom_card_names_seen.insert(String::from(&custom_black_card.text));
                        }
                        Card::DefaultBlackCard(default_black_card) => {
                            default_card_names_seen.insert(String::from(&default_black_card.text));
                        }
                    };
                }
                None => {}
            };
            deck.next_card();
        }
        assert_eq!(custom_card_names_seen.len(), 10);
        assert_eq!(default_card_names_seen.len(), 10);
    }
}
