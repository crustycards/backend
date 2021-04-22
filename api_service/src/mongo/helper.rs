use super::super::environment::EnvironmentVariables;
use bson::doc;
use bson::oid::ObjectId;
use bson::Document;
use mongodb::{options::FindOptions, Client, Collection, Database};
use shared::proto_validation::BoundedPageSize;
use std::marker::{Send, Sync};
use tokio_stream::StreamExt;
use tonic::Status;

pub async fn get_mongo_database_or_panic(env_vars: &EnvironmentVariables) -> Database {
    let mongo_client = match Client::with_uri_str(env_vars.get_mongo_uri()).await {
        Ok(client) => client,
        _ => panic!("Failed to connect to MongoDB."),
    };

    mongo_client.database(&env_vars.get_mongo_database())
}

pub fn resource_not_found_error(resource_name: &str) -> Status {
    Status::not_found(format!(
        "Resource with name `{}` does not exist.",
        resource_name
    ))
}

pub async fn list_items<T>(
    collection: &Collection,
    mut find_doc: Document,
    projection_doc: Document,
    page_size: BoundedPageSize,
    previous_item_id_or: Option<ObjectId>,
    convert_doc_to_item: &(dyn Fn(&Document) -> T + Send + Sync),
) -> Result<(Vec<T>, Option<ObjectId>, i64), Status> {
    let page_size_i64 = page_size.take_i64();

    let matching_doc_count = match collection.count_documents(find_doc.clone(), None).await {
        Ok(count) => count,
        _ => return Err(Status::unknown("Failed to fetch cardpacks.")),
    };

    if let Some(previous_item_id) = previous_item_id_or {
        find_doc.insert("_id", doc! {"$gt": previous_item_id});
    }

    let find_options = FindOptions::builder()
        .sort(doc! {"_id": 1})
        .limit(page_size_i64 + 1)
        .projection(projection_doc)
        .build();

    let res = match collection.find(find_doc, find_options).await {
        Ok(res) => res,
        _ => return Err(Status::unknown("Failed to fetch items.")),
    };

    let mut docs: Vec<Document> = match res
        .collect::<Result<Vec<Document>, mongodb::error::Error>>()
        .await
    {
        Ok(docs) => docs,
        _ => return Err(Status::unknown("Failed to fetch items.")),
    };

    let next_item_id_or = if docs.len() > page_size_i64 as usize {
        docs.pop();
        let last_doc = match docs.last() {
            Some(last_doc) => last_doc,
            None => return Err(Status::unknown("Failed to fetch items.")),
        };
        match last_doc.get_object_id("_id") {
            Ok(object_id) => Some(object_id.clone()),
            _ => return Err(Status::unknown("Failed to fetch items.")),
        }
    } else {
        None
    };

    let items = docs.iter().map(|doc| convert_doc_to_item(doc)).collect();

    Ok((items, next_item_id_or, matching_doc_count))
}
