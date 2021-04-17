use shared::resource_name::{CustomCardpackName, UserName};
use shared::time::{chrono_timestamp_to_timestamp_proto, object_id_to_timestamp_proto};
use shared::basic_validation::ValidatedStringField;
use shared::proto_validation::BoundedPageSize;
use super::helper::*;
use shared::proto::*;
use bson::oid::ObjectId;
use bson::{doc, Document};
use futures_lite::StreamExt;
use mockall::automock;
use mongodb::Collection;
use prost_types::Timestamp;
use std::collections::HashMap;
use tonic::Status;

fn custom_cardpack_projection_doc() -> Document {
    doc! {
      "_id": 1,
      "parentUserId": 1,
      "displayName": 1,
      "updateTime": 1,
      "deleteTime": 1,
    }
}

#[automock]
#[tonic::async_trait]
pub trait CustomCardpackCollection: Send + Sync {
    async fn create_custom_cardpack(
        &self,
        parent: UserName,
        display_name: ValidatedStringField,
    ) -> Result<CustomCardpack, Status>;

    async fn batch_create_custom_cardpacks(
        &self,
        parent: UserName,
        display_names: Vec<ValidatedStringField>,
    ) -> Result<Vec<Option<CustomCardpack>>, Status>;

    async fn get_custom_cardpack(&self, name: CustomCardpackName)
        -> Result<CustomCardpack, Status>;

    async fn soft_delete_custom_cardpack(
        &self,
        name: CustomCardpackName,
    ) -> Result<CustomCardpack, Status>;

    async fn undelete_custom_cardpack(
        &self,
        name: CustomCardpackName,
    ) -> Result<CustomCardpack, Status>;

    async fn list_custom_cardpacks(
        &self,
        parent: UserName,
        page_size: BoundedPageSize,
        last_object_id_or: Option<ObjectId>,
        show_deleted: bool,
    ) -> Result<(Vec<CustomCardpack>, Option<ObjectId>, i64), Status>;

    async fn update_custom_cardpack(
        &self,
        name: CustomCardpackName,
        updated_display_name: ValidatedStringField,
    ) -> Result<CustomCardpack, Status>;

    // Converts a list of cardpack names into a list of cardpack proto structs.
    // The return vec is guaranteed to be the same length as the input,
    // and have its items in the same order. Any cardpacks that weren't found
    // will contain `None` in the given index.
    async fn get_cardpacks_from_names(
        &self,
        cardpack_names: Vec<CustomCardpackName>,
    ) -> Result<Vec<Option<CustomCardpack>>, mongodb::error::Error>;

    // WARNING - DO NOT USE IN PROD!!!
    // Calling this method irreversable erases
    // all mongo collections related to this service.
    // This is meant to clear data between test runs.
    async fn unsafe_wipe_data_and_reset(&self) -> Result<(), mongodb::error::Error>;
}

pub struct MongoCustomCardpackCollection {
    collection: Collection,
}

impl MongoCustomCardpackCollection {
    pub fn new(collection: Collection) -> Self {
        // TODO - Setup indexes here.
        Self { collection }
    }

    fn create_new_custom_cardpack_doc(
        parent: &UserName,
        display_name: &ValidatedStringField,
    ) -> Document {
        doc! {
          "parentUserId": parent.get_object_id(),
          "displayName": display_name.get_string(),
        }
    }
}

#[tonic::async_trait]
impl CustomCardpackCollection for MongoCustomCardpackCollection {
    async fn create_custom_cardpack(
        &self,
        parent: UserName,
        display_name: ValidatedStringField,
    ) -> Result<CustomCardpack, Status> {
        // TODO - Check that parent user exists before creating cardpack.
        // Right now it's possible to create a cardpack that's owned by nobody.

        let doc: Document = Self::create_new_custom_cardpack_doc(&parent, &display_name);

        let res = match self.collection.insert_one(doc, None).await {
            Ok(res) => res,
            Err(err) => {
                return Err(Status::unknown(&format!(
                    "Failed to create cardpack: {}.",
                    err
                )))
            }
        };

        let inserted_object_id = match res.inserted_id {
            bson::Bson::ObjectId(object_id) => object_id,
            _ => return Err(Status::unknown("Failed to create cardpack.")),
        };

        let create_time = object_id_to_timestamp_proto(&inserted_object_id);

        Ok(CustomCardpack {
            name: CustomCardpackName::new_from_parent(parent, inserted_object_id).clone_str(),
            display_name: display_name.take_string(),
            create_time: Some(create_time),
            update_time: None,
            delete_time: None,
        })
    }

    async fn batch_create_custom_cardpacks(
        &self,
        parent: UserName,
        display_names: Vec<ValidatedStringField>,
    ) -> Result<Vec<Option<CustomCardpack>>, Status> {
        // TODO - Check that parent user exists before creating cardpack.
        // Right now it's possible to create a cardpack that's owned by nobody.

        // TODO - Mongodb `insertMany` is not atomic. Let's use a transaction to add this guarantee.

        if display_names.is_empty() {
            return Ok(Vec::new());
        }

        let docs: Vec<Document> = display_names
            .iter()
            .map(|display_name| Self::create_new_custom_cardpack_doc(&parent, display_name))
            .collect();

        let res = match self.collection.insert_many(docs, None).await {
            Ok(res) => res,
            Err(err) => {
                return Err(Status::unknown(&format!(
                    "Failed to create cardpacks: {}.",
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

        let created_cardpacks: Vec<Option<CustomCardpack>> = display_names
            .into_iter()
            .enumerate()
            .map(|(index, display_name)| {
                let inserted_object_id = match inserted_object_ids.get(&index) {
                    Some(oid) => oid.clone(),
                    None => return None,
                };
                let create_time = object_id_to_timestamp_proto(&inserted_object_id);
                Some(CustomCardpack {
                    name: CustomCardpackName::new_from_parent(parent.clone(), inserted_object_id)
                        .clone_str(),
                    display_name: display_name.take_string(),
                    create_time: Some(create_time),
                    update_time: None,
                    delete_time: None,
                })
            })
            .collect();

        Ok(created_cardpacks)
    }

    async fn get_custom_cardpack(
        &self,
        name: CustomCardpackName,
    ) -> Result<CustomCardpack, Status> {
        let name_string = name.clone_str();
        let (user_object_id, custom_cardpack_object_id) = name.take_object_ids();

        let options = mongodb::options::FindOneOptions::builder()
            .projection(custom_cardpack_projection_doc())
            .build();

        let res = match self.collection.find_one(doc!{"_id": custom_cardpack_object_id, "parentUserId": user_object_id, "deleteTime": doc!{"$exists": false}}, options).await {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to fetch cardpack."))
        };

        let res_doc = match res {
            Some(doc) => doc,
            None => return Err(resource_not_found_error(&name_string)),
        };

        Ok(document_to_custom_cardpack(&res_doc))
    }

    async fn soft_delete_custom_cardpack(
        &self,
        name: CustomCardpackName,
    ) -> Result<CustomCardpack, Status> {
        let name_string = name.clone_str();
        let (user_object_id, custom_cardpack_object_id) = name.take_object_ids();

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .projection(custom_cardpack_projection_doc())
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        let res = match self.collection.find_one_and_update(doc!{"_id": custom_cardpack_object_id, "parentUserId": user_object_id, "deleteTime": doc!{"$exists": false}}, doc!{"$currentDate": doc!{"deleteTime": true}}, options).await {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to delete cardpack."))
        };

        let res_doc = match res {
            Some(doc) => doc,
            None => return Err(resource_not_found_error(&name_string)),
        };

        Ok(document_to_custom_cardpack(&res_doc))
    }

    async fn undelete_custom_cardpack(
        &self,
        name: CustomCardpackName,
    ) -> Result<CustomCardpack, Status> {
        let name_string = name.clone_str();
        let (user_object_id, custom_cardpack_object_id) = name.take_object_ids();

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .projection(custom_cardpack_projection_doc())
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        let res = match self.collection.find_one_and_update(doc!{"_id": custom_cardpack_object_id, "parentUserId": user_object_id, "deleteTime": doc!{"$exists": true}}, doc!{"$unset": doc!{"deleteTime": ""}}, options).await {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to undelete cardpack."))
        };

        let res_doc = match res {
            Some(doc) => doc,
            None => return Err(resource_not_found_error(&name_string)),
        };

        Ok(document_to_custom_cardpack(&res_doc))
    }

    async fn list_custom_cardpacks(
        &self,
        parent: UserName,
        page_size: BoundedPageSize,
        last_object_id_or: Option<ObjectId>,
        show_deleted: bool,
    ) -> Result<(Vec<CustomCardpack>, Option<ObjectId>, i64), Status> {
        let parent_user_object_id = parent.take_object_id();
        let find_doc = doc! {"parentUserId": parent_user_object_id, "deleteTime": doc!{"$exists": show_deleted}};
        list_items(
            &self.collection,
            find_doc,
            custom_cardpack_projection_doc(),
            page_size,
            last_object_id_or,
            &|doc| document_to_custom_cardpack(doc),
        )
        .await
    }

    async fn update_custom_cardpack(
        &self,
        name: CustomCardpackName,
        updated_display_name: ValidatedStringField,
    ) -> Result<CustomCardpack, Status> {
        let name_string = name.clone_str();
        let (user_object_id, custom_cardpack_object_id) = name.take_object_ids();

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .projection(custom_cardpack_projection_doc())
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        let find_doc = doc! {
            "_id": custom_cardpack_object_id,
            "parentUserId": user_object_id,
            "deleteTime": doc! {"$exists": false}
        };

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
            .find_one_and_update(find_doc, update_doc, options)
            .await
        {
            Ok(res) => res,
            _ => return Err(Status::unknown("Failed to update cardpack.")),
        };

        let res_doc = match res {
            Some(doc) => doc,
            None => return Err(resource_not_found_error(&name_string)),
        };

        Ok(document_to_custom_cardpack(&res_doc))
    }

    async fn get_cardpacks_from_names(
        &self,
        names: Vec<CustomCardpackName>,
    ) -> Result<Vec<Option<CustomCardpack>>, mongodb::error::Error> {
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let options = mongodb::options::FindOptions::builder()
            .projection(custom_cardpack_projection_doc())
            .build();
        let res = match self.collection.find(doc!{"_id": doc!{"$in": names.clone().into_iter().map(|name| name.take_object_ids().1).collect::<Vec<bson::oid::ObjectId>>()}}, options).await {
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
        let custom_cardpacks: Vec<CustomCardpack> = docs
            .iter()
            .map(|doc| document_to_custom_cardpack(doc))
            .collect();
        let custom_cardpacks_map: HashMap<CustomCardpackName, CustomCardpack> = custom_cardpacks
            .into_iter()
            .filter_map(|custom_cardpack| {
                match CustomCardpackName::new_from_str(&custom_cardpack.name) {
                    Ok(custom_cardpack_name) => Some((custom_cardpack_name, custom_cardpack)),
                    _ => None,
                }
            })
            .collect();
        Ok(names
            .iter()
            .map(
                |custom_cardpack_name| match custom_cardpacks_map.get(custom_cardpack_name) {
                    Some(custom_cardpack) => Some(custom_cardpack.clone()),
                    None => None,
                },
            )
            .collect())
    }

    async fn unsafe_wipe_data_and_reset(&self) -> Result<(), mongodb::error::Error> {
        self.collection.drop(None).await
    }
}

fn document_to_custom_cardpack(doc: &Document) -> CustomCardpack {
    CustomCardpack {
        name: match doc.get_object_id("_id") {
            Ok(object_id) => match doc.get_object_id("parentUserId") {
                Ok(parent_user_object_id) => format!(
                    "users/{}/cardpacks/{}",
                    parent_user_object_id.to_hex(),
                    object_id.to_hex()
                ),
                _ => String::from(""),
            },
            _ => String::from(""),
        },
        display_name: doc.get_str("displayName").unwrap_or("").to_string(),
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
