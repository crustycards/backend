mod amqp;
mod environment;
mod game;
mod helper;
mod service;

use amqp::MessageQueue;
use service::api_resource_fetcher::GrpcApiResourceFetcher;
use service::game_service_impl::GameServiceImpl;
use shared::proto::cardpack_service_client::CardpackServiceClient;
use shared::proto::game_service_server::GameServiceServer;
use shared::proto::user_service_client::UserServiceClient;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env_vars = environment::EnvironmentVariables::new();
    let port: u16 = 50052;
    let address = format!("0.0.0.0:{}", port).parse().unwrap();

    let cardpack_service =
        CardpackServiceClient::connect(String::from(env_vars.get_api_uri())).await?;
    let user_service = UserServiceClient::connect(String::from(env_vars.get_api_uri())).await?;
    let message_queue = MessageQueue::new(env_vars.get_amqp_uri());

    println!("Starting server on port {}", port);
    Server::builder()
        .add_service(GameServiceServer::new(GameServiceImpl::new(
            Box::from(GrpcApiResourceFetcher::new(cardpack_service, user_service)),
            Some(message_queue),
        )))
        .serve(address)
        .await?;
    Ok(())
}
