use super::super::mongo::user_collection::UserCollection;
use super::super::search_client::{IndexUserError, SearchClient};
use futures_lite::StreamExt;
use shared::proto::crusty_cards_api::admin_service_server::AdminService;
use shared::proto::google::protobuf::Empty;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct AdminServiceImpl {
    user_collection: Arc<dyn UserCollection>,
    search_client: Arc<dyn SearchClient>,
}

impl AdminServiceImpl {
    pub fn new(
        user_collection: Arc<dyn UserCollection>,
        search_client: Arc<dyn SearchClient>,
    ) -> Self {
        Self {
            user_collection,
            search_client,
        }
    }

    fn sonic_error_to_unknown_status(error: sonic_channel::result::Error) -> Status {
        Status::unknown(format!("Unknown error from Sonic: {}.", error))
    }

    fn mongodb_error_to_unknown_status(error: mongodb::error::Error) -> Status {
        Status::unknown(format!("Unknown error from Mongodb: {}.", error))
    }
}

#[tonic::async_trait]
impl AdminService for AdminServiceImpl {
    async fn clear_user_search_index(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Empty>, Status> {
        match self.search_client.wipe_user_index() {
            Ok(_) => Ok(Response::new(Empty {})),
            Err(err) => Err(Self::sonic_error_to_unknown_status(err)),
        }
    }

    async fn refresh_user_search_index(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Empty>, Status> {
        let mut user_stream = match self.user_collection.user_stream().await {
            Ok(user_stream) => user_stream,
            Err(err) => return Err(Self::mongodb_error_to_unknown_status(err)),
        };

        while let Some(user_or) = user_stream.next().await {
            match user_or {
                Ok(user) => {
                    if let Err(err) = self.search_client.index_user(&user, false) {
                        return Err(match err {
                            IndexUserError::ParseNameError(error) => error.to_status(),
                            IndexUserError::SonicError(error) => {
                                Self::sonic_error_to_unknown_status(error)
                            }
                        });
                    }
                }
                Err(err) => return Err(Self::mongodb_error_to_unknown_status(err)),
            };
        }
        Ok(Response::new(Empty {}))
    }
}
