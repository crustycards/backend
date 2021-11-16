use super::helper::*;
use bson::{doc, document::ValueAccessError, Bson, Document};
use futures_lite::{Stream, StreamExt};
use mockall::automock;
use mongodb::Collection;
use shared::basic_validation::ValidatedStringField;
use shared::proto::crusty_cards_api::game_config::{
    blank_white_card_config::BlankWhiteCardsAdded, BlankWhiteCardConfig, EndCondition,
};
use shared::proto::crusty_cards_api::*;
use shared::proto::google::protobuf::Empty;
use shared::proto::google::protobuf::Timestamp;
use shared::proto_validation::{
    BoundedPageSize, OptionalField, ValidatedColorScheme, ValidatedGameConfig,
    ValidatedOAuthCredentials,
};
use shared::resource_name::{CustomCardpackName, UserName, UserSettingsName};
use shared::time::chrono_timestamp_to_timestamp_proto;
use std::collections::HashMap;
use std::marker::Unpin;
use tonic::Status;

fn user_projection_doc() -> Document {
    doc! {
      "_id": 1,
      "displayName": 1,
      "updateTime": 1
    }
}

fn user_settings_projection_doc() -> Document {
    doc! {
      "_id": 1,
      "settings": 1
    }
}

type UserStream = Box<dyn Stream<Item = Result<User, mongodb::error::Error>> + Send + Unpin>;

#[automock]
#[tonic::async_trait]
pub trait UserCollection: Send + Sync {
    async fn get_user(&self, name: UserName) -> Result<User, Status>;

    async fn update_user(
        &self,
        name: UserName,
        updated_display_name: ValidatedStringField,
    ) -> Result<User, Status>;

    async fn get_user_settings(&self, name: UserSettingsName) -> Result<UserSettings, Status>;

    async fn update_user_settings(
        &self,
        name: UserSettingsName,
        color_scheme_or: Option<ValidatedColorScheme>,
        quick_start_game_config_or: Option<OptionalField<ValidatedGameConfig>>,
    ) -> Result<UserSettings, Status>;

    async fn get_or_create_user(
        &self,
        validated_oauth_credentials: ValidatedOAuthCredentials,
        display_name: ValidatedStringField,
    ) -> Result<User, Status>;

    async fn assert_user_exists(&self, name: UserName) -> Result<(), Status>;

    // Converts a list of user names into a list of user proto structs.
    // The return vec is guaranteed to be the same length as the input,
    // and have its items in the same order. Any users that weren't found
    // will contain `None` in the given index.
    async fn get_users_from_names(
        &self,
        user_names: Vec<UserName>,
    ) -> Result<Vec<Option<User>>, mongodb::error::Error>;

    async fn add_custom_cardpack_to_favorites(
        &self,
        user_name: UserName,
        custom_cardpack_name: CustomCardpackName,
    ) -> Result<(), Status>;

    async fn remove_custom_cardpack_from_favorites(
        &self,
        user_name: UserName,
        custom_cardpack_name: CustomCardpackName,
    ) -> Result<(), Status>;

    async fn check_has_user_favorited_custom_cardpack(
        &self,
        user_name: UserName,
        custom_cardpack_name: CustomCardpackName,
    ) -> Result<bool, Status>;

    async fn list_favorited_custom_cardpack_names(
        &self,
        user_name: UserName,
        page_size: BoundedPageSize,
        start_index: usize,
    ) -> Result<(Vec<CustomCardpackName>, Option<usize>), Status>;

    // Streams all existing users.
    async fn user_stream(&self) -> Result<UserStream, mongodb::error::Error>;

    // WARNING - DO NOT USE IN PROD!!!
    // Calling this method irreversable erases
    // all mongo collections related to this service.
    // This is meant to clear data between test runs.
    async fn unsafe_wipe_data_and_reset(&self) -> Result<(), mongodb::error::Error>;
}

pub struct MongoUserCollection {
    collection: Collection<Document>,
}

impl MongoUserCollection {
    pub fn new(collection: Collection<Document>) -> Self {
        // TODO - Setup indexes here.
        Self { collection }
    }
}

#[tonic::async_trait]
impl UserCollection for MongoUserCollection {
    async fn get_user(&self, name: UserName) -> Result<User, Status> {
        let name_string = name.clone_str();
        let user_object_id = name.take_object_id();

        let options = mongodb::options::FindOneOptions::builder()
            .projection(user_projection_doc())
            .build();

        let res = match self
            .collection
            .find_one(doc! {"_id": user_object_id}, options)
            .await
        {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to fetch user.")),
        };

        let res_doc = match res {
            Some(doc) => doc,
            None => return Err(resource_not_found_error(&name_string)),
        };

        Ok(document_to_user(&res_doc))
    }

    async fn update_user(
        &self,
        name: UserName,
        updated_display_name: ValidatedStringField,
    ) -> Result<User, Status> {
        let name_string = name.clone_str();
        let user_object_id = name.take_object_id();

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .projection(user_projection_doc())
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        let update_doc = doc! {
            "$set": doc! {
                "displayName": updated_display_name.take_string()
            },
            "$currentDate": doc! {
                "updateTime": true
            }
        };

        let res = match self
            .collection
            .find_one_and_update(doc! { "_id": user_object_id }, update_doc, options)
            .await
        {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to update user.")),
        };

        let res_doc = match res {
            Some(doc) => doc,
            None => return Err(resource_not_found_error(&name_string)),
        };

        Ok(document_to_user(&res_doc))
    }

    async fn get_user_settings(&self, name: UserSettingsName) -> Result<UserSettings, Status> {
        let name_string = name.clone_str();
        let user_object_id = name.take_object_id();

        let options = mongodb::options::FindOneOptions::builder()
            .projection(user_settings_projection_doc())
            .build();

        let res = match self
            .collection
            .find_one(doc! {"_id": user_object_id}, options)
            .await
        {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to fetch user settings.")),
        };

        let res_doc = match res {
            Some(doc) => doc,
            None => return Err(resource_not_found_error(&name_string)),
        };

        Ok(document_to_user_settings(&res_doc))
    }

    async fn update_user_settings(
        &self,
        name: UserSettingsName,
        color_scheme_or: Option<ValidatedColorScheme>,
        quick_start_game_config_or: Option<OptionalField<ValidatedGameConfig>>,
    ) -> Result<UserSettings, Status> {
        if color_scheme_or.is_none() && quick_start_game_config_or.is_none() {
            return self.get_user_settings(name).await;
        }

        let name_string = name.clone_str();
        let user_object_id = name.take_object_id();

        let mut set_doc = doc! {};
        let mut unset_doc = doc! {};

        if let Some(color_scheme) = color_scheme_or {
            set_doc.insert("settings.colorScheme", color_scheme as i32);
        }

        if let Some(optional_quick_start_game_config) = quick_start_game_config_or {
            match optional_quick_start_game_config {
                OptionalField::Set(quick_start_game_config) => {
                    set_doc.insert(
                        "settings.quickStartGameConfig",
                        bson::Bson::Document(game_config_to_document(
                            &quick_start_game_config.raw_config(),
                        )),
                    );
                }
                OptionalField::Unset => {
                    unset_doc.insert("settings.quickStartGameConfig", "");
                }
            }
        }

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .projection(user_settings_projection_doc())
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        let mut update_doc = doc! {};
        if !set_doc.is_empty() {
            update_doc.insert("$set", set_doc);
        }
        if !unset_doc.is_empty() {
            update_doc.insert("$unset", unset_doc);
        }

        let res = match self
            .collection
            .find_one_and_update(doc! {"_id": user_object_id}, update_doc, options)
            .await
        {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to update user settings.")),
        };

        let res_doc = match res {
            Some(doc) => doc,
            None => return Err(resource_not_found_error(&name_string)),
        };

        Ok(document_to_user_settings(&res_doc))
    }

    async fn get_or_create_user(
        &self,
        validated_oauth_credentials: ValidatedOAuthCredentials,
        display_name: ValidatedStringField,
    ) -> Result<User, Status> {
        let oauth_credentials = validated_oauth_credentials.take_oauth_credentials();

        let filter_doc = doc! {
          "oauthId": oauth_credentials.oauth_id.clone(),
          "oauthProvider": oauth_credentials.oauth_provider.clone()
        };

        let replacement_doc = doc! {
          "$setOnInsert": doc!{
            "oauthId": oauth_credentials.oauth_id.clone(),
            "oauthProvider": oauth_credentials.oauth_provider.clone(),
            "displayName": display_name.take_string(),
            "settings": doc!{
              "colorScheme": ValidatedColorScheme::DefaultLight as i32
            }
          }
        };

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .upsert(true)
            .projection(user_projection_doc())
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        let res = match self
            .collection
            .find_one_and_update(filter_doc, replacement_doc, options)
            .await
        {
            Ok(res) => res,
            Err(err) => {
                return Err(Status::unknown(&format!(
                    "Failed to create/fetch user: {}.",
                    err
                )))
            }
        };

        let res_doc = match res {
            Some(res_doc) => res_doc,
            None => {
                // This should never happen since we're doing an upsert.
                return Err(Status::unknown("Failed to create/fetch user."));
            }
        };

        Ok(document_to_user(&res_doc))
    }

    async fn assert_user_exists(&self, name: UserName) -> Result<(), Status> {
        let user_object_id = name.take_object_id();

        let options = mongodb::options::FindOneOptions::builder()
            .projection(doc! { "_id": 1 })
            .build();

        let res = match self
            .collection
            .find_one(doc! {"_id": user_object_id}, options)
            .await
        {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to check if user exists.")),
        };

        if res.is_none() {
            return Err(Status::not_found("User does not exist."));
        } else {
            return Ok(());
        }
    }

    async fn get_users_from_names(
        &self,
        names: Vec<UserName>,
    ) -> Result<Vec<Option<User>>, mongodb::error::Error> {
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let options = mongodb::options::FindOptions::builder()
            .projection(user_projection_doc())
            .build();
        let res = match self.collection.find(doc!{"_id": doc!{"$in": names.clone().into_iter().map(|name| name.take_object_id()).collect::<Vec<bson::oid::ObjectId>>()}}, options).await {
            Ok(res) => res,
            Err(err) => return Err(err)
        };
        let docs: Vec<Document> = res
            .collect::<Vec<Result<Document, mongodb::error::Error>>>()
            .await
            .into_iter()
            .filter_map(|item_or| match item_or {
                Ok(item) => Some(item),
                _ => None,
            })
            .collect();
        let users_map: HashMap<UserName, User> = docs
            .iter()
            .map(|doc| document_to_user(doc))
            .into_iter()
            .filter_map(|user| match UserName::new_from_str(&user.name) {
                Ok(user_name) => Some((user_name, user)),
                _ => None,
            })
            .collect();
        Ok(names
            .iter()
            .map(|user_name| users_map.get(user_name).cloned())
            .collect())
    }

    async fn add_custom_cardpack_to_favorites(
        &self,
        user_name: UserName,
        custom_cardpack_name: CustomCardpackName,
    ) -> Result<(), Status> {
        let res = match self.collection.update_one(doc!{"_id": user_name.take_object_id()}, doc!{"$addToSet": {"favoritedCardpackIds": custom_cardpack_name.take_object_ids().1}}, None).await {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to add custom cardpack to favorites.")),
        };
        if res.modified_count == 0 {
            return Err(Status::unknown(
                "Failed to add custom cardpack to favorites.",
            ));
        }
        Ok(())
    }

    async fn remove_custom_cardpack_from_favorites(
        &self,
        user_name: UserName,
        custom_cardpack_name: CustomCardpackName,
    ) -> Result<(), Status> {
        let res = match self
            .collection
            .update_one(
                doc! {"_id": user_name.take_object_id()},
                doc! {"$pull": {"favoritedCardpackIds": custom_cardpack_name.take_object_ids().1}},
                None,
            )
            .await
        {
            Ok(res) => res,
            _ => {
                return Err(Status::unknown(
                    "Failed to remove custom cardpack from favorites.",
                ))
            }
        };
        if res.modified_count == 0 {
            return Err(Status::unknown(
                "Failed to remove custom cardpack from favorites.",
            ));
        }
        Ok(())
    }

    async fn check_has_user_favorited_custom_cardpack(
        &self,
        user_name: UserName,
        custom_cardpack_name: CustomCardpackName,
    ) -> Result<bool, Status> {
        let count = match self.collection.count_documents(doc!{"_id": user_name.take_object_id(), "favoritedCardpackIds": doc!{"$elemMatch": doc!{"$eq": custom_cardpack_name.take_object_ids().1}}}, None).await {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to load favorited custom cardpacks.")),
        };
        Ok(count > 0)
    }

    async fn list_favorited_custom_cardpack_names(
        &self,
        user_name: UserName,
        page_size: BoundedPageSize,
        start_index: usize,
    ) -> Result<(Vec<CustomCardpackName>, Option<usize>), Status> {
        let page_size_i64 = page_size.take_i64();
        let options = mongodb::options::FindOneOptions::builder()
            .projection(doc!{"favoritedCardpackIds": doc!{"$slice": vec!{start_index as i32, (page_size_i64 + 1) as i32}}})
            .build();
        let res = match self
            .collection
            .find_one(doc! {"_id": user_name.clone().take_object_id()}, options)
            .await
        {
            Ok(res) => res,
            _ => {
                return Err(Status::unknown(
                    "Failed to remove custom cardpack from favorites.",
                ))
            }
        };
        let doc = match res {
            Some(doc) => doc,
            None => return Err(Status::not_found("User does not exist.")),
        };
        let mut bson_list = match doc.get_array("favoritedCardpackIds") {
            Ok(array) => array.clone(),
            Err(err) => match err {
                ValueAccessError::NotPresent => Vec::new(),
                ValueAccessError::UnexpectedType => {
                    return Err(Status::unknown(
                        "Field `favoritedCardpackIds` was unexpected type.",
                    ))
                }
                _ => return Err(Status::unknown("Could not parse favorited cardpack ids.")),
            },
        };
        let next_index_or = if bson_list.len() == (page_size_i64 + 1) as usize {
            bson_list.pop();
            Some(start_index + page_size_i64 as usize)
        } else {
            None
        };
        let mut custom_cardpack_names = Vec::new();
        for bson in bson_list {
            match bson {
                Bson::ObjectId(object_id) => custom_cardpack_names.push(
                    CustomCardpackName::new_from_parent(user_name.clone(), object_id),
                ),
                _ => {
                    return Err(Status::unknown(
                        "Array field `favoritedCardpackIds` contained an unexpected type.",
                    ))
                }
            };
        }
        Ok((custom_cardpack_names, next_index_or))
    }

    async fn user_stream(&self) -> Result<UserStream, mongodb::error::Error> {
        let options = mongodb::options::FindOptions::builder()
            .projection(user_projection_doc())
            .build();
        Ok(Box::from(
            self.collection
                .find(None, options)
                .await?
                .map(|doc_or| Ok(document_to_user(&doc_or?))),
        ))
    }

    async fn unsafe_wipe_data_and_reset(&self) -> Result<(), mongodb::error::Error> {
        self.collection.drop(None).await
    }
}

fn document_to_user(doc: &Document) -> User {
    User {
        name: match doc.get_object_id("_id") {
            Ok(object_id) => format!("users/{}", object_id.to_hex()),
            _ => String::from(""),
        },
        display_name: String::from(doc.get_str("displayName").unwrap_or("")),
        create_time: match doc.get_object_id("_id") {
            Ok(object_id) => Some(Timestamp {
                seconds: object_id.timestamp().to_chrono().timestamp(),
                nanos: 0,
            }),
            _ => None,
        },
        update_time: match doc.get_datetime("updateTime") {
            Ok(update_time) => Some(chrono_timestamp_to_timestamp_proto(
                &update_time.to_chrono(),
            )),
            _ => None,
        },
    }
}

fn game_config_to_document(game_config: &GameConfig) -> Document {
    let mut doc = Document::new();

    if !game_config.display_name.is_empty() {
        doc.insert("displayName", game_config.display_name.clone());
    }

    if game_config.max_players != 0 {
        doc.insert("maxPlayers", game_config.max_players);
    }

    match &game_config.end_condition {
        Some(end_condition) => {
            match end_condition {
                EndCondition::MaxScore(max_score) => {
                    if max_score != &0 {
                        doc.insert("maxScore", max_score);
                    }
                }
                EndCondition::EndlessMode(_) => {
                    doc.insert("endlessMode", true);
                }
            };
        }
        None => {}
    };

    if game_config.hand_size != 0 {
        doc.insert("handSize", game_config.hand_size);
    }

    if !game_config.custom_cardpack_names.is_empty() {
        doc.insert(
            "customCardpackNames",
            game_config.custom_cardpack_names.clone(),
        );
    }

    if !game_config.default_cardpack_names.is_empty() {
        doc.insert(
            "defaultCardpackNames",
            game_config.default_cardpack_names.clone(),
        );
    }

    match &game_config.blank_white_card_config {
        Some(blank_white_card_config) => {
            doc.insert(
                "blankWhiteCardConfig",
                blank_white_card_config_to_document(blank_white_card_config),
            );
        }
        None => {}
    };

    doc
}

fn blank_white_card_config_to_document(blank_white_card_config: &BlankWhiteCardConfig) -> Document {
    let mut doc = Document::new();

    if blank_white_card_config.behavior != 0 {
        doc.insert("behavior", blank_white_card_config.behavior);
    }

    match &blank_white_card_config.blank_white_cards_added {
        Some(blank_white_cards_added) => {
            match blank_white_cards_added {
                BlankWhiteCardsAdded::CardCount(card_count) => {
                    if card_count != &0 {
                        doc.insert("cardCount", card_count);
                    }
                }
                BlankWhiteCardsAdded::Percentage(percentage) => {
                    if percentage.abs() > 0.0 {
                        doc.insert("percentage", percentage);
                    }
                }
            };
        }
        None => {}
    };

    doc
}

fn document_to_user_settings(doc: &Document) -> UserSettings {
    let name = match doc.get_object_id("_id") {
        Ok(object_id) => format!("users/{}/settings", object_id.to_hex()),
        _ => String::from(""),
    };

    let settings_doc = match doc.get_document("settings") {
        Ok(doc) => doc,
        _ => {
            return UserSettings {
                name,
                color_scheme: 0,
                quick_start_game_config: None,
            }
        }
    };

    UserSettings {
        name,
        color_scheme: match settings_doc.get_i32("colorScheme") {
            Ok(color_scheme_i32) => color_scheme_i32,
            _ => 0,
        },
        quick_start_game_config: match settings_doc.get_document("quickStartGameConfig") {
            Ok(quick_start_game_config) => Some(document_to_game_config(quick_start_game_config)),
            _ => None,
        },
    }
}

fn document_to_game_config(doc: &Document) -> GameConfig {
    let mut end_condition = None;

    if let Ok(max_score) = doc.get_i32("maxScore") {
        end_condition = Some(EndCondition::MaxScore(max_score));
    }

    if doc.get_bool("endlessMode").is_ok() {
        end_condition = Some(EndCondition::EndlessMode(Empty {}));
    }

    GameConfig {
        display_name: String::from(doc.get_str("displayName").unwrap_or("")),
        max_players: doc.get_i32("maxPlayers").unwrap_or(0),
        end_condition,
        hand_size: doc.get_i32("handSize").unwrap_or(0),
        custom_cardpack_names: {
            let mut custom_cardpack_names = Vec::new();
            if let Ok(bson_custom_cardpack_names) = doc.get_array("customCardpackNames") {
                for bson_name in bson_custom_cardpack_names {
                    if let bson::Bson::String(name) = bson_name {
                        custom_cardpack_names.push(String::from(name))
                    }
                }
            }
            custom_cardpack_names
        },
        default_cardpack_names: {
            let mut default_cardpack_names = Vec::new();
            if let Ok(bson_default_cardpack_names) = doc.get_array("defaultCardpackNames") {
                for bson_name in bson_default_cardpack_names {
                    if let bson::Bson::String(name) = bson_name {
                        default_cardpack_names.push(String::from(name))
                    }
                }
            }
            default_cardpack_names
        },
        blank_white_card_config: match doc.get_document("blankWhiteCardConfig") {
            Ok(blank_white_card_config_doc) => Some(document_to_blank_white_card_config(
                blank_white_card_config_doc,
            )),
            _ => None,
        },
    }
}

fn document_to_blank_white_card_config(doc: &Document) -> BlankWhiteCardConfig {
    let mut blank_white_cards_added = None;

    if let Ok(card_count) = doc.get_i32("cardCount") {
        blank_white_cards_added = Some(BlankWhiteCardsAdded::CardCount(card_count))
    }

    if let Ok(percentage) = doc.get_f64("percentage") {
        blank_white_cards_added = Some(BlankWhiteCardsAdded::Percentage(percentage))
    }

    BlankWhiteCardConfig {
        behavior: doc.get_i32("behavior").unwrap_or(0),
        blank_white_cards_added,
    }
}

// TODO - Uncomment this and fix the test. Also, test the rest of this collection.
// #[cfg(test)]
// mod tests {
//     use super::super::super::search_client::MockSearchClient;
//     use super::*;

//     #[tokio::test]
//     async fn get_users_from_names() {
//         let user_service = get_local_test_user_service(Some(Box::from(
//             |mock_search_client: &mut MockSearchClient| {
//                 mock_search_client
//                     .expect_index_user()
//                     .times(1)
//                     .returning(|_, _| Ok(()));
//             },
//         )))
//         .await;

//         assert_eq!(
//             vec! {None},
//             user_service
//                 .get_users_from_names(
//                     vec! {UserName::new("users/507f1f77bcf86cd799439011").unwrap()}
//                 )
//                 .await
//                 .unwrap()
//         );

//         let oauth_credentials = OAuthCredentials {
//             oauth_provider: String::from("google"),
//             oauth_id: String::from("1234"),
//         };
//         let mut user = User {
//             name: String::from(""),
//             display_name: String::from("Tommy"),
//             create_time: None,
//             update_time: None,
//         };
//         let get_or_create_user_request = GetOrCreateUserRequest {
//             oauth_credentials: Some(oauth_credentials),
//             user: Some(user),
//         };
//         user = user_service
//             .get_or_create_user(Request::new(get_or_create_user_request))
//             .await
//             .unwrap()
//             .into_inner();

//         assert_eq!(
//             vec! {None, Some(user.clone()), None},
//             user_service
//                 .get_users_from_names(vec! {
//                     UserName::new("users/507f1f77bcf86cd799439011").unwrap(),
//                     UserName::new(&user.name).unwrap(),
//                     UserName::new("users/507f1f77bcf86cd799439012").unwrap()
//                 })
//                 .await
//                 .unwrap()
//         );
//     }
// }
