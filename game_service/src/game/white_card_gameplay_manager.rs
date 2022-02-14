use super::super::helper::{
    get_answer_fields_from_black_card_in_round, playable_white_card_is_in_list,
    playable_white_cards_have_same_identifier,
};
use super::player_id::PlayerId;
use super::white_card_deck::WhiteCardDeck;
use shared::proto::crusty_cards_api::{
    game_config::blank_white_card_config::Behavior, playable_white_card::Card, BlackCardInRound,
    PlayableWhiteCard,
};
use shared::proto_validation::ValidatedGameConfig;
use std::collections::HashMap;
use tonic::Status;

// TODO - Test this module thoroughly.

pub struct WhiteCardGameplayManager {
    // Guaranteed to contain a value for all players.
    hands_and_played_cards: HashMap<PlayerId, Vec<PlayableWhiteCard>>,
    // Not guaranteed to contain values for all players.
    played_cards: HashMap<PlayerId, Vec<PlayableWhiteCard>>,
    white_card_deck: WhiteCardDeck,
    hand_size: usize,
}

impl WhiteCardGameplayManager {
    pub fn new(white_card_deck: WhiteCardDeck, hand_size: usize) -> WhiteCardGameplayManager {
        WhiteCardGameplayManager {
            hands_and_played_cards: HashMap::new(),
            played_cards: HashMap::new(),
            white_card_deck,
            hand_size,
        }
    }

    pub fn add_player(&mut self, player_id: PlayerId) {
        self.hands_and_played_cards.insert(player_id, Vec::new());
    }

    pub fn remove_player(&mut self, player_id: &PlayerId) {
        if let Some(mut hand) = self.hands_and_played_cards.remove(player_id) {
            self.white_card_deck.discard_many(&mut hand);
        }
    }

    pub fn return_played_cards_to_hands(&mut self) {
        self.played_cards.clear();
    }

    pub fn discard_played_cards_and_draw_to_full(&mut self) {
        self.discard_played_cards();
        self.draw_hands_to_full();
    }

    pub fn discard_player_hands(&mut self) {
        for (_, hand) in self.hands_and_played_cards.iter_mut() {
            self.white_card_deck.discard_many(hand);
        }
    }

    pub fn get_hand_belonging_to_player(
        &self,
        player_id: &PlayerId,
    ) -> Option<Vec<&PlayableWhiteCard>> {
        let hand_and_played_cards = match self.hands_and_played_cards.get(player_id) {
            Some(cards) => cards,
            None => return None,
        };
        let played_cards_or = self.played_cards.get(player_id);
        let mut hand = Vec::new();
        match played_cards_or {
            Some(played_cards) => {
                for card in hand_and_played_cards {
                    if !playable_white_card_is_in_list(card, played_cards) {
                        hand.push(card);
                    }
                }
            }
            None => {
                for card in hand_and_played_cards {
                    hand.push(card);
                }
            }
        };
        Some(hand)
    }

    pub fn play_for_artificial_players(&mut self, current_black_card: &BlackCardInRound) {
        // TODO - Handle what to do if artificial player's hand contains blank white cards.
        let answer_fields = get_answer_fields_from_black_card_in_round(current_black_card);
        for (player_id, hand) in self.hands_and_played_cards.iter() {
            if let PlayerId::ArtificialPlayer(_id) = player_id {
                if self.played_cards.get(player_id).is_some() {
                    continue;
                }
                if hand.len() >= answer_fields {
                    let mut played_cards = Vec::new();
                    for card in &hand[0..answer_fields] {
                        played_cards.push(card.clone());
                    }
                    self.played_cards.insert(player_id.clone(), played_cards);
                }
            }
        }
    }

    pub fn player_has_played_this_round(&self, player_id: &PlayerId) -> bool {
        self.played_cards.get(player_id).is_some()
    }

    pub fn play_cards_for_player(
        &mut self,
        player_id: PlayerId,
        cards: &[PlayableWhiteCard],
        current_black_card: &BlackCardInRound,
        config: &ValidatedGameConfig,
    ) -> Result<(), Status> {
        let answer_fields = get_answer_fields_from_black_card_in_round(current_black_card);
        if cards.len() != answer_fields {
            return Err(Status::invalid_argument(format!(
                "Must play exactly {} cards",
                answer_fields
            )));
        }

        // We need to exchange the cards from the function header for
        // references to cards from the user's hand, since the cards
        // in the function header are only guaranteed to contain
        // the card identifier, and not any actual text.
        let mut played_cards: Vec<PlayableWhiteCard> = Vec::new();

        for card in cards {
            match &card.card {
                Some(card) => match card {
                    Card::CustomWhiteCard(custom_card) => {
                        if custom_card.name.is_empty() {
                            return Err(Status::invalid_argument(
                                "Custom white cards require a value for the `name` property.",
                            ));
                        }
                    }
                    Card::BlankWhiteCard(blank_card) => {
                        let blank_white_card_behavior =
                            Behavior::from_i32(config.get_blank_white_card_config().behavior)
                                .unwrap_or(Behavior::Unspecified);

                        if blank_white_card_behavior != Behavior::OpenText {
                            return Err(Status::invalid_argument(
                                "This game does not support open-text blank white cards.",
                            ));
                        }

                        if blank_card.id.is_empty() || blank_card.open_text.trim().is_empty() {
                            return Err(Status::invalid_argument("Blank white cards require values for both `id` and `open_text` properties."));
                        }
                    }
                    Card::DefaultWhiteCard(default_card) => {
                        if default_card.name.is_empty() {
                            return Err(Status::invalid_argument(
                                "Default white cards require a value for the `name` property.",
                            ));
                        }
                    }
                },
                None => {}
            };

            match self.get_card_from_player_hand(&player_id, card) {
                Some(hand_card) => played_cards.push(hand_card.clone()),
                None => {
                    return Err(Status::invalid_argument(
                        "One or more cards is not in the user's hand.",
                    ))
                }
            };
        }

        self.played_cards.insert(player_id, played_cards);
        Ok(())
    }

    pub fn unplay_cards_for_player(&mut self, player_id: &PlayerId) {
        self.played_cards.remove(player_id);
    }

    pub fn get_played_cards(&self) -> &HashMap<PlayerId, Vec<PlayableWhiteCard>> {
        &self.played_cards
    }

    fn discard_played_cards(&mut self) {
        for (player_id, played_cards) in &self.played_cards {
            if let Some(hand) = self.hands_and_played_cards.get_mut(player_id) {
                hand.retain(|card| !playable_white_card_is_in_list(card, played_cards));
            }
        }
        for (_, mut cards) in self.played_cards.drain() {
            self.white_card_deck.discard_many(&mut cards);
        }
    }

    fn draw_hands_to_full(&mut self) {
        for hand in self.hands_and_played_cards.values_mut() {
            let amount_needed_to_draw: usize = std::cmp::max(self.hand_size - hand.len(), 0);
            if amount_needed_to_draw > 0 {
                hand.append(
                    &mut self
                        .white_card_deck
                        .draw_many(amount_needed_to_draw)
                        .unwrap(),
                );
            }
        }
    }

    fn get_card_from_player_hand(
        &self,
        player_id: &PlayerId,
        card: &PlayableWhiteCard,
    ) -> Option<&PlayableWhiteCard> {
        match self.get_hand_belonging_to_player(player_id) {
            Some(hand) => hand
                .into_iter()
                .find(|hand_card| playable_white_cards_have_same_identifier(card, *hand_card)),
            None => None,
        }
    }
}
