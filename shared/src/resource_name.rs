use super::basic_validation::ValidatedStringField;
use bson::oid::ObjectId;
use tonic::Status;

#[derive(PartialEq, std::fmt::Debug)]
pub enum ParseNameError {
    PathDoesNotMatchFormat(String, String),
    IncorrectNumberOfTokenFields(String),
    ObjectIdParseError(String),
}

impl ParseNameError {
    pub fn to_status(&self) -> Status {
        match self {
            ParseNameError::PathDoesNotMatchFormat(format, path) => {
                Status::invalid_argument(format!(
                    "Resource with name `{}` should adhere to format `{}`.",
                    path, format
                ))
            }
            ParseNameError::IncorrectNumberOfTokenFields(path) => Status::invalid_argument(
                format!("Incorrect number of token fields in path `{}`.", path),
            ),
            ParseNameError::ObjectIdParseError(path) => {
                Status::not_found(format!("Resource with name `{}` does not exist.", path))
            }
        }
    }
}

impl std::fmt::Display for ParseNameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PathDoesNotMatchFormat(expected_format, path) => formatter.write_str(&format!(
                "Expected name format `{}` but got `{}`.",
                expected_format, path
            )),
            Self::IncorrectNumberOfTokenFields(path) => formatter.write_str(&format!(
                "Incorrect number of token fields in name `{}`.",
                path
            )),
            Self::ObjectIdParseError(path) => formatter.write_str(&format!(
                "Failed to parse MongoDB ObjectId from name `{}`.",
                path
            )),
        }
    }
}

fn parse_name(format: &str, path: &str) -> Result<Vec<String>, ParseNameError> {
    let mut tokens = Vec::new();
    for (a, b) in format.split('/').zip(path.split('/')) {
        if a == "{}" {
            tokens.push(String::from(b));
        } else if a != b {
            return Err(ParseNameError::PathDoesNotMatchFormat(
                format.to_string(),
                path.to_string(),
            ));
        }
    }
    if format.split('/').enumerate().last().unwrap().0
        != path.split('/').enumerate().last().unwrap().0
    {
        return Err(ParseNameError::PathDoesNotMatchFormat(
            format.to_string(),
            path.to_string(),
        ));
    }
    Ok(tokens)
}

// TODO - Test this function.
fn parse_one_token_name_to_string(format: &str, path: &str) -> Result<String, ParseNameError> {
    let tokens = match parse_name(format, path) {
        Ok(tokens) => tokens,
        Err(err) => return Err(err),
    };

    if tokens.len() != 1 {
        return Err(ParseNameError::IncorrectNumberOfTokenFields(
            path.to_string(),
        ));
    }

    // Unwrap is safe here because we checked
    // that `tokens` contains exactly one element.
    Ok(String::from(tokens.first().unwrap()))
}

// TODO - Test this function.
fn parse_one_token_name_to_object_id(format: &str, path: &str) -> Result<ObjectId, ParseNameError> {
    let id_string = match parse_one_token_name_to_string(format, path) {
        Ok(id_string) => id_string,
        Err(err) => return Err(err),
    };

    match ObjectId::parse_str(&id_string) {
        Ok(object_id) => Ok(object_id),
        _ => Err(ParseNameError::ObjectIdParseError(path.to_string())),
    }
}

// TODO - Test this function.
fn parse_two_token_name_to_object_ids(
    format: &str,
    path: &str,
) -> Result<(ObjectId, ObjectId), ParseNameError> {
    let tokens = match parse_name(format, path) {
        Ok(tokens) => tokens,
        Err(err) => return Err(err),
    };

    if tokens.len() != 2 {
        return Err(ParseNameError::IncorrectNumberOfTokenFields(
            path.to_string(),
        ));
    }

    // Unwraps are safe here because we checked
    // that `tokens` contains exactly two elements.
    let first_object_id = match ObjectId::parse_str(tokens.get(0).unwrap()) {
        Ok(object_id) => object_id,
        _ => return Err(ParseNameError::ObjectIdParseError(path.to_string())),
    };
    let second_object_id = match ObjectId::parse_str(tokens.get(1).unwrap()) {
        Ok(object_id) => object_id,
        _ => return Err(ParseNameError::ObjectIdParseError(path.to_string())),
    };
    Ok((first_object_id, second_object_id))
}

// TODO - Test this function.
fn parse_three_token_name_to_object_ids(
    format: &str,
    path: &str,
) -> Result<(ObjectId, ObjectId, ObjectId), ParseNameError> {
    let tokens = match parse_name(format, path) {
        Ok(tokens) => tokens,
        Err(err) => return Err(err),
    };

    if tokens.len() != 3 {
        return Err(ParseNameError::IncorrectNumberOfTokenFields(
            path.to_string(),
        ));
    }

    // Unwraps are safe here because we checked
    // that `tokens` contains exactly three elements.
    let first_object_id = match ObjectId::parse_str(tokens.get(0).unwrap()) {
        Ok(object_id) => object_id,
        _ => return Err(ParseNameError::ObjectIdParseError(path.to_string())),
    };
    let second_object_id = match ObjectId::parse_str(tokens.get(1).unwrap()) {
        Ok(object_id) => object_id,
        _ => return Err(ParseNameError::ObjectIdParseError(path.to_string())),
    };
    let third_object_id = match ObjectId::parse_str(tokens.get(2).unwrap()) {
        Ok(object_id) => object_id,
        _ => return Err(ParseNameError::ObjectIdParseError(path.to_string())),
    };
    Ok((first_object_id, second_object_id, third_object_id))
}

macro_rules! top_level_mongo_based_resource_name {
    ($struct_name:ident, $resource_path:expr) => {
        #[derive(Clone, Hash, PartialEq, Eq)]
        pub struct $struct_name {
            object_id: ObjectId,
        }

        impl $struct_name {
            pub fn new(resource_name: &ValidatedStringField) -> Result<Self, ParseNameError> {
                Self::new_from_str(resource_name.get_string())
            }

            pub fn new_from_str(resource_name: &str) -> Result<Self, ParseNameError> {
                match parse_one_token_name_to_object_id($resource_path, resource_name) {
                    Ok(object_id) => Ok(Self { object_id }),
                    Err(err) => Err(err),
                }
            }

            pub fn clone_str(&self) -> String {
                format!($resource_path, self.object_id.to_hex())
            }

            pub fn get_object_id(&self) -> &ObjectId {
                &self.object_id
            }

            pub fn take_object_id(self) -> ObjectId {
                self.object_id
            }
        }
    };
}

top_level_mongo_based_resource_name!(UserName, "users/{}");
top_level_mongo_based_resource_name!(UserSettingsName, "users/{}/settings");
top_level_mongo_based_resource_name!(UserProfileImageName, "users/{}/profileImage");

impl UserProfileImageName {
    pub fn to_user_name(&self) -> UserName {
        UserName {
            object_id: self.object_id,
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct CustomCardpackName {
    parent_user_object_id: ObjectId,
    object_id: ObjectId,
}

impl CustomCardpackName {
    pub fn new(
        custom_cardpack_resource_name: &ValidatedStringField,
    ) -> Result<Self, ParseNameError> {
        Self::new_from_str(custom_cardpack_resource_name.get_string())
    }

    pub fn new_from_str(custom_cardpack_resource_name: &str) -> Result<Self, ParseNameError> {
        match parse_two_token_name_to_object_ids(
            "users/{}/cardpacks/{}",
            custom_cardpack_resource_name,
        ) {
            Ok((parent_user_object_id, object_id)) => Ok(Self {
                parent_user_object_id,
                object_id,
            }),
            Err(err) => Err(err),
        }
    }

    pub fn new_from_parent(parent: UserName, object_id: ObjectId) -> Self {
        let parent_user_object_id = parent.take_object_id();
        Self {
            parent_user_object_id,
            object_id,
        }
    }

    pub fn clone_str(&self) -> String {
        format!(
            "users/{}/cardpacks/{}",
            self.parent_user_object_id.to_hex(),
            self.object_id.to_hex()
        )
    }

    pub fn get_object_ids(&self) -> (&ObjectId, &ObjectId) {
        (&self.parent_user_object_id, &self.object_id)
    }

    pub fn take_object_ids(self) -> (ObjectId, ObjectId) {
        (self.parent_user_object_id, self.object_id)
    }
}

macro_rules! custom_card_name {
    ($struct_name:ident, $resource_path:expr) => {
        #[derive(Clone, Hash, PartialEq, Eq)]
        pub struct $struct_name {
            parent_user_object_id: ObjectId,
            parent_custom_cardpack_object_id: ObjectId,
            object_id: ObjectId,
        }

        impl $struct_name {
            pub fn new(resource_name: &ValidatedStringField) -> Result<Self, ParseNameError> {
                match parse_three_token_name_to_object_ids(
                    &format!("{}{}{}", "users/{}/cardpacks/{}/", $resource_path, "/{}"),
                    resource_name.get_string(),
                ) {
                    Ok((parent_user_object_id, parent_custom_cardpack_object_id, object_id)) => {
                        Ok(Self {
                            parent_user_object_id,
                            parent_custom_cardpack_object_id,
                            object_id,
                        })
                    }
                    Err(err) => Err(err),
                }
            }

            pub fn new_from_parent(parent: CustomCardpackName, object_id: ObjectId) -> Self {
                let (parent_user_object_id, parent_custom_cardpack_object_id) =
                    parent.take_object_ids();
                Self {
                    parent_user_object_id,
                    parent_custom_cardpack_object_id,
                    object_id,
                }
            }

            pub fn clone_str(&self) -> String {
                format!(
                    "users/{}/cardpacks/{}/{}/{}",
                    self.parent_user_object_id.to_hex(),
                    self.parent_custom_cardpack_object_id.to_hex(),
                    $resource_path,
                    self.object_id.to_hex()
                )
            }

            pub fn take_object_ids(self) -> (ObjectId, ObjectId, ObjectId) {
                (
                    self.parent_user_object_id,
                    self.parent_custom_cardpack_object_id,
                    self.object_id,
                )
            }
        }
    };
}

custom_card_name!(CustomBlackCardName, "blackCards");
custom_card_name!(CustomWhiteCardName, "whiteCards");

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct DefaultCardpackName {
    string: String,
}

impl DefaultCardpackName {
    pub fn new(
        default_cardpack_resource_name: &ValidatedStringField,
    ) -> Result<Self, ParseNameError> {
        Self::new_from_str(default_cardpack_resource_name.get_string())
    }

    pub fn new_from_str(default_cardpack_resource_name: &str) -> Result<Self, ParseNameError> {
        match parse_one_token_name_to_string("defaultCardpacks/{}", default_cardpack_resource_name)
        {
            Ok(string) => Ok(Self {
                string: format!("defaultCardpacks/{}", string),
            }),
            Err(err) => Err(err),
        }
    }

    pub fn get_string(&self) -> &str {
        &self.string
    }

    pub fn clone_str(&self) -> String {
        self.string.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_parse_name() {
        let mut parsed_name_or = parse_name("", "");
        assert!(parsed_name_or.is_ok());
        assert!(parsed_name_or.unwrap().is_empty());

        parsed_name_or = parse_name("users/{}/cardpacks/{}", "users/1234/cardpacks/5678");
        assert!(parsed_name_or.is_ok());
        assert_eq!(parsed_name_or.unwrap(), vec!("1234", "5678"));

        parsed_name_or = parse_name("users/{}/settings", "users/1234/settings");
        assert!(parsed_name_or.is_ok());
        assert_eq!(parsed_name_or.unwrap(), vec!("1234"));

        parsed_name_or = parse_name("users/{}/settings", "users/1234");
        assert!(parsed_name_or.is_err());
        assert_eq!(
            parsed_name_or.err().unwrap(),
            ParseNameError::PathDoesNotMatchFormat(
                "users/{}/settings".to_string(),
                "users/1234".to_string()
            )
        );

        parsed_name_or = parse_name("users/{}", "users/1234/settings");
        assert!(parsed_name_or.is_err());
        assert_eq!(
            parsed_name_or.err().unwrap(),
            ParseNameError::PathDoesNotMatchFormat(
                "users/{}".to_string(),
                "users/1234/settings".to_string()
            )
        );

        parsed_name_or = parse_name("", "users/1234");
        assert!(parsed_name_or.is_err());
        assert_eq!(
            parsed_name_or.err().unwrap(),
            ParseNameError::PathDoesNotMatchFormat("".to_string(), "users/1234".to_string())
        );

        parsed_name_or = parse_name("users/{}", "");
        assert!(parsed_name_or.is_err());
        assert_eq!(
            parsed_name_or.err().unwrap(),
            ParseNameError::PathDoesNotMatchFormat("users/{}".to_string(), "".to_string())
        );

        parsed_name_or = parse_name("users/{}", "cardpacks/1234");
        assert!(parsed_name_or.is_err());
        assert_eq!(
            parsed_name_or.err().unwrap(),
            ParseNameError::PathDoesNotMatchFormat(
                "users/{}".to_string(),
                "cardpacks/1234".to_string()
            )
        );
    }

    #[test]
    fn test_custom_card_name() {
        custom_card_name!(TestCardName, "testCards");

        // Fails with with non-matching pattern.
        let mut test_card_name_or = TestCardName::new(&ValidatedStringField::new("users/507f191e810c19729de860ea/cardpacks/507f191e810c19729de860eb/foo/507f191e810c19729de860ec", "test_proto.test_field_name").unwrap());
        assert!(test_card_name_or.is_err());

        test_card_name_or = TestCardName::new(&ValidatedStringField::new("users/507f191e810c19729de860ea/cardpacks/507f191e810c19729de860eb/testCards/507f191e810c19729de860ec", "test_proto.test_field_name").unwrap());
        assert!(test_card_name_or.is_ok());
        let mut test_card_name = test_card_name_or.unwrap();

        assert_eq!(test_card_name.clone_str(), "users/507f191e810c19729de860ea/cardpacks/507f191e810c19729de860eb/testCards/507f191e810c19729de860ec");
        let (user_object_id, cardpack_object_id, test_card_object_id) =
            test_card_name.take_object_ids();
        assert_eq!("507f191e810c19729de860ea", user_object_id.to_string());
        assert_eq!("507f191e810c19729de860eb", cardpack_object_id.to_string());
        assert_eq!("507f191e810c19729de860ec", test_card_object_id.to_string());

        test_card_name = TestCardName::new_from_parent(
            CustomCardpackName::new_from_str(
                "users/507f191e810c19729de860ed/cardpacks/507f191e810c19729de860ee",
            )
            .unwrap(),
            ObjectId::from_str("507f191e810c19729de860ef").unwrap(),
        );
        let (user_object_id, cardpack_object_id, test_card_object_id) =
            test_card_name.take_object_ids();
        assert_eq!("507f191e810c19729de860ed", user_object_id.to_string());
        assert_eq!("507f191e810c19729de860ee", cardpack_object_id.to_string());
        assert_eq!("507f191e810c19729de860ef", test_card_object_id.to_string());
    }
}
