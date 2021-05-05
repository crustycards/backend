use shared::proto::crusty_cards_api::{
    black_card_in_round::Card, playable_white_card::Card as PlayableCard, BlackCardInRound,
    PlayableWhiteCard,
};

#[derive(PartialEq)]
enum PlayableCardType {
    Custom,
    Blank,
    Default,
}

#[derive(PartialEq)]
struct PlayableWhiteCardIdentifier<'a> {
    card_type: PlayableCardType,
    card_id: &'a str,
}

fn get_playable_white_card_identifier(
    card: &PlayableWhiteCard,
) -> Option<PlayableWhiteCardIdentifier> {
    match &card.card {
        Some(c) => match c {
            PlayableCard::CustomWhiteCard(custom_white_card) => Some(PlayableWhiteCardIdentifier {
                card_type: PlayableCardType::Custom,
                card_id: &custom_white_card.name,
            }),
            PlayableCard::BlankWhiteCard(blank_white_card) => Some(PlayableWhiteCardIdentifier {
                card_type: PlayableCardType::Blank,
                card_id: &blank_white_card.id,
            }),
            PlayableCard::DefaultWhiteCard(default_white_card) => {
                Some(PlayableWhiteCardIdentifier {
                    card_type: PlayableCardType::Default,
                    card_id: &default_white_card.name,
                })
            }
        },
        None => None,
    }
}

pub fn playable_white_cards_have_same_identifier(
    card_one: &PlayableWhiteCard,
    card_two: &PlayableWhiteCard,
) -> bool {
    let card_one_identifier = match get_playable_white_card_identifier(card_one) {
        Some(id) => id,
        None => return false,
    };

    let card_two_identifier = match get_playable_white_card_identifier(card_two) {
        Some(id) => id,
        None => return false,
    };

    card_one_identifier == card_two_identifier
}

pub fn playable_white_card_is_in_list(
    card: &PlayableWhiteCard,
    cards: &[PlayableWhiteCard],
) -> bool {
    for c in cards {
        if playable_white_cards_have_same_identifier(card, c) {
            return true;
        }
    }
    false
}

pub fn get_answer_fields_from_black_card_in_round(card: &BlackCardInRound) -> usize {
    match &card.card {
        Some(c) => match c {
            Card::CustomBlackCard(custom_black_card) => custom_black_card.answer_fields as usize,
            Card::DefaultBlackCard(default_black_card) => default_black_card.answer_fields as usize,
        },
        None => 0,
    }
}
