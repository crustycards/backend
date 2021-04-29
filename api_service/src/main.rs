mod environment;
mod mongo;
mod proto_helper;
mod search_client;
mod service;

use mongo::custom_black_card_collection::MongoCustomBlackCardCollection;
use mongo::custom_cardpack_collection::MongoCustomCardpackCollection;
use mongo::custom_white_card_collection::MongoCustomWhiteCardCollection;
use mongo::helper::get_mongo_database_or_panic;
use mongo::user_collection::MongoUserCollection;
use search_client::SonicSearchClient;
use service::admin_service_impl::AdminServiceImpl;
use service::cardpack_service_impl::CardpackServiceImpl;
use service::default_cardpacks::DefaultCardpackHandler;
use service::user_service_impl::UserServiceImpl;
use shared::proto::crusty_cards_api::{
    admin_service_server::AdminServiceServer, cardpack_service_server::CardpackServiceServer,
    user_service_server::UserServiceServer,
};
use std::sync::Arc;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env_vars = environment::EnvironmentVariables::new();
    let port: u16 = 50052;
    let address = format!("0.0.0.0:{}", port).parse().unwrap();

    let mongo_database = get_mongo_database_or_panic(&env_vars).await;
    let user_collection = Arc::from(MongoUserCollection::new(mongo_database.collection("users")));
    let sonic_client = Arc::from(SonicSearchClient::new(&env_vars)?);

    println!("Starting server on port {}", port);
    Server::builder()
        .add_service(AdminServiceServer::new(AdminServiceImpl::new(
            user_collection.clone(),
            sonic_client.clone(),
        )))
        .add_service(UserServiceServer::new(UserServiceImpl::new(
            user_collection.clone(),
            sonic_client,
        )))
        .add_service(CardpackServiceServer::new(CardpackServiceImpl::new(
            Box::from(MongoCustomCardpackCollection::new(
                mongo_database.collection("cardpacks"),
            )),
            Box::from(MongoCustomBlackCardCollection::new(
                mongo_database.collection("blackCards"),
            )),
            Box::from(MongoCustomWhiteCardCollection::new(
                mongo_database.collection("whiteCards"),
            )),
            DefaultCardpackHandler::new_with_hardcoded_packs(),
            user_collection,
        )))
        .serve(address)
        .await?;
    Ok(())
}
