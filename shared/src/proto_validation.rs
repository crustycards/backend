use super::proto::{
    game_config::{
        blank_white_card_config::{Behavior, BlankWhiteCardsAdded},
        BlankWhiteCardConfig, EndCondition,
    },
    GameConfig,
    user_settings::ColorScheme,
    OAuthCredentials,
};
use super::constants::*;
use tonic::Status;
use super::grpc_error::empty_request_field_error;
use super::basic_validation::{BoundedNumberField, ValidatedStringField};

pub struct ValidatedOAuthCredentials {
    oauth_credentials: OAuthCredentials,
}

impl ValidatedOAuthCredentials {
    pub fn new(oauth_credentials: &OAuthCredentials, field_name: &str) -> Result<Self, Status> {
        Ok(Self {
            oauth_credentials: OAuthCredentials {
                oauth_provider: ValidatedStringField::new(
                    &oauth_credentials.oauth_provider,
                    &format!("{}.oauth_provider", field_name),
                )?
                .take_string(),
                oauth_id: ValidatedStringField::new(
                    &oauth_credentials.oauth_id,
                    &format!("{}.oauth_id", field_name),
                )?
                .take_string(),
            },
        })
    }

    pub fn take_oauth_credentials(self) -> OAuthCredentials {
        self.oauth_credentials
    }
}

pub struct AnswerFieldCount {
    count: BoundedNumberField,
}

impl AnswerFieldCount {
    pub fn new(count: i32, field_name: &str) -> Result<Self, Status> {
        if count == 0 {
            return Err(empty_request_field_error(field_name));
        }

        Ok(Self {
            count: BoundedNumberField::new(count, 1, 3, field_name)?,
        })
    }

    pub fn get_count(&self) -> &i32 {
        &self.count.get_value()
    }

    pub fn take_count(self) -> i32 {
        self.count.take_value()
    }
}

pub struct BoundedPageSize {
    page_size: i64,
}

impl BoundedPageSize {
    fn get_bounded_page_size(page_size: i32) -> Result<i64, Status> {
        if page_size < 0 {
            return Err(Status::invalid_argument("Page size cannot be negative."));
        } else if page_size == 0 {
            return Ok(50);
        } else if page_size > 1000 {
            return Ok(1000);
        } else {
            return Ok(page_size as i64);
        }
    }

    pub fn new(page_size: i32) -> Result<Self, Status> {
        Ok(Self {
            page_size: Self::get_bounded_page_size(page_size)?,
        })
    }

    // This returns an i64 because that's what mongodb takes as the query return limit.
    pub fn take_i64(self) -> i64 {
        self.page_size
    }
}

pub enum ValidatedColorScheme {
    DefaultLight = 1,
    DefaultDark = 2,
}

impl ValidatedColorScheme {
    pub fn new(color_scheme_i32: i32) -> Result<Self, Status> {
        let color_scheme = match ColorScheme::from_i32(color_scheme_i32) {
            Some(color_scheme) => color_scheme,
            None => {
                return Err(Status::invalid_argument(
                    "Color scheme must be a currently supported value.",
                ))
            }
        };

        Ok(match color_scheme {
            ColorScheme::Unspecified => {
                return Err(Status::invalid_argument(
                    "Color scheme cannot be unspecified.",
                ))
            }
            ColorScheme::DefaultLight => ValidatedColorScheme::DefaultLight,
            ColorScheme::DefaultDark => ValidatedColorScheme::DefaultDark,
        })
    }
}

pub enum OptionalField<T> {
    Set(T),
    Unset,
}

pub struct ValidatedGameConfig {
    display_name: String,
    max_players: usize,
    end_condition: EndCondition,
    hand_size: usize,
    custom_cardpack_names: Vec<String>,
    default_cardpack_names: Vec<String>,
    blank_white_card_config: BlankWhiteCardConfig,
}

// TODO - Let's make a constructor that accepts a field_name so that we can make better error messages.
impl ValidatedGameConfig {
    pub fn new(config: GameConfig) -> Result<Self, Status> {
        let display_name = config.display_name.trim();

        if display_name.is_empty() {
            return Err(Status::invalid_argument(
                "Game config property `display_name` cannot be empty.",
            ));
        }

        if config.max_players > MAX_PLAYER_LIMIT {
            return Err(Status::invalid_argument(&format!(
                "Game config property `max_players` must not exceed {}.",
                MAX_PLAYER_LIMIT
            )));
        }
        if config.max_players < MIN_PLAYER_LIMIT {
            return Err(Status::invalid_argument(&format!(
                "Game config property `max_players` must be at least {}.",
                MIN_PLAYER_LIMIT
            )));
        }

        let end_condition: EndCondition = match &config.end_condition {
            Some(end_condition) => {
                match end_condition {
                    EndCondition::EndlessMode(_) => {
                        EndCondition::EndlessMode(())
                    },
                    EndCondition::MaxScore(max_score) => {
                        if max_score > &MAX_SCORE_LIMIT {
                            return Err(Status::invalid_argument(&format!(
                                "Game config property `max_score` must not exceed {}.",
                                MAX_SCORE_LIMIT
                            )));
                        }
                        if max_score < &MIN_SCORE_LIMIT {
                            return Err(Status::invalid_argument(&format!(
                                "Game config property `max_score` must be at least {}.",
                                MIN_SCORE_LIMIT
                            )));
                        }
                        EndCondition::MaxScore(*max_score)
                    }
                }
            },
            None => return Err(Status::invalid_argument("Game config must specify a win condition using either the `max_score` or `endless_mode` property."))
        };

        if config.hand_size > MAX_HAND_SIZE_LIMIT {
            return Err(Status::invalid_argument(&format!(
                "Game config property `hand_size` must not exceed {}.",
                MAX_HAND_SIZE_LIMIT
            )));
        }
        if config.hand_size < MIN_HAND_SIZE_LIMIT {
            return Err(Status::invalid_argument(&format!(
                "Game config property `hand_size` must be at least {}.",
                MIN_HAND_SIZE_LIMIT
            )));
        }

        if config.custom_cardpack_names.is_empty() && config.default_cardpack_names.is_empty() {
            return Err(Status::invalid_argument("Game config must contain at least one value for either `custom_cardpack_names` or `default_cardpack_names`."));
        }

        let blank_white_card_config = match config.blank_white_card_config {
            Some(config) => {
                match Self::validate_blank_white_card_config(&config) {
                    Err(err) => return Err(err),
                    _ => {}
                };
                config
            }
            None => {
                return Err(Status::invalid_argument(
                    "Game config property `blank_white_card_config` cannot be blank.",
                ))
            }
        };

        Ok(Self {
            display_name: display_name.to_string(),
            max_players: config.max_players as usize,
            end_condition,
            hand_size: config.hand_size as usize,
            custom_cardpack_names: config.custom_cardpack_names,
            default_cardpack_names: config.default_cardpack_names,
            blank_white_card_config,
        })
    }

    pub fn get_max_players(&self) -> usize {
        self.max_players
    }

    pub fn get_end_condition(&self) -> &EndCondition {
        &self.end_condition
    }

    pub fn get_hand_size(&self) -> usize {
        self.hand_size
    }

    pub fn get_custom_cardpack_names(&self) -> &[String] {
        &self.custom_cardpack_names
    }

    pub fn get_default_cardpack_names(&self) -> &[String] {
        &self.default_cardpack_names
    }

    pub fn get_blank_white_card_config(&self) -> &BlankWhiteCardConfig {
        &self.blank_white_card_config
    }

    pub fn raw_config(&self) -> GameConfig {
        GameConfig {
            display_name: self.display_name.clone(),
            max_players: self.max_players as i32,
            end_condition: Some(self.end_condition.clone()),
            hand_size: self.hand_size as i32,
            custom_cardpack_names: self.custom_cardpack_names.clone(),
            default_cardpack_names: self.default_cardpack_names.clone(),
            blank_white_card_config: Some(self.blank_white_card_config.clone()),
        }
    }

    fn validate_blank_white_card_config(
        blank_white_card_config: &BlankWhiteCardConfig,
    ) -> Result<(), Status> {
        let behavior = match Behavior::from_i32(blank_white_card_config.behavior) {
            Some(behavior) => behavior,
            None => return Err(Status::invalid_argument("Game config property `blank_white_card_config.behavior` must be a valid enum value.")) // TODO - Test this case.
        };

        match &blank_white_card_config.blank_white_cards_added {
            Some(blank_white_cards_added) => {
                match blank_white_cards_added {
                    BlankWhiteCardsAdded::CardCount(card_count) => {
                        if card_count < &0 {
                            return Err(Status::invalid_argument(
                                "Game config property `blank_white_card_config.card_count` cannot be negative.",
                            ));
                        }
                        if card_count > &10000 {
                            return Err(Status::invalid_argument(
                                "Game config property `blank_white_card_config.card_count` must not exceed 10000.",
                            ));
                        }
                    }
                    BlankWhiteCardsAdded::Percentage(percentage) => {
                        if percentage < &0.0 {
                            return Err(Status::invalid_argument(
                                "Game config property `blank_white_card_config.percentage` cannot be negative.",
                            ));
                        }
                        if percentage > &0.8 {
                            return Err(Status::invalid_argument(
                                "Game config property `blank_white_card_config.percentage` must not exceed 0.8.",
                            ));
                        }
                    }
                };
            }
            None => {}
        };

        match behavior {
            Behavior::Unspecified => {
                return Err(Status::invalid_argument(
                    "Game config property `blank_white_card_config.behavior` cannot be left unspecified.",
                ));
            }
            Behavior::Disabled => {
                if blank_white_card_config.blank_white_cards_added.is_some() {
                    return Err(Status::invalid_argument("Game config cannot have value for `card_count` or `percentage` since property `blank_white_card_config.behavior` is set to DISABLED."));
                }
            }
            _ => {
                if blank_white_card_config.blank_white_cards_added.is_none() {
                    return Err(Status::invalid_argument("Game config requires value for `card_count` or `percentage` since property `blank_white_card_config.behavior` is not set to DISABLED."));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test::helper::get_valid_test_game_config;
    use super::*;

    #[test]
    fn test_get_bounded_page_size() {
        assert_eq!(format!("{}", BoundedPageSize::get_bounded_page_size(-1000).unwrap_err()), "status: InvalidArgument, message: \"Page size cannot be negative.\", details: [], metadata: MetadataMap { headers: {} }");
        assert_eq!(format!("{}", BoundedPageSize::get_bounded_page_size(-1).unwrap_err()), "status: InvalidArgument, message: \"Page size cannot be negative.\", details: [], metadata: MetadataMap { headers: {} }");
        assert_eq!(BoundedPageSize::get_bounded_page_size(0).unwrap(), 50);
        assert_eq!(BoundedPageSize::get_bounded_page_size(1).unwrap(), 1);
        assert_eq!(BoundedPageSize::get_bounded_page_size(100).unwrap(), 100);
        assert_eq!(BoundedPageSize::get_bounded_page_size(1000).unwrap(), 1000);
        assert_eq!(BoundedPageSize::get_bounded_page_size(1001).unwrap(), 1000);
        assert_eq!(BoundedPageSize::get_bounded_page_size(10000).unwrap(), 1000);
    }

    #[test]
    fn validate_and_sanitize_game_config() {
        // Sanity check.
        let mut game_config = get_valid_test_game_config();
        assert_eq!(ValidatedGameConfig::new(game_config).is_ok(), true);

        // Catches missing display_name.
        game_config = get_valid_test_game_config();
        game_config.display_name.clear();
        assert_eq!(
            format!(
                "{}",
                ValidatedGameConfig::new(game_config).err().unwrap()
            ),
            "status: InvalidArgument, message: \"Game config property `display_name` cannot be empty.\", details: [], metadata: MetadataMap { headers: {} }"
        );

        // Catches display_name containing only whitespace.
        game_config = get_valid_test_game_config();
        game_config.display_name = String::from("        ");
        assert_eq!(
            format!(
                "{}",
                ValidatedGameConfig::new(game_config).err().unwrap()
            ),
            "status: InvalidArgument, message: \"Game config property `display_name` cannot be empty.\", details: [], metadata: MetadataMap { headers: {} }"
        );

        // Trims whitespace from display_name.
        game_config = get_valid_test_game_config();
        game_config.display_name = String::from(" Game Name ");
        assert_eq!(ValidatedGameConfig::new(game_config.clone()).is_ok(), true);
        assert_eq!(
            ValidatedGameConfig::new(game_config.clone())
                .unwrap()
                .display_name,
            "Game Name"
        );

        // Catches invalid max_players.
        game_config = get_valid_test_game_config();
        game_config.max_players = MIN_PLAYER_LIMIT;
        assert_eq!(ValidatedGameConfig::new(game_config.clone()).is_ok(), true);
        game_config.max_players = MIN_PLAYER_LIMIT - 1;
        assert_eq!(
            format!(
                "{}",
                ValidatedGameConfig::new(game_config.clone()).err().unwrap()
            ),
            "status: InvalidArgument, message: \"Game config property `max_players` must be at least 2.\", details: [], metadata: MetadataMap { headers: {} }"
        );
        game_config.max_players = MAX_PLAYER_LIMIT;
        assert_eq!(ValidatedGameConfig::new(game_config.clone()).is_ok(), true);
        game_config.max_players = MAX_PLAYER_LIMIT + 1;
        assert_eq!(
            format!(
                "{}",
                ValidatedGameConfig::new(game_config).err().unwrap()
            ),
            "status: InvalidArgument, message: \"Game config property `max_players` must not exceed 100.\", details: [], metadata: MetadataMap { headers: {} }"
        );

        // Catches invalid max_score.
        game_config = get_valid_test_game_config();
        game_config.end_condition = Some(EndCondition::MaxScore(MIN_SCORE_LIMIT));
        assert_eq!(ValidatedGameConfig::new(game_config.clone()).is_ok(), true);
        game_config.end_condition = Some(EndCondition::MaxScore(MIN_SCORE_LIMIT - 1));
        assert_eq!(format!("{}", ValidatedGameConfig::new(game_config.clone()).err().unwrap()), "status: InvalidArgument, message: \"Game config property `max_score` must be at least 1.\", details: [], metadata: MetadataMap { headers: {} }");
        game_config.end_condition = Some(EndCondition::MaxScore(MIN_SCORE_LIMIT - 2));
        assert_eq!(
            format!(
                "{}",
                ValidatedGameConfig::new(game_config.clone()).err().unwrap()
            ),
            "status: InvalidArgument, message: \"Game config property `max_score` must be at least 1.\", details: [], metadata: MetadataMap { headers: {} }"
        );
        game_config.end_condition = Some(EndCondition::MaxScore(MAX_SCORE_LIMIT));
        assert_eq!(ValidatedGameConfig::new(game_config.clone()).is_ok(), true);
        game_config.end_condition = Some(EndCondition::MaxScore(MAX_SCORE_LIMIT + 1));
        assert_eq!(
            format!(
                "{}",
                ValidatedGameConfig::new(game_config).err().unwrap()
            ),
            "status: InvalidArgument, message: \"Game config property `max_score` must not exceed 100.\", details: [], metadata: MetadataMap { headers: {} }"
        );

        // Catches missing end_condition.
        game_config = get_valid_test_game_config();
        game_config.end_condition = None;
        assert_eq!(format!("{}", ValidatedGameConfig::new(game_config).err().unwrap()), "status: InvalidArgument, message: \"Game config must specify a win condition using either the `max_score` or `endless_mode` property.\", details: [], metadata: MetadataMap { headers: {} }");

        // Catches invalid hand_size.
        game_config = get_valid_test_game_config();
        game_config.hand_size = MIN_HAND_SIZE_LIMIT;
        assert_eq!(ValidatedGameConfig::new(game_config.clone()).is_ok(), true);
        game_config.hand_size = MIN_HAND_SIZE_LIMIT - 1;
        assert_eq!(
            format!(
                "{}",
                ValidatedGameConfig::new(game_config.clone()).err().unwrap()
            ),
            "status: InvalidArgument, message: \"Game config property `hand_size` must be at least 3.\", details: [], metadata: MetadataMap { headers: {} }"
        );
        game_config.hand_size = MAX_HAND_SIZE_LIMIT;
        assert_eq!(ValidatedGameConfig::new(game_config.clone()).is_ok(), true);
        game_config.hand_size = MAX_HAND_SIZE_LIMIT + 1;
        assert_eq!(
            format!(
                "{}",
                ValidatedGameConfig::new(game_config).err().unwrap()
            ),
            "status: InvalidArgument, message: \"Game config property `hand_size` must not exceed 20.\", details: [], metadata: MetadataMap { headers: {} }"
        );

        // Catches empty custom_cardpack_names and default_cardpack_names lists.
        game_config = get_valid_test_game_config();
        game_config.custom_cardpack_names.clear();
        game_config.default_cardpack_names.clear();
        assert_eq!(format!("{}", ValidatedGameConfig::new(game_config).err().unwrap()), "status: InvalidArgument, message: \"Game config must contain at least one value for either `custom_cardpack_names` or `default_cardpack_names`.\", details: [], metadata: MetadataMap { headers: {} }");

        // Ignores empty custom_cardpack_names list if default_cardpack_names is not empty.
        game_config = get_valid_test_game_config();
        game_config.default_cardpack_names.clear();
        assert_eq!(ValidatedGameConfig::new(game_config).is_ok(), true);

        // Ignores empty default_cardpack_names list if custom_cardpack_names is not empty.
        game_config = get_valid_test_game_config();
        game_config.custom_cardpack_names.clear();
        assert_eq!(ValidatedGameConfig::new(game_config).is_ok(), true);

        // Catches empty blank_white_card_config.behavior.
        game_config = get_valid_test_game_config();
        match &mut game_config.blank_white_card_config {
            Some(config) => {
                config.behavior = Behavior::Unspecified.into();
            }
            None => panic!(),
        };
        assert_eq!(format!("{}", ValidatedGameConfig::new(game_config).err().unwrap()), "status: InvalidArgument, message: \"Game config property `blank_white_card_config.behavior` cannot be left unspecified.\", details: [], metadata: MetadataMap { headers: {} }");

        // Catches when blank_white_card_config.behavior is DISABLED while also missing blank_white_cards_added (and vice versa).
        game_config = get_valid_test_game_config();
        game_config.blank_white_card_config = Some(BlankWhiteCardConfig {
            behavior: Behavior::OpenText.into(),
            blank_white_cards_added: None,
        });
        assert_eq!(format!("{}", ValidatedGameConfig::new(game_config.clone()).err().unwrap()), "status: InvalidArgument, message: \"Game config requires value for `card_count` or `percentage` since property `blank_white_card_config.behavior` is not set to DISABLED.\", details: [], metadata: MetadataMap { headers: {} }");
        game_config.blank_white_card_config = Some(BlankWhiteCardConfig {
            behavior: Behavior::Disabled.into(),
            blank_white_cards_added: Some(BlankWhiteCardsAdded::CardCount(10)),
        });
        assert_eq!(format!("{}", ValidatedGameConfig::new(game_config).err().unwrap()), "status: InvalidArgument, message: \"Game config cannot have value for `card_count` or `percentage` since property `blank_white_card_config.behavior` is set to DISABLED.\", details: [], metadata: MetadataMap { headers: {} }");

        // Catches when blank_white_card_config.card_count is out of range.
        game_config = get_valid_test_game_config();
        game_config.blank_white_card_config = Some(BlankWhiteCardConfig {
            behavior: Behavior::OpenText.into(),
            blank_white_cards_added: Some(BlankWhiteCardsAdded::CardCount(-1)),
        });
        assert_eq!(format!("{}", ValidatedGameConfig::new(game_config.clone()).err().unwrap()), "status: InvalidArgument, message: \"Game config property `blank_white_card_config.card_count` cannot be negative.\", details: [], metadata: MetadataMap { headers: {} }");
        match &mut game_config.blank_white_card_config {
            Some(config) => {
                config.blank_white_cards_added = Some(BlankWhiteCardsAdded::CardCount(10000));
            }
            None => panic!(),
        };
        assert_eq!(ValidatedGameConfig::new(game_config.clone()).is_ok(), true);
        match &mut game_config.blank_white_card_config {
            Some(config) => {
                config.blank_white_cards_added = Some(BlankWhiteCardsAdded::CardCount(10001));
            }
            None => panic!(),
        };
        assert_eq!(format!("{}", ValidatedGameConfig::new(game_config).err().unwrap()), "status: InvalidArgument, message: \"Game config property `blank_white_card_config.card_count` must not exceed 10000.\", details: [], metadata: MetadataMap { headers: {} }");

        // Catches when blank_white_card_config.percentage is out of range.
        game_config = get_valid_test_game_config();
        game_config.blank_white_card_config = Some(BlankWhiteCardConfig {
            behavior: Behavior::OpenText.into(),
            blank_white_cards_added: Some(BlankWhiteCardsAdded::Percentage(-1.0)),
        });
        assert_eq!(format!("{}", ValidatedGameConfig::new(game_config.clone()).err().unwrap()), "status: InvalidArgument, message: \"Game config property `blank_white_card_config.percentage` cannot be negative.\", details: [], metadata: MetadataMap { headers: {} }");
        match &mut game_config.blank_white_card_config {
            Some(config) => {
                config.blank_white_cards_added = Some(BlankWhiteCardsAdded::Percentage(0.5));
            }
            None => panic!(),
        };
        assert_eq!(ValidatedGameConfig::new(game_config.clone()).is_ok(), true);
        match &mut game_config.blank_white_card_config {
            Some(config) => {
                config.blank_white_cards_added = Some(BlankWhiteCardsAdded::Percentage(0.8));
            }
            None => panic!(),
        };
        assert_eq!(ValidatedGameConfig::new(game_config.clone()).is_ok(), true);
        match &mut game_config.blank_white_card_config {
            Some(config) => {
                config.blank_white_cards_added = Some(BlankWhiteCardsAdded::Percentage(0.8001));
            }
            None => panic!(),
        };
        assert_eq!(format!("{}", ValidatedGameConfig::new(game_config).err().unwrap()), "status: InvalidArgument, message: \"Game config property `blank_white_card_config.percentage` must not exceed 0.8.\", details: [], metadata: MetadataMap { headers: {} }");
    }
}
