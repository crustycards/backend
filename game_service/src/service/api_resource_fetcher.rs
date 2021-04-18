use async_trait::async_trait;
use mockall::automock;
use shared::proto::cardpack_service_client::CardpackServiceClient;
use shared::proto::user_service_client::UserServiceClient;
use shared::proto::GetUserRequest;
use shared::proto::{
    CustomBlackCard, CustomWhiteCard, DefaultBlackCard, DefaultWhiteCard,
    ListCustomBlackCardsRequest, ListCustomWhiteCardsRequest, ListDefaultBlackCardsRequest,
    ListDefaultWhiteCardsRequest, User,
};
use tonic::transport::Channel;
use tonic::{Request, Status};

#[automock]
#[async_trait]
pub trait ApiResourceFetcher: Send + Sync {
    async fn get_user(&self, user_name: String) -> Result<User, Status>;

    // Retrieves all black and white custom cards from multiple custom cardpacks, or returns None if any error is encountered.
    async fn get_custom_cards_from_multiple_custom_cardpacks(
        &self,
        cardpack_names: &[String],
    ) -> Result<(Vec<CustomBlackCard>, Vec<CustomWhiteCard>), Status>;

    // Retrieves all black and white default cards from multiple default cardpacks, or returns None if any error is encountered.
    async fn get_default_cards_from_multiple_default_cardpacks(
        &self,
        default_cardpack_names: &[String],
    ) -> Result<(Vec<DefaultBlackCard>, Vec<DefaultWhiteCard>), Status>;
}

pub struct GrpcApiResourceFetcher {
    cardpack_service_client: CardpackServiceClient<Channel>,
    user_service_client: UserServiceClient<Channel>,
}

impl GrpcApiResourceFetcher {
    pub fn new(
        cardpack_service: CardpackServiceClient<Channel>,
        user_service: UserServiceClient<Channel>,
    ) -> GrpcApiResourceFetcher {
        GrpcApiResourceFetcher {
            cardpack_service_client: cardpack_service,
            user_service_client: user_service,
        }
    }

    async fn get_custom_black_cards_from_custom_cardpack(
        &self,
        custom_cardpack_name: &str,
    ) -> Result<Vec<CustomBlackCard>, Status> {
        let mut cards = Vec::new();
        let mut next_page_token = String::new();
        let mut has_finished_first_call = false;
        while !next_page_token.is_empty() || !has_finished_first_call {
            let request = ListCustomBlackCardsRequest {
                parent: String::from(custom_cardpack_name),
                page_size: 1000,
                page_token: next_page_token,
                show_deleted: false,
            };
            // Cloning Tonic generated clients is extremely cheap, and allows multithreading like we need here.
            let mut response = match self
                .cardpack_service_client
                .clone()
                .list_custom_black_cards(Request::new(request))
                .await
            {
                Ok(response) => response,
                Err(err) => return Err(err),
            };
            has_finished_first_call = true;
            next_page_token = String::from(&response.get_ref().next_page_token);
            cards.append(&mut response.get_mut().custom_black_cards);
        }
        return Ok(cards);
    }

    async fn get_custom_white_cards_from_custom_cardpack(
        &self,
        custom_cardpack_name: &str,
    ) -> Result<Vec<CustomWhiteCard>, Status> {
        let mut cards = Vec::new();
        let mut next_page_token = String::new();
        let mut has_finished_first_call = false;
        while !next_page_token.is_empty() || !has_finished_first_call {
            let request = ListCustomWhiteCardsRequest {
                parent: String::from(custom_cardpack_name),
                page_size: 1000,
                page_token: next_page_token,
                show_deleted: false,
            };
            // Cloning Tonic generated clients is extremely cheap, and allows multithreading like we need here.
            let mut response = match self
                .cardpack_service_client
                .clone()
                .list_custom_white_cards(Request::new(request))
                .await
            {
                Ok(response) => response,
                Err(err) => return Err(err),
            };
            has_finished_first_call = true;
            next_page_token = String::from(&response.get_ref().next_page_token);
            cards.append(&mut response.get_mut().custom_white_cards);
        }
        return Ok(cards);
    }

    async fn get_default_black_cards_from_default_cardpack(
        &self,
        default_cardpack_name: &str,
    ) -> Result<Vec<DefaultBlackCard>, Status> {
        let mut cards = Vec::new();
        let mut next_page_token = String::new();
        let mut has_finished_first_call = false;
        while !next_page_token.is_empty() || !has_finished_first_call {
            let request = ListDefaultBlackCardsRequest {
                parent: String::from(default_cardpack_name),
                page_size: 1000,
                page_token: next_page_token,
            };
            // Cloning Tonic generated clients is extremely cheap, and allows multithreading like we need here.
            let mut response = match self
                .cardpack_service_client
                .clone()
                .list_default_black_cards(Request::new(request))
                .await
            {
                Ok(response) => response,
                Err(err) => return Err(err),
            };
            has_finished_first_call = true;
            next_page_token = String::from(&response.get_ref().next_page_token);
            cards.append(&mut response.get_mut().default_black_cards);
        }
        return Ok(cards);
    }

    async fn get_default_white_cards_from_default_cardpack(
        &self,
        default_cardpack_name: &str,
    ) -> Result<Vec<DefaultWhiteCard>, Status> {
        let mut cards = Vec::new();
        let mut next_page_token = String::new();
        let mut has_finished_first_call = false;
        while !next_page_token.is_empty() || !has_finished_first_call {
            let request = ListDefaultWhiteCardsRequest {
                parent: String::from(default_cardpack_name),
                page_size: 1000,
                page_token: next_page_token,
            };
            // Cloning Tonic generated clients is extremely cheap, and allows multithreading like we need here.
            let mut response = match self
                .cardpack_service_client
                .clone()
                .list_default_white_cards(Request::new(request))
                .await
            {
                Ok(response) => response,
                Err(err) => return Err(err),
            };
            has_finished_first_call = true;
            next_page_token = String::from(&response.get_ref().next_page_token);
            cards.append(&mut response.get_mut().default_white_cards);
        }
        return Ok(cards);
    }
}

#[async_trait]
impl ApiResourceFetcher for GrpcApiResourceFetcher {
    async fn get_user(&self, user_name: String) -> Result<User, Status> {
        let request = GetUserRequest { name: user_name };
        // Cloning Tonic generated clients is extremely cheap, and allows multithreading like we need here.
        match self
            .user_service_client
            .clone()
            .get_user(Request::new(request))
            .await
        {
            Ok(response) => Ok(response.into_inner()),
            Err(err) => Err(err),
        }
    }

    async fn get_custom_cards_from_multiple_custom_cardpacks(
        &self,
        cardpack_names: &[String],
    ) -> Result<(Vec<CustomBlackCard>, Vec<CustomWhiteCard>), Status> {
        // TODO - Do this work in multiple threads.
        let mut custom_black_cards = Vec::new();
        let mut custom_white_cards = Vec::new();
        for cardpack_name in cardpack_names {
            custom_black_cards.append(&mut match self
                .get_custom_black_cards_from_custom_cardpack(cardpack_name)
                .await
            {
                Ok(cards) => cards,
                Err(err) => return Err(err),
            });
            custom_white_cards.append(&mut match self
                .get_custom_white_cards_from_custom_cardpack(cardpack_name)
                .await
            {
                Ok(cards) => cards,
                Err(err) => return Err(err),
            });
        }
        Ok((custom_black_cards, custom_white_cards))
    }

    async fn get_default_cards_from_multiple_default_cardpacks(
        &self,
        default_cardpack_names: &[String],
    ) -> Result<(Vec<DefaultBlackCard>, Vec<DefaultWhiteCard>), Status> {
        // TODO - Do this work in multiple threads.
        let mut default_black_cards = Vec::new();
        let mut default_white_cards = Vec::new();
        for default_cardpack_name in default_cardpack_names {
            default_black_cards.append(&mut match self
                .get_default_black_cards_from_default_cardpack(default_cardpack_name)
                .await
            {
                Ok(cards) => cards,
                Err(err) => return Err(err),
            });
            default_white_cards.append(&mut match self
                .get_default_white_cards_from_default_cardpack(default_cardpack_name)
                .await
            {
                Ok(cards) => cards,
                Err(err) => return Err(err),
            });
        }
        Ok((default_black_cards, default_white_cards))
    }
}
