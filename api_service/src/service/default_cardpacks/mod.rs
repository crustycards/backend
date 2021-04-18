mod hardcoded_data;
use sha2::{Digest, Sha256};
use shared::proto::{DefaultBlackCard, DefaultCardpack, DefaultWhiteCard};
use shared::resource_name::DefaultCardpackName;
use std::collections::HashMap;
use std::sync::Arc;

pub struct DefaultCardpackHandler {
    pack_list: Vec<Arc<DefaultCardpackData>>,
    packs_by_name: HashMap<String, Arc<DefaultCardpackData>>,
}

impl DefaultCardpackHandler {
    pub fn new_with_custom_packs(packs: Vec<DefaultCardpackData>) -> DefaultCardpackHandler {
        let pack_list: Vec<Arc<DefaultCardpackData>> =
            packs.into_iter().map(|pack| Arc::from(pack)).collect();
        let mut packs_by_name: HashMap<String, Arc<DefaultCardpackData>> = HashMap::new();
        for pack in &pack_list {
            packs_by_name.insert(
                String::from(&pack.get_default_cardpack().name),
                Arc::clone(pack),
            );
        }
        DefaultCardpackHandler {
            pack_list,
            packs_by_name,
        }
    }

    pub fn new_with_hardcoded_packs() -> DefaultCardpackHandler {
        DefaultCardpackHandler::new_with_custom_packs(
            hardcoded_data::get_hardcoded_default_cardpack_data_list(),
        )
    }

    pub fn get_pack_list(&self) -> &Vec<Arc<DefaultCardpackData>> {
        &self.pack_list
    }

    pub fn get_pack_by_name(
        &self,
        name: &DefaultCardpackName,
    ) -> Option<&Arc<DefaultCardpackData>> {
        self.packs_by_name.get(name.get_string())
    }
}

pub struct DefaultCardpackData {
    default_cardpack: DefaultCardpack,
    default_black_cards: Vec<DefaultBlackCard>,
    default_white_cards: Vec<DefaultWhiteCard>,
}

impl DefaultCardpackData {
    fn new(
        default_cardpack: DefaultCardpack,
        default_black_cards: Vec<DefaultBlackCard>,
        default_white_cards: Vec<DefaultWhiteCard>,
    ) -> DefaultCardpackData {
        DefaultCardpackData {
            default_cardpack,
            default_black_cards,
            default_white_cards,
        }
    }

    pub fn create_list_from_raw_data(
        data: Vec<(String, Vec<(String, i32)>, Vec<String>)>,
    ) -> Vec<DefaultCardpackData> {
        let mut default_cardpack_data_list = Vec::new();
        for (
            i,
            (
                default_cardpack_display_name,
                default_black_card_data_list,
                default_white_card_data_list,
            ),
        ) in data.into_iter().enumerate()
        {
            let default_cardpack = create_default_cardpack(i, default_cardpack_display_name);
            let default_black_card_list = create_alpha_sorted_default_black_card_list(
                default_black_card_data_list,
                &default_cardpack.name,
            );
            let default_white_card_list = create_alpha_sorted_default_white_card_list(
                default_white_card_data_list,
                &default_cardpack.name,
            );
            default_cardpack_data_list.push(DefaultCardpackData::new(
                default_cardpack,
                default_black_card_list,
                default_white_card_list,
            ));
        }
        return default_cardpack_data_list;
    }

    pub fn get_default_cardpack(&self) -> &DefaultCardpack {
        &self.default_cardpack
    }

    pub fn get_default_black_cards(&self) -> &Vec<DefaultBlackCard> {
        &self.default_black_cards
    }

    pub fn get_default_white_cards(&self) -> &Vec<DefaultWhiteCard> {
        &self.default_white_cards
    }
}

fn create_alpha_sorted_default_black_card_list(
    mut data: Vec<(String, i32)>,
    parent_default_cardpack_name: &str,
) -> Vec<DefaultBlackCard> {
    data.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    let mut default_black_cards = Vec::new();
    for (i, (text, answer_fields)) in data.into_iter().enumerate() {
        default_black_cards.push(create_default_black_card(
            i,
            text,
            answer_fields,
            parent_default_cardpack_name,
        ));
    }
    return default_black_cards;
}

fn create_alpha_sorted_default_white_card_list(
    mut data: Vec<String>,
    parent_default_cardpack_name: &str,
) -> Vec<DefaultWhiteCard> {
    data.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    let mut default_white_cards = Vec::new();
    for (i, text) in data.into_iter().enumerate() {
        default_white_cards.push(create_default_white_card(
            i,
            text,
            parent_default_cardpack_name,
        ));
    }
    return default_white_cards;
}

fn create_default_cardpack(index: usize, display_name: String) -> DefaultCardpack {
    if display_name.is_empty() {
        panic!("The `display_name` property must not be empty for instances of DefaultBlackCard");
    }
    if display_name.trim() != display_name {
        panic!(format!("The `display_name` property must not start or end with whitespace for instances of DefaultBlackCard. Got `{}`.", display_name));
    }
    let default_cardpack = DefaultCardpack {
        name: format!(
            "defaultCardpacks/{}",
            hash_string(&format!("{}{}", index, display_name))
        ),
        display_name,
    };
    return default_cardpack;
}

fn create_default_black_card(
    index: usize,
    text: String,
    answer_fields: i32,
    parent_default_cardpack_name: &str,
) -> DefaultBlackCard {
    if text.is_empty() {
        panic!("The `text` property must not be empty for instances of DefaultBlackCard");
    }
    if text.trim() != text {
        panic!(format!("The `text` property must not start or end with whitespace for instances of DefaultBlackCard. Got `{}`.", text));
    }
    if answer_fields < 1 || answer_fields > 3 {
        panic!(
            "The `answer_fields` property must be 1, 2, or 3 for instances of DefaultBlackCard."
        );
    }
    let default_black_card = DefaultBlackCard {
        name: format!(
            "{}/defaultBlackCards/{}",
            parent_default_cardpack_name,
            hash_string(&format!(
                "{}{}{}{}",
                index, text, answer_fields, parent_default_cardpack_name
            ))
        ),
        text,
        answer_fields,
    };
    return default_black_card;
}

fn create_default_white_card(
    index: usize,
    text: String,
    parent_default_cardpack_name: &str,
) -> DefaultWhiteCard {
    if text.is_empty() {
        panic!("The `text` property must not be empty for instances of DefaultWhiteCard");
    }
    if text.trim() != text {
        panic!(format!("The `text` property must not start or end with whitespace for instances of DefaultWhiteCard. Got `{}`.", text));
    }
    let default_white_card = DefaultWhiteCard {
        name: format!(
            "{}/defaultWhiteCards/{}",
            parent_default_cardpack_name,
            hash_string(&format!(
                "{}{}{}",
                index, text, parent_default_cardpack_name
            ))
        ),
        text,
    };
    return default_white_card;
}

fn hash_string(data: &str) -> String {
    let hash: [u8; 32] = Sha256::digest(data.as_bytes()).into();
    return hex::encode(hash);
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::basic_validation::ValidatedStringField;

    #[test]
    fn test_create_list_from_raw_data() {
        // Can create empty list without panic.
        let mut default_cardpack_data_list =
            DefaultCardpackData::create_list_from_raw_data(Vec::new());
        assert_eq!(default_cardpack_data_list.is_empty(), true);

        // Can create list with single empty cardpack.
        default_cardpack_data_list = DefaultCardpackData::create_list_from_raw_data(vec![(
            String::from("Test Cardpack"),
            Vec::new(),
            Vec::new(),
        )]);
        assert_eq!(default_cardpack_data_list.len(), 1);
        assert_eq!(
            default_cardpack_data_list
                .first()
                .unwrap()
                .get_default_cardpack()
                .name,
            "defaultCardpacks/6328dfe30860851a5684c49fcc7c3efb81821b88c06aeefc1626d60afd8bdb71"
        );
        assert_eq!(
            default_cardpack_data_list
                .first()
                .unwrap()
                .get_default_cardpack()
                .display_name,
            "Test Cardpack"
        );
        assert_eq!(
            default_cardpack_data_list
                .first()
                .unwrap()
                .get_default_black_cards()
                .len(),
            0
        );
        assert_eq!(
            default_cardpack_data_list
                .first()
                .unwrap()
                .get_default_white_cards()
                .len(),
            0
        );

        // Identical cards and cardpacks have different names.
        default_cardpack_data_list = DefaultCardpackData::create_list_from_raw_data(vec![
            (
                String::from("Test Cardpack"),
                vec![(String::from("Black Card 1"), 1)],
                vec![String::from("White Card 1")],
            ),
            (
                String::from("Test Cardpack"),
                vec![(String::from("Black Card 1"), 1)],
                vec![String::from("White Card 1")],
            ),
        ]);
        // Cardpack assertions.
        assert_eq!(default_cardpack_data_list.len(), 2);
        assert_eq!(
            default_cardpack_data_list
                .get(0)
                .unwrap()
                .get_default_cardpack()
                .name,
            "defaultCardpacks/6328dfe30860851a5684c49fcc7c3efb81821b88c06aeefc1626d60afd8bdb71"
        );
        assert_eq!(
            default_cardpack_data_list
                .get(0)
                .unwrap()
                .get_default_cardpack()
                .display_name,
            "Test Cardpack"
        );
        assert_eq!(
            default_cardpack_data_list
                .get(1)
                .unwrap()
                .get_default_cardpack()
                .name,
            "defaultCardpacks/f60097a264f27896c92c91e7f3bd1f2501cff9808e55e7997050e57bcfc40573"
        );
        assert_eq!(
            default_cardpack_data_list
                .get(1)
                .unwrap()
                .get_default_cardpack()
                .display_name,
            "Test Cardpack"
        );
        // Black card assertions.
        assert_eq!(
            default_cardpack_data_list
                .get(0)
                .unwrap()
                .get_default_black_cards()
                .len(),
            1
        );
        assert_eq!(default_cardpack_data_list.get(0).unwrap().get_default_black_cards().first().unwrap().name, "defaultCardpacks/6328dfe30860851a5684c49fcc7c3efb81821b88c06aeefc1626d60afd8bdb71/defaultBlackCards/0fabc03506fde6e12b8d791c7fa473871afa4c39fd5cd894d099cea8e1b7d13a");
        assert_eq!(
            default_cardpack_data_list
                .get(0)
                .unwrap()
                .get_default_black_cards()
                .first()
                .unwrap()
                .text,
            "Black Card 1"
        );
        assert_eq!(
            default_cardpack_data_list
                .get(0)
                .unwrap()
                .get_default_black_cards()
                .first()
                .unwrap()
                .answer_fields,
            1
        );
        assert_eq!(
            default_cardpack_data_list
                .get(1)
                .unwrap()
                .get_default_black_cards()
                .len(),
            1
        );
        assert_eq!(default_cardpack_data_list.get(1).unwrap().get_default_black_cards().first().unwrap().name, "defaultCardpacks/f60097a264f27896c92c91e7f3bd1f2501cff9808e55e7997050e57bcfc40573/defaultBlackCards/f57b31d50f3c571c6ec5b4bab5a7ff93045743bd02fa80c4c431eb760bd3465c");
        assert_eq!(
            default_cardpack_data_list
                .get(1)
                .unwrap()
                .get_default_black_cards()
                .first()
                .unwrap()
                .text,
            "Black Card 1"
        );
        assert_eq!(
            default_cardpack_data_list
                .get(1)
                .unwrap()
                .get_default_black_cards()
                .first()
                .unwrap()
                .answer_fields,
            1
        );
        assert_ne!(
            default_cardpack_data_list
                .get(0)
                .unwrap()
                .get_default_black_cards()
                .first()
                .unwrap()
                .name
                .split(
                    &default_cardpack_data_list
                        .get(0)
                        .unwrap()
                        .get_default_cardpack()
                        .name
                )
                .collect::<Vec<_>>()
                .get(1)
                .unwrap(),
            default_cardpack_data_list
                .get(1)
                .unwrap()
                .get_default_black_cards()
                .first()
                .unwrap()
                .name
                .split(
                    &default_cardpack_data_list
                        .get(1)
                        .unwrap()
                        .get_default_cardpack()
                        .name
                )
                .collect::<Vec<_>>()
                .get(1)
                .unwrap()
        );
        // White card assertions.
        assert_eq!(
            default_cardpack_data_list
                .get(0)
                .unwrap()
                .get_default_white_cards()
                .len(),
            1
        );
        assert_eq!(default_cardpack_data_list.get(0).unwrap().get_default_white_cards().first().unwrap().name, "defaultCardpacks/6328dfe30860851a5684c49fcc7c3efb81821b88c06aeefc1626d60afd8bdb71/defaultWhiteCards/f460d79b64934c549d112cfc87dbc91e8accf98dc309fc1db5d7f0f91f3136ed");
        assert_eq!(
            default_cardpack_data_list
                .get(0)
                .unwrap()
                .get_default_white_cards()
                .first()
                .unwrap()
                .text,
            "White Card 1"
        );
        assert_eq!(
            default_cardpack_data_list
                .get(1)
                .unwrap()
                .get_default_white_cards()
                .len(),
            1
        );
        assert_eq!(default_cardpack_data_list.get(1).unwrap().get_default_white_cards().first().unwrap().name, "defaultCardpacks/f60097a264f27896c92c91e7f3bd1f2501cff9808e55e7997050e57bcfc40573/defaultWhiteCards/8b975c3c75ce978e062d610eb64bba6cbdffd046bbfa5f91d9d0423b95e7827a");
        assert_eq!(
            default_cardpack_data_list
                .get(1)
                .unwrap()
                .get_default_white_cards()
                .first()
                .unwrap()
                .text,
            "White Card 1"
        );
        assert_ne!(
            default_cardpack_data_list
                .get(0)
                .unwrap()
                .get_default_white_cards()
                .first()
                .unwrap()
                .name
                .split(
                    &default_cardpack_data_list
                        .get(0)
                        .unwrap()
                        .get_default_cardpack()
                        .name
                )
                .collect::<Vec<_>>()
                .get(1)
                .unwrap(),
            default_cardpack_data_list
                .get(1)
                .unwrap()
                .get_default_white_cards()
                .first()
                .unwrap()
                .name
                .split(
                    &default_cardpack_data_list
                        .get(1)
                        .unwrap()
                        .get_default_cardpack()
                        .name
                )
                .collect::<Vec<_>>()
                .get(1)
                .unwrap()
        );

        // Black cards and white cards are automatically sorted by their `text` property.
        default_cardpack_data_list = DefaultCardpackData::create_list_from_raw_data(vec![(
            String::from("Test Cardpack"),
            vec![
                (String::from("b"), 1),
                (String::from("A"), 1),
                (String::from("C"), 1),
                (String::from("d"), 1),
            ],
            vec![
                String::from("h"),
                String::from("G"),
                String::from("E"),
                String::from("f"),
            ],
        )]);
        assert_eq!(
            default_cardpack_data_list
                .first()
                .unwrap()
                .get_default_black_cards()
                .get(0)
                .unwrap()
                .text,
            "A"
        );
        assert_eq!(
            default_cardpack_data_list
                .first()
                .unwrap()
                .get_default_black_cards()
                .get(1)
                .unwrap()
                .text,
            "b"
        );
        assert_eq!(
            default_cardpack_data_list
                .first()
                .unwrap()
                .get_default_black_cards()
                .get(2)
                .unwrap()
                .text,
            "C"
        );
        assert_eq!(
            default_cardpack_data_list
                .first()
                .unwrap()
                .get_default_black_cards()
                .get(3)
                .unwrap()
                .text,
            "d"
        );
        assert_eq!(
            default_cardpack_data_list
                .first()
                .unwrap()
                .get_default_white_cards()
                .get(0)
                .unwrap()
                .text,
            "E"
        );
        assert_eq!(
            default_cardpack_data_list
                .first()
                .unwrap()
                .get_default_white_cards()
                .get(1)
                .unwrap()
                .text,
            "f"
        );
        assert_eq!(
            default_cardpack_data_list
                .first()
                .unwrap()
                .get_default_white_cards()
                .get(2)
                .unwrap()
                .text,
            "G"
        );
        assert_eq!(
            default_cardpack_data_list
                .first()
                .unwrap()
                .get_default_white_cards()
                .get(3)
                .unwrap()
                .text,
            "h"
        );
    }

    #[test]
    #[should_panic(
        expected = "The `display_name` property must not start or end with whitespace for instances of DefaultBlackCard. Got `    Test Cardpack    `."
    )]
    fn test_create_list_from_raw_data_panics_with_cardpack_display_name_whitespace() {
        DefaultCardpackData::create_list_from_raw_data(vec![(
            String::from("    Test Cardpack    "),
            vec![(String::from("Black Card 1"), 1)],
            vec![String::from("White Card 1")],
        )]);
    }

    #[test]
    #[should_panic(
        expected = "The `text` property must not start or end with whitespace for instances of DefaultBlackCard. Got `    Black Card 1    `."
    )]
    fn test_create_list_from_raw_data_panics_with_black_card_text_whitespace() {
        DefaultCardpackData::create_list_from_raw_data(vec![(
            String::from("Test Cardpack"),
            vec![(String::from("    Black Card 1    "), 1)],
            vec![String::from("White Card 1")],
        )]);
    }

    #[test]
    #[should_panic(
        expected = "The `answer_fields` property must be 1, 2, or 3 for instances of DefaultBlackCard."
    )]
    fn test_create_list_from_raw_data_panics_with_black_card_out_of_bounds_answer_fields() {
        DefaultCardpackData::create_list_from_raw_data(vec![(
            String::from("Test Cardpack"),
            vec![(String::from("Black Card 1"), 0)],
            vec![String::from("White Card 1")],
        )]);
    }

    #[test]
    #[should_panic(
        expected = "The `text` property must not start or end with whitespace for instances of DefaultWhiteCard. Got `    White Card 1    `."
    )]
    fn test_create_list_from_raw_data_panics_with_white_card_text_whitespace() {
        DefaultCardpackData::create_list_from_raw_data(vec![(
            String::from("Test Cardpack"),
            vec![(String::from("Black Card 1"), 1)],
            vec![String::from("    White Card 1    ")],
        )]);
    }

    #[test]
    fn test_create_default_cardpack_handler_with_hardcoded_packs_without_panic() {
        DefaultCardpackHandler::new_with_hardcoded_packs();
    }

    #[test]
    fn test_default_cardpack_handler() {
        let handler = DefaultCardpackHandler::new_with_custom_packs(
            DefaultCardpackData::create_list_from_raw_data(vec![(
                String::from("Cardpack 1"),
                vec![(String::from("Black Card 1"), 1)],
                vec![String::from("White Card 1")],
            )]),
        );
        assert_eq!(handler.get_pack_list().len(), 1);
        assert!(handler
            .get_pack_by_name(
                &DefaultCardpackName::new(
                    &ValidatedStringField::new("defaultCardpacks/fake_cardpack_name", "").unwrap()
                )
                .unwrap()
            )
            .is_none());
        let pack = handler
            .get_pack_by_name(
                &DefaultCardpackName::new(
                    &ValidatedStringField::new(
                        &handler
                            .get_pack_list()
                            .first()
                            .unwrap()
                            .get_default_cardpack()
                            .name,
                        "",
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(pack.get_default_black_cards().len(), 1);
        assert_eq!(pack.get_default_white_cards().len(), 1);
    }
}
