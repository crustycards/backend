use shared::resource_name::{CustomCardpackName, CustomWhiteCardName};
use shared::time::{object_id_to_timestamp_proto, chrono_timestamp_to_timestamp_proto};
use shared::basic_validation::ValidatedStringField;
use shared::proto_validation::BoundedPageSize;
use super::helper::*;
use shared::proto::*;
use bson::oid::ObjectId;
use bson::{doc, Document};
use mockall::automock;
use mongodb::Collection;
use prost_types::Timestamp;
use std::collections::HashMap;
use tonic::Status;

fn custom_white_card_projection_doc() -> Document {
    doc! {
      "_id": 1,
      "parentUserId": 1,
      "parentCustomCardpackId": 1,
      "text": 1,
      "updateTime": 1,
      "deleteTime": 1
    }
}

#[automock]
#[tonic::async_trait]
pub trait CustomWhiteCardCollection: Send + Sync {
    async fn create_custom_white_card(
        &self,
        parent: CustomCardpackName,
        card_text: ValidatedStringField,
    ) -> Result<CustomWhiteCard, Status>;

    async fn batch_create_custom_white_cards(
        &self,
        parent: CustomCardpackName,
        card_texts: Vec<ValidatedStringField>,
    ) -> Result<Vec<Option<CustomWhiteCard>>, Status>;

    async fn get_custom_white_card(
        &self,
        name: CustomWhiteCardName,
    ) -> Result<CustomWhiteCard, Status>;

    async fn soft_delete_custom_white_card(
        &self,
        name: CustomWhiteCardName,
    ) -> Result<CustomWhiteCard, Status>;

    async fn undelete_custom_white_card(
        &self,
        name: CustomWhiteCardName,
    ) -> Result<CustomWhiteCard, Status>;

    async fn list_custom_white_cards(
        &self,
        parent: CustomCardpackName,
        page_size: BoundedPageSize,
        last_object_id_or: Option<ObjectId>,
        show_deleted: bool,
    ) -> Result<(Vec<CustomWhiteCard>, Option<ObjectId>, i64), Status>;

    async fn update_custom_white_card(
        &self,
        name: CustomWhiteCardName,
        updated_card_text: ValidatedStringField,
    ) -> Result<CustomWhiteCard, Status>;

    // WARNING - DO NOT USE IN PROD!!!
    // Calling this method irreversable erases
    // all mongo collections related to this service.
    // This is meant to clear data between test runs.
    async fn unsafe_wipe_data_and_reset(&self) -> Result<(), mongodb::error::Error>;
}

pub struct MongoCustomWhiteCardCollection {
    collection: Collection,
}

impl MongoCustomWhiteCardCollection {
    pub fn new(collection: Collection) -> Self {
        // TODO - Setup indexes here.
        Self { collection }
    }

    fn create_new_custom_white_card_doc(
        parent: &CustomCardpackName,
        card_text: &ValidatedStringField,
    ) -> Document {
        let (parent_user_object_id, parent_custom_cardpack_object_id) = parent.get_object_ids();
        doc! {
          "parentUserId": parent_user_object_id,
          "parentCustomCardpackId": parent_custom_cardpack_object_id,
          "text": card_text.get_string(),
        }
    }
}

#[tonic::async_trait]
impl CustomWhiteCardCollection for MongoCustomWhiteCardCollection {
    async fn create_custom_white_card(
        &self,
        parent: CustomCardpackName,
        card_text: ValidatedStringField,
    ) -> Result<CustomWhiteCard, Status> {
        // TODO - Check that parent cardpack exists before creating card.
        // Right now it's possible to create a card that's owned by nobody.

        let doc: Document = Self::create_new_custom_white_card_doc(&parent, &card_text);

        let res = match self.collection.insert_one(doc, None).await {
            Ok(res) => res,
            Err(err) => return Err(Status::unknown(&format!("Failed to create card: {}.", err))),
        };

        let inserted_object_id = match res.inserted_id {
            bson::Bson::ObjectId(object_id) => object_id,
            _ => return Err(Status::unknown("Failed to create card.")),
        };

        let create_time = object_id_to_timestamp_proto(&inserted_object_id);

        Ok(CustomWhiteCard {
            name: CustomWhiteCardName::new_from_parent(parent, inserted_object_id).clone_str(),
            text: card_text.take_string(),
            create_time: Some(create_time),
            update_time: None,
            delete_time: None,
        })
    }

    async fn batch_create_custom_white_cards(
        &self,
        parent: CustomCardpackName,
        card_texts: Vec<ValidatedStringField>,
    ) -> Result<Vec<Option<CustomWhiteCard>>, Status> {
        // TODO - Check that parent cardpack exists before creating card.
        // Right now it's possible to create a card that's owned by nobody.

        // TODO - Mongodb `insertMany` is not atomic. Let's use a transaction to add this guarantee.

        if card_texts.is_empty() {
            return Ok(Vec::new());
        }

        let docs: Vec<Document> = card_texts
            .iter()
            .map(|card_text| Self::create_new_custom_white_card_doc(&parent, card_text))
            .collect();

        let res = match self.collection.insert_many(docs, None).await {
            Ok(res) => res,
            Err(err) => {
                return Err(Status::unknown(&format!(
                    "Failed to create cards: {}.",
                    err
                )))
            }
        };

        let inserted_object_ids: HashMap<usize, ObjectId> = res
            .inserted_ids
            .into_iter()
            .filter_map(|(index, bson_id)| match bson_id {
                bson::Bson::ObjectId(object_id) => Some((index, object_id)),
                _ => None,
            })
            .collect();

        let created_white_cards: Vec<Option<CustomWhiteCard>> = card_texts
            .into_iter()
            .enumerate()
            .map(|(index, card_text)| {
                let inserted_object_id = match inserted_object_ids.get(&index) {
                    Some(oid) => oid.clone(),
                    None => return None,
                };
                let create_time = object_id_to_timestamp_proto(&inserted_object_id);
                Some(CustomWhiteCard {
                    name: CustomWhiteCardName::new_from_parent(parent.clone(), inserted_object_id)
                        .clone_str(),
                    text: card_text.take_string(),
                    create_time: Some(create_time),
                    update_time: None,
                    delete_time: None,
                })
            })
            .collect();

        Ok(created_white_cards)
    }

    async fn get_custom_white_card(
        &self,
        name: CustomWhiteCardName,
    ) -> Result<CustomWhiteCard, Status> {
        let name_string = name.clone_str();
        let (user_object_id, custom_cardpack_object_id, custom_white_card_object_id) =
            name.take_object_ids();

        let options = mongodb::options::FindOneOptions::builder()
            .projection(custom_white_card_projection_doc())
            .build();

        let res = match self.collection.find_one(doc!{"_id": custom_white_card_object_id, "parentCustomCardpackId": custom_cardpack_object_id, "parentUserId": user_object_id, "deleteTime": doc!{"$exists": false}}, options).await {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to fetch card."))
        };

        let res_doc = match res {
            Some(doc) => doc,
            None => return Err(resource_not_found_error(&name_string)),
        };

        Ok(document_to_custom_white_card(&res_doc))
    }

    async fn soft_delete_custom_white_card(
        &self,
        name: CustomWhiteCardName,
    ) -> Result<CustomWhiteCard, Status> {
        let name_string = name.clone_str();
        let (user_object_id, custom_cardpack_object_id, custom_white_card_object_id) =
            name.take_object_ids();

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .projection(custom_white_card_projection_doc())
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        let res = match self.collection.find_one_and_update(doc!{"_id": custom_white_card_object_id, "parentCustomCardpackId": custom_cardpack_object_id, "parentUserId": user_object_id, "deleteTime": doc!{"$exists": false}}, doc!{"$currentDate": doc!{"deleteTime": true}}, options).await {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to delete card."))
        };

        let res_doc = match res {
            Some(doc) => doc,
            None => return Err(resource_not_found_error(&name_string)),
        };

        Ok(document_to_custom_white_card(&res_doc))
    }

    async fn undelete_custom_white_card(
        &self,
        name: CustomWhiteCardName,
    ) -> Result<CustomWhiteCard, Status> {
        let name_string = name.clone_str();
        let (user_object_id, custom_cardpack_object_id, custom_white_card_object_id) =
            name.take_object_ids();

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .projection(custom_white_card_projection_doc())
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        let res = match self.collection.find_one_and_update(doc!{"_id": custom_white_card_object_id, "parentCustomCardpackId": custom_cardpack_object_id, "parentUserId": user_object_id, "deleteTime": doc!{"$exists": true}}, doc!{"$unset": doc!{"deleteTime": ""}}, options).await {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to undelete card."))
        };

        let res_doc = match res {
            Some(doc) => doc,
            None => return Err(resource_not_found_error(&name_string)),
        };

        Ok(document_to_custom_white_card(&res_doc))
    }

    async fn list_custom_white_cards(
        &self,
        parent: CustomCardpackName,
        page_size: BoundedPageSize,
        last_object_id_or: Option<ObjectId>,
        show_deleted: bool,
    ) -> Result<(Vec<CustomWhiteCard>, Option<ObjectId>, i64), Status> {
        let (parent_user_object_id, parent_custom_cardpack_object_id) = parent.take_object_ids();
        let find_doc = doc! {"parentUserId": parent_user_object_id, "parentCustomCardpackId": parent_custom_cardpack_object_id, "deleteTime": doc!{"$exists": show_deleted}};
        list_items(
            &self.collection,
            find_doc,
            custom_white_card_projection_doc(),
            page_size,
            last_object_id_or,
            &|doc| document_to_custom_white_card(doc),
        )
        .await
    }

    async fn update_custom_white_card(
        &self,
        name: CustomWhiteCardName,
        updated_card_text: ValidatedStringField,
    ) -> Result<CustomWhiteCard, Status> {
        let name_string = name.clone_str();
        let (user_object_id, custom_cardpack_object_id, custom_white_card_object_id) =
            name.take_object_ids();

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .projection(custom_white_card_projection_doc())
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        let find_doc = doc! {
            "_id": custom_white_card_object_id,
            "parentCustomCardpackId": custom_cardpack_object_id,
            "parentUserId": user_object_id,
            "deleteTime": doc! {"$exists": false}
        };

        let update_doc = doc! {
            "$set": doc! {
                "text": updated_card_text.take_string()
            },
            "$currentDate": doc! {
                "updateTime": true
            }
        };

        let res = match self
            .collection
            .find_one_and_update(find_doc, update_doc, options)
            .await
        {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to update card.")),
        };

        let res_doc = match res {
            Some(doc) => doc,
            None => return Err(resource_not_found_error(&name_string)),
        };

        Ok(document_to_custom_white_card(&res_doc))
    }

    async fn unsafe_wipe_data_and_reset(&self) -> Result<(), mongodb::error::Error> {
        self.collection.drop(None).await
    }
}

fn document_to_custom_white_card(doc: &Document) -> CustomWhiteCard {
    CustomWhiteCard {
        name: match doc.get_object_id("_id") {
            Ok(object_id) => match doc.get_object_id("parentCustomCardpackId") {
                Ok(parent_custom_cardpack_object_id) => match doc.get_object_id("parentUserId") {
                    Ok(parent_user_object_id) => format!(
                        "users/{}/cardpacks/{}/whiteCards/{}",
                        parent_user_object_id.to_hex(),
                        parent_custom_cardpack_object_id.to_hex(),
                        object_id.to_hex()
                    ),
                    _ => String::from(""),
                },
                _ => String::from(""),
            },
            _ => String::from(""),
        },
        text: doc.get_str("text").unwrap_or("").to_string(),
        create_time: match doc.get_object_id("_id") {
            Ok(object_id) => Some(Timestamp {
                seconds: object_id.timestamp().timestamp(),
                nanos: 0,
            }),
            _ => None,
        },
        update_time: match doc.get_datetime("updateTime") {
            Ok(update_time) => Some(chrono_timestamp_to_timestamp_proto(update_time)),
            _ => None,
        },
        delete_time: match doc.get_datetime("deleteTime") {
            Ok(delete_time) => Some(chrono_timestamp_to_timestamp_proto(delete_time)),
            _ => None,
        },
    }
}
