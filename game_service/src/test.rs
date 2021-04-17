#[cfg(test)]
pub mod helper {
    use super::super::service::constants::*;
    use cards_proto::{
        game_config::blank_white_card_config::Behavior,
        game_config::{BlankWhiteCardConfig, EndCondition},
        CustomBlackCard, CustomWhiteCard, DefaultBlackCard, DefaultWhiteCard, GameConfig,
    };

    pub fn generate_test_custom_black_cards(count: usize) -> Vec<CustomBlackCard> {
        let mut cards = Vec::new();
        for i in 0..count {
            let card = CustomBlackCard {
                name: format!("users/???/cardpacks/???/blackCards/{}", i),
                text: format!("custom_card_{}", i),
                answer_fields: (i % MAX_BLACK_CARD_ANSWER_FIELDS) as i32 + 1,
                create_time: None,
                update_time: None,
                delete_time: None,
            };
            cards.push(card);
        }
        return cards;
    }

    pub fn generate_test_custom_white_cards(count: usize) -> Vec<CustomWhiteCard> {
        let mut cards = Vec::new();
        for i in 0..count {
            let card = CustomWhiteCard {
                name: format!("users/???/cardpacks/???/whiteCards/{}", i),
                text: format!("custom_card_{}", i),
                create_time: None,
                update_time: None,
                delete_time: None,
            };
            cards.push(card);
        }
        return cards;
    }

    pub fn generate_test_default_black_cards(count: usize) -> Vec<DefaultBlackCard> {
        let mut cards = Vec::new();
        for i in 0..count {
            let card = DefaultBlackCard {
                name: format!("defaultCardpacks/???/defaultBlackCards/{}", i),
                text: format!("default_card_{}", i),
                answer_fields: (i % MAX_BLACK_CARD_ANSWER_FIELDS) as i32 + 1,
            };
            cards.push(card);
        }
        return cards;
    }

    pub fn generate_test_default_white_cards(count: usize) -> Vec<DefaultWhiteCard> {
        let mut cards = Vec::new();
        for i in 0..count {
            let card = DefaultWhiteCard {
                name: format!("defaultCardpacks/???/defaultWhiteCards/{}", i),
                text: format!("default_card_{}", i),
            };
            cards.push(card);
        }
        return cards;
    }

    pub fn get_valid_test_game_config() -> GameConfig {
        get_valid_endless_test_game_config()
    }

    pub fn get_valid_endless_test_game_config() -> GameConfig {
        GameConfig {
            display_name: String::from("Test Game"),
            max_players: MINIMUM_PLAYERS_REQUIRED_TO_PLAY as i32,
            end_condition: Some(EndCondition::EndlessMode(())),
            hand_size: MIN_HAND_SIZE_LIMIT,
            custom_cardpack_names: vec![String::from("test_custom_cardpack_name")],
            default_cardpack_names: vec![String::from("test_default_cardpack_name")],
            blank_white_card_config: Some(BlankWhiteCardConfig {
                behavior: Behavior::Disabled.into(),
                blank_white_cards_added: None,
            }),
        }
    }
}
