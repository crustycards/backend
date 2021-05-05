use super::super::mongo::custom_black_card_collection::CustomBlackCardCollection;
use super::super::mongo::custom_cardpack_collection::CustomCardpackCollection;
use super::super::mongo::custom_white_card_collection::CustomWhiteCardCollection;
use super::super::mongo::user_collection::UserCollection;
use super::super::proto_helper::page_token::*;
use super::default_cardpacks::DefaultCardpackHandler;
use super::helper::*;
use shared::basic_validation::ValidatedStringField;
use shared::proto::crusty_cards_api::cardpack_service_server::CardpackService;
use shared::proto::crusty_cards_api::*;
use shared::proto::google::protobuf::Empty;
use shared::proto_validation::{AnswerFieldCount, BoundedPageSize};
use shared::resource_name::*;
use std::collections::HashSet;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct CardpackServiceImpl {
    custom_cardpack_collection: Box<dyn CustomCardpackCollection>,
    custom_black_card_collection: Box<dyn CustomBlackCardCollection>,
    custom_white_card_collection: Box<dyn CustomWhiteCardCollection>,
    default_cardpack_handler: DefaultCardpackHandler,
    user_collection: Arc<dyn UserCollection>,
}

impl CardpackServiceImpl {
    pub fn new(
        custom_cardpack_collection: Box<dyn CustomCardpackCollection>,
        custom_black_card_collection: Box<dyn CustomBlackCardCollection>,
        custom_white_card_collection: Box<dyn CustomWhiteCardCollection>,
        default_cardpack_handler: DefaultCardpackHandler,
        user_collection: Arc<dyn UserCollection>,
    ) -> Self {
        Self {
            custom_cardpack_collection,
            custom_black_card_collection,
            custom_white_card_collection,
            default_cardpack_handler,
            user_collection,
        }
    }

    fn validate_create_custom_black_card_request(
        request: &CreateCustomBlackCardRequest,
    ) -> Result<(CustomCardpackName, ValidatedStringField, AnswerFieldCount), Status> {
        let parent =
            match CustomCardpackName::new(&ValidatedStringField::new(&request.parent, "parent")?) {
                Ok(custom_cardpack_name) => custom_cardpack_name,
                Err(err) => return Err(err.to_status()),
            };

        let custom_black_card = match &request.custom_black_card {
            Some(card) => card,
            None => return Err(missing_request_field_error("custom_black_card")),
        };

        let card_text =
            ValidatedStringField::new(&custom_black_card.text, "custom_black_card.text")?;

        let answer_field_count = AnswerFieldCount::new(
            custom_black_card.answer_fields,
            "custom_black_card.answer_fields",
        )?;

        Ok((parent, card_text, answer_field_count))
    }

    fn validate_create_custom_white_card_request(
        request: &CreateCustomWhiteCardRequest,
    ) -> Result<(CustomCardpackName, ValidatedStringField), Status> {
        let parent =
            match CustomCardpackName::new(&ValidatedStringField::new(&request.parent, "parent")?) {
                Ok(custom_cardpack_name) => custom_cardpack_name,
                Err(err) => return Err(err.to_status()),
            };

        let custom_white_card = match &request.custom_white_card {
            Some(card) => card,
            None => return Err(missing_request_field_error("custom_white_card")),
        };

        let card_text =
            ValidatedStringField::new(&custom_white_card.text, "custom_white_card.text")?;

        Ok((parent, card_text))
    }
}

#[tonic::async_trait]
impl CardpackService for CardpackServiceImpl {
    async fn create_custom_cardpack(
        &self,
        request: Request<CreateCustomCardpackRequest>,
    ) -> Result<Response<CustomCardpack>, Status> {
        let parent = match UserName::new(&ValidatedStringField::new(
            &request.get_ref().parent,
            "parent",
        )?) {
            Ok(parent) => parent,
            Err(err) => return Err(err.to_status()),
        };

        let custom_cardpack = match &request.get_ref().custom_cardpack {
            Some(custom_cardpack) => custom_cardpack,
            None => return Err(missing_request_field_error("custom_cardpack")),
        };

        let display_name = ValidatedStringField::new(
            &custom_cardpack.display_name,
            "custom_cardpack.display_name",
        )?;

        Ok(Response::new(
            self.custom_cardpack_collection
                .create_custom_cardpack(parent, display_name)
                .await?,
        ))
    }

    async fn get_custom_cardpack(
        &self,
        request: Request<GetCustomCardpackRequest>,
    ) -> Result<Response<CustomCardpack>, Status> {
        let custom_cardpack_name = match CustomCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().name,
            "name",
        )?) {
            Ok(custom_cardpack_name) => custom_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        Ok(Response::new(
            self.custom_cardpack_collection
                .get_custom_cardpack(custom_cardpack_name)
                .await?,
        ))
    }

    async fn list_custom_cardpacks(
        &self,
        request: Request<ListCustomCardpacksRequest>,
    ) -> Result<Response<ListCustomCardpacksResponse>, Status> {
        let user_name = match UserName::new(&ValidatedStringField::new(
            &request.get_ref().parent,
            "parent",
        )?) {
            Ok(user_name) => user_name,
            Err(err) => return Err(err.to_status()),
        };

        let request_without_page_token = {
            let mut req = request.get_ref().clone();
            req.page_token = String::from("");
            req.page_size = 0;
            req
        };

        let mut last_object_id_or = None;
        if !request.get_ref().page_token.is_empty() {
            last_object_id_or = Some(parse_page_token_object_id(
                &request_without_page_token,
                &request.get_ref().page_token,
            )?);
        }

        let (custom_cardpacks, next_object_id_or, total_size) = self
            .custom_cardpack_collection
            .list_custom_cardpacks(
                user_name,
                BoundedPageSize::new(request.get_ref().page_size)?,
                last_object_id_or,
                request.get_ref().show_deleted,
            )
            .await?;

        let next_page_token = match next_object_id_or {
            Some(next_object_id) => {
                create_page_token(&request_without_page_token, next_object_id.to_hex())
            }
            None => String::from(""),
        };

        Ok(Response::new(ListCustomCardpacksResponse {
            custom_cardpacks,
            next_page_token,
            total_size,
        }))
    }

    async fn update_custom_cardpack(
        &self,
        request: Request<UpdateCustomCardpackRequest>,
    ) -> Result<Response<CustomCardpack>, Status> {
        let update_mask = match &request.get_ref().update_mask {
            Some(update_mask) => update_mask,
            None => return Err(missing_request_field_error("update_mask")),
        };

        let custom_cardpack = match &request.get_ref().custom_cardpack {
            Some(custom_cardpack) => custom_cardpack,
            None => return Err(missing_request_field_error("custom_cardpack")),
        };

        let custom_cardpack_name = match CustomCardpackName::new(&ValidatedStringField::new(
            &custom_cardpack.name,
            "custom_cardpack.name",
        )?) {
            Ok(custom_cardpack_name) => custom_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        let update_fields: HashSet<String> = update_mask.paths.iter().cloned().collect();
        let updated_display_name_or = if update_fields.contains("display_name") {
            match ValidatedStringField::new(
                &custom_cardpack.display_name,
                "custom_cardpack.display_name",
            ) {
                Ok(display_name) => Some(display_name),
                Err(err) => return Err(err),
            }
        } else {
            None
        };

        match updated_display_name_or {
            Some(updated_display_name) => Ok(Response::new(
                self.custom_cardpack_collection
                    .update_custom_cardpack(custom_cardpack_name, updated_display_name)
                    .await?,
            )),
            None => Ok(Response::new(
                self.custom_cardpack_collection
                    .get_custom_cardpack(custom_cardpack_name)
                    .await?,
            )),
        }
    }

    async fn delete_custom_cardpack(
        &self,
        request: Request<DeleteCustomCardpackRequest>,
    ) -> Result<Response<CustomCardpack>, Status> {
        let custom_cardpack_name = match CustomCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().name,
            "name",
        )?) {
            Ok(custom_cardpack_name) => custom_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        Ok(Response::new(
            self.custom_cardpack_collection
                .soft_delete_custom_cardpack(custom_cardpack_name)
                .await?,
        ))
    }

    async fn create_custom_black_card(
        &self,
        request: Request<CreateCustomBlackCardRequest>,
    ) -> Result<Response<CustomBlackCard>, Status> {
        let (parent, card_text, answer_fields) =
            match CardpackServiceImpl::validate_create_custom_black_card_request(request.get_ref())
            {
                Ok((parent, card_text, answer_fields)) => (parent, card_text, answer_fields),
                Err(grpc_err) => return Err(grpc_err),
            };

        let new_card = self
            .custom_black_card_collection
            .create_custom_black_card(parent, card_text, answer_fields)
            .await?;

        Ok(Response::new(new_card))
    }

    async fn create_custom_white_card(
        &self,
        request: Request<CreateCustomWhiteCardRequest>,
    ) -> Result<Response<CustomWhiteCard>, Status> {
        let (parent, card_text) =
            match CardpackServiceImpl::validate_create_custom_white_card_request(request.get_ref())
            {
                Ok((parent, card_text)) => (parent, card_text),
                Err(grpc_err) => return Err(grpc_err),
            };

        let new_card = self
            .custom_white_card_collection
            .create_custom_white_card(parent, card_text)
            .await?;

        Ok(Response::new(new_card))
    }

    async fn list_custom_black_cards(
        &self,
        request: Request<ListCustomBlackCardsRequest>,
    ) -> Result<Response<ListCustomBlackCardsResponse>, Status> {
        let custom_cardpack_name = match CustomCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().parent,
            "parent",
        )?) {
            Ok(custom_cardpack_name) => custom_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        let request_without_page_token = {
            let mut req = request.get_ref().clone();
            req.page_token = String::from("");
            req.page_size = 0;
            req
        };

        let mut last_object_id_or = None;
        if !request.get_ref().page_token.is_empty() {
            last_object_id_or = Some(parse_page_token_object_id(
                &request_without_page_token,
                &request.get_ref().page_token,
            )?);
        }

        let (custom_black_cards, next_object_id_or, total_size) = self
            .custom_black_card_collection
            .list_custom_black_cards(
                custom_cardpack_name,
                BoundedPageSize::new(request.get_ref().page_size)?,
                last_object_id_or,
                request.get_ref().show_deleted,
            )
            .await?;

        let next_page_token = match next_object_id_or {
            Some(next_object_id) => {
                create_page_token(&request_without_page_token, next_object_id.to_hex())
            }
            None => String::from(""),
        };

        Ok(Response::new(ListCustomBlackCardsResponse {
            custom_black_cards,
            next_page_token,
            total_size,
        }))
    }

    async fn list_custom_white_cards(
        &self,
        request: Request<ListCustomWhiteCardsRequest>,
    ) -> Result<Response<ListCustomWhiteCardsResponse>, Status> {
        let custom_cardpack_name = match CustomCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().parent,
            "parent",
        )?) {
            Ok(custom_cardpack_name) => custom_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        let request_without_page_token = {
            let mut req = request.get_ref().clone();
            req.page_token = String::from("");
            req.page_size = 0;
            req
        };

        let mut last_object_id_or = None;
        if !request.get_ref().page_token.is_empty() {
            last_object_id_or = Some(parse_page_token_object_id(
                &request_without_page_token,
                &request.get_ref().page_token,
            )?);
        }

        let (custom_white_cards, next_object_id_or, total_size) = self
            .custom_white_card_collection
            .list_custom_white_cards(
                custom_cardpack_name,
                BoundedPageSize::new(request.get_ref().page_size)?,
                last_object_id_or,
                request.get_ref().show_deleted,
            )
            .await?;

        let next_page_token = match next_object_id_or {
            Some(next_object_id) => {
                create_page_token(&request_without_page_token, next_object_id.to_hex())
            }
            None => String::from(""),
        };

        Ok(Response::new(ListCustomWhiteCardsResponse {
            custom_white_cards,
            next_page_token,
            total_size,
        }))
    }

    async fn update_custom_black_card(
        &self,
        request: Request<UpdateCustomBlackCardRequest>,
    ) -> Result<Response<CustomBlackCard>, Status> {
        let update_mask = match &request.get_ref().update_mask {
            Some(update_mask) => update_mask,
            None => return Err(missing_request_field_error("update_mask")),
        };

        let custom_black_card = match &request.get_ref().custom_black_card {
            Some(custom_black_card) => custom_black_card,
            None => return Err(missing_request_field_error("custom_black_card")),
        };

        let custom_black_card_name = match CustomBlackCardName::new(&ValidatedStringField::new(
            &custom_black_card.name,
            "custom_black_card.name",
        )?) {
            Ok(custom_black_card_name) => custom_black_card_name,
            Err(err) => return Err(err.to_status()),
        };

        let update_fields: HashSet<String> = update_mask.paths.iter().cloned().collect();

        let updated_card_text_or = if update_fields.contains("text") {
            Some(ValidatedStringField::new(
                &custom_black_card.text,
                "custom_black_card.text",
            )?)
        } else {
            None
        };

        let updated_answer_fields_or = if update_fields.contains("answer_fields") {
            Some(AnswerFieldCount::new(
                custom_black_card.answer_fields,
                "custom_black_card.answer_fields",
            )?)
        } else {
            None
        };

        Ok(Response::new(
            self.custom_black_card_collection
                .update_custom_black_card(
                    custom_black_card_name,
                    updated_card_text_or,
                    updated_answer_fields_or,
                )
                .await?,
        ))
    }

    async fn update_custom_white_card(
        &self,
        request: Request<UpdateCustomWhiteCardRequest>,
    ) -> Result<Response<CustomWhiteCard>, Status> {
        let update_mask = match &request.get_ref().update_mask {
            Some(update_mask) => update_mask,
            None => return Err(missing_request_field_error("update_mask")),
        };

        let custom_white_card = match &request.get_ref().custom_white_card {
            Some(custom_white_card) => custom_white_card,
            None => return Err(missing_request_field_error("custom_white_card")),
        };

        let custom_white_card_name = match CustomWhiteCardName::new(&ValidatedStringField::new(
            &custom_white_card.name,
            "custom_white_card.name",
        )?) {
            Ok(custom_white_card_name) => custom_white_card_name,
            Err(err) => return Err(err.to_status()),
        };

        let update_fields: HashSet<String> = update_mask.paths.iter().cloned().collect();
        let updated_card_text_or = if update_fields.contains("text") {
            match ValidatedStringField::new(&custom_white_card.text, "custom_white_card.text") {
                Ok(card_text) => Some(card_text),
                Err(err) => return Err(err),
            }
        } else {
            None
        };

        match updated_card_text_or {
            Some(updated_card_text) => Ok(Response::new(
                self.custom_white_card_collection
                    .update_custom_white_card(custom_white_card_name, updated_card_text)
                    .await?,
            )),
            None => Ok(Response::new(
                self.custom_white_card_collection
                    .get_custom_white_card(custom_white_card_name)
                    .await?,
            )),
        }
    }

    async fn delete_custom_black_card(
        &self,
        request: Request<DeleteCustomBlackCardRequest>,
    ) -> Result<Response<CustomBlackCard>, Status> {
        let custom_black_card_name = match CustomBlackCardName::new(&ValidatedStringField::new(
            &request.get_ref().name,
            "name",
        )?) {
            Ok(custom_black_card_name) => custom_black_card_name,
            Err(err) => return Err(err.to_status()),
        };

        Ok(Response::new(
            self.custom_black_card_collection
                .soft_delete_custom_black_card(custom_black_card_name)
                .await?,
        ))
    }

    async fn delete_custom_white_card(
        &self,
        request: Request<DeleteCustomWhiteCardRequest>,
    ) -> Result<Response<CustomWhiteCard>, Status> {
        let custom_white_card_name = match CustomWhiteCardName::new(&ValidatedStringField::new(
            &request.get_ref().name,
            "name",
        )?) {
            Ok(custom_white_card_name) => custom_white_card_name,
            Err(err) => return Err(err.to_status()),
        };

        Ok(Response::new(
            self.custom_white_card_collection
                .soft_delete_custom_white_card(custom_white_card_name)
                .await?,
        ))
    }

    async fn batch_create_custom_black_cards(
        &self,
        request: Request<BatchCreateCustomBlackCardsRequest>,
    ) -> Result<Response<BatchCreateCustomBlackCardsResponse>, Status> {
        let parent = match CustomCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().parent,
            "parent",
        )?) {
            Ok(custom_cardpack_name) => custom_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        for req in &request.get_ref().requests {
            if !req.parent.is_empty() && req.parent != request.get_ref().parent {
                return Err(batch_create_differing_parent_error());
            }
        }

        if request.get_ref().requests.len() > 10000 {
            return Err(batch_request_exceeds_request_limit_error(10000));
        }

        let mut data = Vec::new();
        for req in &request.get_ref().requests {
            let (_, card_text, answer_field_count) =
                match CardpackServiceImpl::validate_create_custom_black_card_request(&req) {
                    Ok((parent, card_text, answer_field_count)) => {
                        (parent, card_text, answer_field_count)
                    }
                    Err(grpc_err) => return Err(grpc_err),
                };
            data.push((card_text, answer_field_count));
        }

        let response = BatchCreateCustomBlackCardsResponse {
            custom_black_cards: self
                .custom_black_card_collection
                .batch_create_custom_black_cards(parent, data)
                .await?
                .into_iter()
                .filter_map(|item| item)
                .collect(),
        };

        Ok(Response::new(response))
    }

    async fn batch_create_custom_white_cards(
        &self,
        request: Request<BatchCreateCustomWhiteCardsRequest>,
    ) -> Result<Response<BatchCreateCustomWhiteCardsResponse>, Status> {
        let parent = match CustomCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().parent,
            "parent",
        )?) {
            Ok(custom_cardpack_name) => custom_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        for req in &request.get_ref().requests {
            if !req.parent.is_empty() && req.parent != request.get_ref().parent {
                return Err(batch_create_differing_parent_error());
            }
        }

        if request.get_ref().requests.len() > 10000 {
            return Err(batch_request_exceeds_request_limit_error(10000));
        }

        let mut card_texts = Vec::new();
        for req in &request.get_ref().requests {
            let (_, card_text) =
                match CardpackServiceImpl::validate_create_custom_white_card_request(&req) {
                    Ok((parent, card_text)) => (parent, card_text),
                    Err(grpc_err) => return Err(grpc_err),
                };
            card_texts.push(card_text);
        }

        let response = BatchCreateCustomWhiteCardsResponse {
            custom_white_cards: self
                .custom_white_card_collection
                .batch_create_custom_white_cards(parent, card_texts)
                .await?
                .into_iter()
                .filter_map(|item| item)
                .collect(),
        };

        Ok(Response::new(response))
    }

    async fn get_default_cardpack(
        &self,
        request: Request<GetDefaultCardpackRequest>,
    ) -> Result<Response<DefaultCardpack>, Status> {
        let default_cardpack_name = match DefaultCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().name,
            "name",
        )?) {
            Ok(default_cardpack_name) => default_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        let pack_data = match self
            .default_cardpack_handler
            .get_pack_by_name(&default_cardpack_name)
        {
            Some(pack) => pack,
            None => return Err(Status::unknown("Failed to fetch cardpack.")),
        };

        Ok(Response::new(pack_data.get_default_cardpack().clone()))
    }

    async fn list_default_cardpacks(
        &self,
        request: Request<ListDefaultCardpacksRequest>,
    ) -> Result<Response<ListDefaultCardpacksResponse>, Status> {
        let bounded_page_size = BoundedPageSize::new(request.get_ref().page_size)?;

        let request_without_page_token = {
            let mut req = request.get_ref().clone();
            req.page_token = String::from("");
            req.page_size = 0;
            req
        };

        let mut start_index: usize = 0;

        if !request.get_ref().page_token.is_empty() {
            start_index = match parse_page_token_string(
                &request_without_page_token,
                &request.get_ref().page_token,
            ) {
                Ok(index_string) => match index_string.parse::<usize>() {
                    Ok(index) => index,
                    Err(_) => return Err(invalid_page_token_error()),
                },
                Err(grpc_err) => return Err(grpc_err),
            };
        }

        let end_index: usize = start_index + bounded_page_size.take_i64() as usize;

        let mut default_cardpacks: Vec<DefaultCardpack> = Vec::new();
        let all_packs = self.default_cardpack_handler.get_pack_list();

        for i in start_index..end_index {
            match all_packs.get(i) {
                Some(pack) => default_cardpacks.push(pack.get_default_cardpack().clone()),
                None => break,
            };
        }

        let mut next_page_token = String::from("");
        if !default_cardpacks.is_empty() && all_packs.len() > end_index {
            next_page_token =
                create_page_token(&request_without_page_token, format!("{}", end_index));
        }

        Ok(Response::new(ListDefaultCardpacksResponse {
            default_cardpacks,
            next_page_token,
            total_size: self.default_cardpack_handler.get_pack_list().len() as i64,
        }))
    }

    async fn list_default_black_cards(
        &self,
        request: Request<ListDefaultBlackCardsRequest>,
    ) -> Result<Response<ListDefaultBlackCardsResponse>, Status> {
        let default_cardpack_name = match DefaultCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().parent,
            "parent",
        )?) {
            Ok(default_cardpack_name) => default_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        let bounded_page_size = BoundedPageSize::new(request.get_ref().page_size)?;

        let request_without_page_token = {
            let mut req = request.get_ref().clone();
            req.page_token = String::from("");
            req.page_size = 0;
            req
        };

        let mut start_index: usize = 0;

        if !request.get_ref().page_token.is_empty() {
            start_index = match parse_page_token_string(
                &request_without_page_token,
                &request.get_ref().page_token,
            ) {
                Ok(index_string) => match index_string.parse::<usize>() {
                    Ok(index) => index,
                    Err(_) => return Err(invalid_page_token_error()),
                },
                Err(grpc_err) => return Err(grpc_err),
            };
        }

        let end_index: usize = start_index + bounded_page_size.take_i64() as usize;

        let mut default_black_cards: Vec<DefaultBlackCard> = Vec::new();
        let all_cards_from_pack = match self
            .default_cardpack_handler
            .get_pack_by_name(&default_cardpack_name)
        {
            Some(pack) => pack.get_default_black_cards(),
            None => return Err(Status::invalid_argument("Parent does not exist.")),
        };

        for i in start_index..end_index {
            match all_cards_from_pack.get(i) {
                Some(card) => default_black_cards.push(card.clone()),
                None => break,
            };
        }

        let mut next_page_token = String::from("");
        if !default_black_cards.is_empty() && all_cards_from_pack.len() > end_index {
            next_page_token =
                create_page_token(&request_without_page_token, format!("{}", end_index));
        }

        Ok(Response::new(ListDefaultBlackCardsResponse {
            default_black_cards,
            next_page_token,
            total_size: all_cards_from_pack.len() as i64,
        }))
    }

    async fn list_default_white_cards(
        &self,
        request: Request<ListDefaultWhiteCardsRequest>,
    ) -> Result<Response<ListDefaultWhiteCardsResponse>, Status> {
        let default_cardpack_name = match DefaultCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().parent,
            "parent",
        )?) {
            Ok(default_cardpack_name) => default_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        let bounded_page_size = BoundedPageSize::new(request.get_ref().page_size)?;

        let request_without_page_token = {
            let mut req = request.get_ref().clone();
            req.page_token = String::from("");
            req.page_size = 0;
            req
        };

        let mut start_index: usize = 0;

        if !request.get_ref().page_token.is_empty() {
            start_index = match parse_page_token_string(
                &request_without_page_token,
                &request.get_ref().page_token,
            ) {
                Ok(index_string) => match index_string.parse::<usize>() {
                    Ok(index) => index,
                    Err(_) => return Err(invalid_page_token_error()),
                },
                Err(grpc_err) => return Err(grpc_err),
            };
        }

        let end_index: usize = start_index + bounded_page_size.take_i64() as usize;

        let mut default_white_cards: Vec<DefaultWhiteCard> = Vec::new();
        let all_cards_from_pack = match self
            .default_cardpack_handler
            .get_pack_by_name(&default_cardpack_name)
        {
            Some(pack) => pack.get_default_white_cards(),
            None => return Err(Status::invalid_argument("Parent does not exist.")),
        };

        for i in start_index..end_index {
            match all_cards_from_pack.get(i) {
                Some(card) => default_white_cards.push(card.clone()),
                None => break,
            };
        }

        let mut next_page_token = String::from("");
        if !default_white_cards.is_empty() && all_cards_from_pack.len() > end_index {
            next_page_token =
                create_page_token(&request_without_page_token, format!("{}", end_index));
        }

        Ok(Response::new(ListDefaultWhiteCardsResponse {
            default_white_cards,
            next_page_token,
            total_size: all_cards_from_pack.len() as i64,
        }))
    }

    async fn undelete_custom_cardpack(
        &self,
        request: Request<UndeleteCustomCardpackRequest>,
    ) -> Result<Response<CustomCardpack>, Status> {
        let custom_cardpack_name = match CustomCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().name,
            "name",
        )?) {
            Ok(custom_cardpack_name) => custom_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        Ok(Response::new(
            self.custom_cardpack_collection
                .undelete_custom_cardpack(custom_cardpack_name)
                .await?,
        ))
    }

    async fn undelete_custom_black_card(
        &self,
        request: Request<UndeleteCustomBlackCardRequest>,
    ) -> Result<Response<CustomBlackCard>, Status> {
        let custom_black_card_name = match CustomBlackCardName::new(&ValidatedStringField::new(
            &request.get_ref().name,
            "name",
        )?) {
            Ok(custom_black_card_name) => custom_black_card_name,
            Err(err) => return Err(err.to_status()),
        };

        Ok(Response::new(
            self.custom_black_card_collection
                .undelete_custom_black_card(custom_black_card_name)
                .await?,
        ))
    }

    async fn undelete_custom_white_card(
        &self,
        request: Request<UndeleteCustomWhiteCardRequest>,
    ) -> Result<Response<CustomWhiteCard>, Status> {
        let custom_white_card_name = match CustomWhiteCardName::new(&ValidatedStringField::new(
            &request.get_ref().name,
            "name",
        )?) {
            Ok(custom_white_card_name) => custom_white_card_name,
            Err(err) => return Err(err.to_status()),
        };

        Ok(Response::new(
            self.custom_white_card_collection
                .undelete_custom_white_card(custom_white_card_name)
                .await?,
        ))
    }

    async fn list_favorited_custom_cardpacks(
        &self,
        request: Request<ListFavoritedCustomCardpacksRequest>,
    ) -> Result<Response<ListFavoritedCustomCardpacksResponse>, Status> {
        let user_name = match UserName::new(&ValidatedStringField::new(
            &request.get_ref().parent,
            "parent",
        )?) {
            Ok(user_name) => user_name,
            Err(err) => return Err(err.to_status()),
        };

        let bounded_page_size = BoundedPageSize::new(request.get_ref().page_size)?;

        let request_without_page_token = {
            let mut req = request.get_ref().clone();
            req.page_token = String::from("");
            req.page_size = 0;
            req
        };

        let mut start_index: usize = 0;
        if !request.get_ref().page_token.is_empty() {
            start_index = match parse_page_token_string(
                &request_without_page_token,
                &request.get_ref().page_token,
            ) {
                Ok(index_string) => match index_string.parse::<usize>() {
                    Ok(index) => index,
                    Err(_) => return Err(invalid_page_token_error()),
                },
                Err(grpc_err) => return Err(grpc_err),
            };
        }

        let (custom_cardpack_names, next_index_or) = self
            .user_collection
            .list_favorited_custom_cardpack_names(user_name, bounded_page_size, start_index)
            .await?;

        let custom_cardpacks_or = match self
            .custom_cardpack_collection
            .get_cardpacks_from_names(custom_cardpack_names)
            .await
        {
            Ok(custom_cardpacks) => custom_cardpacks,
            _ => return Err(Status::unknown("Failed to fetch cardpacks.")),
        };

        let custom_cardpacks: Vec<CustomCardpack> = custom_cardpacks_or
            .into_iter()
            .map(|cardpack_or| match cardpack_or {
                Some(cardpack) => cardpack,
                None => CustomCardpack {
                    name: String::from(""),
                    display_name: String::from(""),
                    create_time: None,
                    update_time: None,
                    delete_time: None,
                },
            })
            .collect();

        let next_page_token = match next_index_or {
            Some(next_index) => {
                create_page_token(&request_without_page_token, format!("{}", next_index))
            }
            None => String::from(""),
        };

        Ok(Response::new(ListFavoritedCustomCardpacksResponse {
            custom_cardpacks,
            next_page_token,
        }))
    }

    async fn like_custom_cardpack(
        &self,
        request: Request<LikeCustomCardpackRequest>,
    ) -> Result<Response<Empty>, Status> {
        let user_name =
            match UserName::new(&ValidatedStringField::new(&request.get_ref().user, "user")?) {
                Ok(user_name) => user_name,
                Err(err) => return Err(err.to_status()),
            };

        let custom_cardpack_name = match CustomCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().custom_cardpack,
            "custom_cardpack",
        )?) {
            Ok(custom_cardpack_name) => custom_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        self.user_collection
            .add_custom_cardpack_to_favorites(user_name, custom_cardpack_name)
            .await?;

        Ok(Response::new(Empty {}))
    }

    async fn unlike_custom_cardpack(
        &self,
        request: Request<UnlikeCustomCardpackRequest>,
    ) -> Result<Response<Empty>, Status> {
        let user_name =
            match UserName::new(&ValidatedStringField::new(&request.get_ref().user, "user")?) {
                Ok(user_name) => user_name,
                Err(err) => return Err(err.to_status()),
            };

        let custom_cardpack_name = match CustomCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().custom_cardpack,
            "custom_cardpack",
        )?) {
            Ok(custom_cardpack_name) => custom_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        self.user_collection
            .remove_custom_cardpack_from_favorites(user_name, custom_cardpack_name)
            .await?;

        Ok(Response::new(Empty {}))
    }

    async fn check_does_user_like_custom_cardpack(
        &self,
        request: Request<CheckDoesUserLikeCustomCardpackRequest>,
    ) -> Result<Response<CheckDoesUserLikeCustomCardpackResponse>, Status> {
        let user_name =
            match UserName::new(&ValidatedStringField::new(&request.get_ref().user, "user")?) {
                Ok(user_name) => user_name,
                Err(err) => return Err(err.to_status()),
            };

        let custom_cardpack_name = match CustomCardpackName::new(&ValidatedStringField::new(
            &request.get_ref().custom_cardpack,
            "custom_cardpack",
        )?) {
            Ok(custom_cardpack_name) => custom_cardpack_name,
            Err(err) => return Err(err.to_status()),
        };

        let is_liked = self
            .user_collection
            .check_has_user_favorited_custom_cardpack(user_name, custom_cardpack_name)
            .await?;

        Ok(Response::new(CheckDoesUserLikeCustomCardpackResponse {
            is_liked,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::mongo::custom_black_card_collection::MockCustomBlackCardCollection;
    use super::super::super::mongo::custom_cardpack_collection::MockCustomCardpackCollection;
    use super::super::super::mongo::custom_white_card_collection::MockCustomWhiteCardCollection;
    use super::super::super::mongo::user_collection::MockUserCollection;
    use super::super::default_cardpacks::DefaultCardpackData;
    use super::*;

    async fn get_local_test_cardpack_service_with_custom_default_cardpacks(
        custom_default_cardpack_handler: DefaultCardpackHandler,
    ) -> CardpackServiceImpl {
        CardpackServiceImpl::new(
            Box::from(MockCustomCardpackCollection::new()),
            Box::from(MockCustomBlackCardCollection::new()),
            Box::from(MockCustomWhiteCardCollection::new()),
            custom_default_cardpack_handler,
            Arc::from(MockUserCollection::new()),
        )
    }

    async fn test_delete_and_undelete_item<
        T: PartialEq + std::fmt::Debug,
        ListReq,
        DeleteItemFut,
        UndeleteItemFut,
        Fut,
        ListFut,
    >(
        item: T,
        delete_item: &impl Fn() -> DeleteItemFut,
        undelete_item: &impl Fn() -> UndeleteItemFut,
        cannot_be_deleted_error_message: String,
        cannot_be_undeleted_error_message: String,
        build_list_items_request: &impl Fn(bool) -> ListReq,
        list_items: &impl Fn(ListReq) -> ListFut,
        use_get_item_fn: bool,
        get_item: &impl Fn() -> Fut,
        item_has_delete_time: &impl Fn(&T) -> bool,
    ) where
        ListFut: std::future::Future<Output = (Vec<T>, String)>,
        DeleteItemFut: std::future::Future<Output = Result<T, String>>,
        UndeleteItemFut: std::future::Future<Output = Result<T, String>>,
        Fut: std::future::Future<Output = T>,
    {
        // Sanity check that item exists.
        assert!(!item_has_delete_time(&item));
        if use_get_item_fn {
            assert!(!item_has_delete_time(&get_item().await));
        }

        // Delete item.
        assert!(item_has_delete_time(&delete_item().await.unwrap()));
        // Attempt to delete item again.
        assert_eq!(
            delete_item().await.err().unwrap(),
            cannot_be_deleted_error_message
        );

        // Retrieve deleted item (by name and by listing from parent).
        if use_get_item_fn {
            assert!(item_has_delete_time(&get_item().await));
        }
        assert!(list_items(build_list_items_request(true))
            .await
            .0
            .first()
            .is_some());
        // Also check that item doesn't show in undeleted list.
        assert!(list_items(build_list_items_request(false))
            .await
            .0
            .is_empty());

        // Undelete item.
        assert!(!item_has_delete_time(&undelete_item().await.unwrap()));
        // Attempt to undelete item again.
        assert_eq!(
            undelete_item().await.err().unwrap(),
            cannot_be_undeleted_error_message
        );

        // Retrieve undeleted item (by name and by listing from parent).
        if use_get_item_fn {
            assert!(!item_has_delete_time(&get_item().await));
        }
        assert!(list_items(build_list_items_request(false))
            .await
            .0
            .first()
            .is_some());
        // Also check that item doesn't show in deleted list.
        assert!(list_items(build_list_items_request(true))
            .await
            .0
            .is_empty());
    }

    async fn test_list_pagination<T: PartialEq + std::fmt::Debug, Req, Fut>(
        expected_items: &Vec<T>,
        build_request: &impl Fn(i32, String) -> Req,
        list_items: &impl Fn(Req) -> Fut,
    ) where
        Fut: std::future::Future<Output = (Vec<T>, String)>,
    {
        let mut items = Vec::new();
        let mut next_page_token = String::from("");
        let mut page_size = 1; // Used to test various page sizes.

        loop {
            let mut res = list_items(build_request(page_size, next_page_token)).await;
            next_page_token = res.1;
            if !next_page_token.is_empty() {
                assert_eq!(res.0.len(), page_size as usize);
            }
            page_size += 1;
            items.append(&mut res.0);
            if next_page_token.is_empty() {
                // TODO - Somehow ensure that we are testing a request with a page size that
                // matches the exact end of the collection, as well as one that overflows.
                // Right now we are testing one of the two at random since we are simply
                // incrementing the page size at each iteration.
                break;
            }
        }

        assert_eq!(items.len(), expected_items.len());
        for i in 0..items.len() {
            assert_eq!(
                items.get(i).unwrap() == expected_items.get(i).unwrap(),
                true
            );
        }
    }

    #[tokio::test]
    async fn get_default_cardpack_by_name() {
        let cardpack_service = get_local_test_cardpack_service_with_custom_default_cardpacks(
            DefaultCardpackHandler::new_with_custom_packs(
                DefaultCardpackData::create_list_from_raw_data(vec![(
                    String::from("Cardpack"),
                    Vec::new(),
                    Vec::new(),
                )]),
            ),
        )
        .await;

        let list_default_cardpacks_request = ListDefaultCardpacksRequest {
            page_size: 0,
            page_token: String::from(""),
        };
        let list_default_cardpacks_response = cardpack_service
            .list_default_cardpacks(Request::new(list_default_cardpacks_request))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(list_default_cardpacks_response.default_cardpacks.len(), 1);
        let cardpack = list_default_cardpacks_response
            .default_cardpacks
            .first()
            .unwrap();

        let get_default_cardpack_request = GetDefaultCardpackRequest {
            name: cardpack.name.clone(),
        };
        let other_cardpack = cardpack_service
            .get_default_cardpack(Request::new(get_default_cardpack_request))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(cardpack.display_name, other_cardpack.display_name);
    }

    #[tokio::test]
    async fn list_default_cardpacks() {
        // Test listing cardpacks when no cardpacks exist.
        let mut cardpack_service = get_local_test_cardpack_service_with_custom_default_cardpacks(
            DefaultCardpackHandler::new_with_custom_packs(
                DefaultCardpackData::create_list_from_raw_data(Vec::new()),
            ),
        )
        .await;
        let mut list_default_cardpacks_request = ListDefaultCardpacksRequest {
            page_size: 50,
            page_token: String::from(""),
        };
        let mut response = cardpack_service
            .list_default_cardpacks(Request::new(list_default_cardpacks_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_cardpacks.len(), 0);
        assert!(response.next_page_token.is_empty());
        assert_eq!(response.total_size, 0);

        // Add some default cardpacks.
        cardpack_service = get_local_test_cardpack_service_with_custom_default_cardpacks(
            DefaultCardpackHandler::new_with_custom_packs(
                DefaultCardpackData::create_list_from_raw_data(vec![
                    (String::from("Cardpack 1"), Vec::new(), Vec::new()),
                    (String::from("Cardpack 2"), Vec::new(), Vec::new()),
                    (String::from("Cardpack 3"), Vec::new(), Vec::new()),
                    (String::from("Cardpack 4"), Vec::new(), Vec::new()),
                ]),
            ),
        )
        .await;

        // Request exact amount of items than are available.
        list_default_cardpacks_request = ListDefaultCardpacksRequest {
            page_size: 4,
            page_token: String::from(""),
        };
        response = cardpack_service
            .list_default_cardpacks(Request::new(list_default_cardpacks_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_cardpacks.len(), 4);
        assert!(response.next_page_token.is_empty());
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_cardpacks.first().unwrap().display_name,
            "Cardpack 1"
        );
        assert_eq!(
            response.default_cardpacks.last().unwrap().display_name,
            "Cardpack 4"
        );

        // Request one less than the amount of items than are available.
        list_default_cardpacks_request = ListDefaultCardpacksRequest {
            page_size: 3,
            page_token: String::from(""),
        };
        response = cardpack_service
            .list_default_cardpacks(Request::new(list_default_cardpacks_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_cardpacks.len(), 3);
        assert_eq!(response.next_page_token.is_empty(), false);
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_cardpacks.first().unwrap().display_name,
            "Cardpack 1"
        );
        assert_eq!(
            response.default_cardpacks.last().unwrap().display_name,
            "Cardpack 3"
        );

        // Request more items than are available.
        list_default_cardpacks_request = ListDefaultCardpacksRequest {
            page_size: 50,
            page_token: String::from(""),
        };
        response = cardpack_service
            .list_default_cardpacks(Request::new(list_default_cardpacks_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_cardpacks.len(), 4);
        assert!(response.next_page_token.is_empty());
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_cardpacks.first().unwrap().display_name,
            "Cardpack 1"
        );
        assert_eq!(
            response.default_cardpacks.last().unwrap().display_name,
            "Cardpack 4"
        );

        // Can redeem page tokens.
        list_default_cardpacks_request = ListDefaultCardpacksRequest {
            page_size: 1,
            page_token: String::from(""),
        };
        response = cardpack_service
            .list_default_cardpacks(Request::new(list_default_cardpacks_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_cardpacks.len(), 1);
        assert_eq!(response.next_page_token.is_empty(), false);
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_cardpacks.first().unwrap().display_name,
            "Cardpack 1"
        );
        list_default_cardpacks_request.page_token = response.next_page_token;
        response = cardpack_service
            .list_default_cardpacks(Request::new(list_default_cardpacks_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_cardpacks.len(), 1);
        assert_eq!(response.next_page_token.is_empty(), false);
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_cardpacks.first().unwrap().display_name,
            "Cardpack 2"
        );
    }

    #[tokio::test]
    async fn list_default_black_cards() {
        // Test listing cards when no cards exist.
        let mut default_cardpack_data_list =
            DefaultCardpackData::create_list_from_raw_data(vec![(
                String::from("Cardpack"),
                Vec::new(),
                Vec::new(),
            )]);
        let mut pack_name = default_cardpack_data_list
            .first()
            .unwrap()
            .get_default_cardpack()
            .name
            .clone();
        let mut cardpack_service = get_local_test_cardpack_service_with_custom_default_cardpacks(
            DefaultCardpackHandler::new_with_custom_packs(default_cardpack_data_list),
        )
        .await;
        let mut list_default_black_cards_request = ListDefaultBlackCardsRequest {
            parent: pack_name.clone(),
            page_size: 50,
            page_token: String::from(""),
        };
        let mut response = cardpack_service
            .list_default_black_cards(Request::new(list_default_black_cards_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_black_cards.len(), 0);
        assert!(response.next_page_token.is_empty());
        assert_eq!(response.total_size, 0);

        // Add some default black cards.
        default_cardpack_data_list = DefaultCardpackData::create_list_from_raw_data(vec![(
            String::from("Cardpack"),
            vec![
                (String::from("Black Card 1"), 1),
                (String::from("Black Card 2"), 1),
                (String::from("Black Card 3"), 1),
                (String::from("Black Card 4"), 1),
            ],
            Vec::new(),
        )]);
        pack_name = default_cardpack_data_list
            .first()
            .unwrap()
            .get_default_cardpack()
            .name
            .clone();
        cardpack_service = get_local_test_cardpack_service_with_custom_default_cardpacks(
            DefaultCardpackHandler::new_with_custom_packs(default_cardpack_data_list),
        )
        .await;

        // Request exact amount of items than are available.
        list_default_black_cards_request = ListDefaultBlackCardsRequest {
            parent: pack_name.clone(),
            page_size: 4,
            page_token: String::from(""),
        };
        response = cardpack_service
            .list_default_black_cards(Request::new(list_default_black_cards_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_black_cards.len(), 4);
        assert!(response.next_page_token.is_empty());
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_black_cards.first().unwrap().text,
            "Black Card 1"
        );
        assert_eq!(
            response.default_black_cards.last().unwrap().text,
            "Black Card 4"
        );

        // Request one less than the amount of items than are available.
        list_default_black_cards_request = ListDefaultBlackCardsRequest {
            parent: pack_name.clone(),
            page_size: 3,
            page_token: String::from(""),
        };
        response = cardpack_service
            .list_default_black_cards(Request::new(list_default_black_cards_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_black_cards.len(), 3);
        assert_eq!(response.next_page_token.is_empty(), false);
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_black_cards.first().unwrap().text,
            "Black Card 1"
        );
        assert_eq!(
            response.default_black_cards.last().unwrap().text,
            "Black Card 3"
        );

        // Request more items than are available.
        list_default_black_cards_request = ListDefaultBlackCardsRequest {
            parent: pack_name.clone(),
            page_size: 50,
            page_token: String::from(""),
        };
        response = cardpack_service
            .list_default_black_cards(Request::new(list_default_black_cards_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_black_cards.len(), 4);
        assert!(response.next_page_token.is_empty());
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_black_cards.first().unwrap().text,
            "Black Card 1"
        );
        assert_eq!(
            response.default_black_cards.last().unwrap().text,
            "Black Card 4"
        );

        // Can redeem page tokens.
        list_default_black_cards_request = ListDefaultBlackCardsRequest {
            parent: pack_name,
            page_size: 1,
            page_token: String::from(""),
        };
        response = cardpack_service
            .list_default_black_cards(Request::new(list_default_black_cards_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_black_cards.len(), 1);
        assert_eq!(response.next_page_token.is_empty(), false);
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_black_cards.first().unwrap().text,
            "Black Card 1"
        );
        list_default_black_cards_request.page_token = response.next_page_token;
        response = cardpack_service
            .list_default_black_cards(Request::new(list_default_black_cards_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_black_cards.len(), 1);
        assert_eq!(response.next_page_token.is_empty(), false);
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_black_cards.first().unwrap().text,
            "Black Card 2"
        );
    }

    #[tokio::test]
    async fn list_default_white_cards() {
        // Test listing cards when no cards exist.
        let mut default_cardpack_data_list =
            DefaultCardpackData::create_list_from_raw_data(vec![(
                String::from("Cardpack"),
                Vec::new(),
                Vec::new(),
            )]);
        let mut pack_name = default_cardpack_data_list
            .first()
            .unwrap()
            .get_default_cardpack()
            .name
            .clone();
        let mut cardpack_service = get_local_test_cardpack_service_with_custom_default_cardpacks(
            DefaultCardpackHandler::new_with_custom_packs(default_cardpack_data_list),
        )
        .await;
        let mut list_default_white_cards_request = ListDefaultWhiteCardsRequest {
            parent: pack_name,
            page_size: 50,
            page_token: String::from(""),
        };
        let mut response = cardpack_service
            .list_default_white_cards(Request::new(list_default_white_cards_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_white_cards.len(), 0);
        assert!(response.next_page_token.is_empty());
        assert_eq!(response.total_size, 0);

        // Add some default white cards.
        default_cardpack_data_list = DefaultCardpackData::create_list_from_raw_data(vec![(
            String::from("Cardpack"),
            Vec::new(),
            vec![
                String::from("White Card 1"),
                String::from("White Card 2"),
                String::from("White Card 3"),
                String::from("White Card 4"),
            ],
        )]);
        pack_name = default_cardpack_data_list
            .first()
            .unwrap()
            .get_default_cardpack()
            .name
            .clone();
        cardpack_service = get_local_test_cardpack_service_with_custom_default_cardpacks(
            DefaultCardpackHandler::new_with_custom_packs(default_cardpack_data_list),
        )
        .await;

        // Request exact amount of items than are available.
        list_default_white_cards_request = ListDefaultWhiteCardsRequest {
            parent: pack_name.clone(),
            page_size: 4,
            page_token: String::from(""),
        };
        response = cardpack_service
            .list_default_white_cards(Request::new(list_default_white_cards_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_white_cards.len(), 4);
        assert!(response.next_page_token.is_empty());
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_white_cards.first().unwrap().text,
            "White Card 1"
        );
        assert_eq!(
            response.default_white_cards.last().unwrap().text,
            "White Card 4"
        );

        // Request one less than the amount of items than are available.
        list_default_white_cards_request = ListDefaultWhiteCardsRequest {
            parent: pack_name.clone(),
            page_size: 3,
            page_token: String::from(""),
        };
        response = cardpack_service
            .list_default_white_cards(Request::new(list_default_white_cards_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_white_cards.len(), 3);
        assert_eq!(response.next_page_token.is_empty(), false);
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_white_cards.first().unwrap().text,
            "White Card 1"
        );
        assert_eq!(
            response.default_white_cards.last().unwrap().text,
            "White Card 3"
        );

        // Request more items than are available.
        list_default_white_cards_request = ListDefaultWhiteCardsRequest {
            parent: pack_name.clone(),
            page_size: 50,
            page_token: String::from(""),
        };
        response = cardpack_service
            .list_default_white_cards(Request::new(list_default_white_cards_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_white_cards.len(), 4);
        assert!(response.next_page_token.is_empty());
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_white_cards.first().unwrap().text,
            "White Card 1"
        );
        assert_eq!(
            response.default_white_cards.last().unwrap().text,
            "White Card 4"
        );

        // Can redeem page tokens.
        list_default_white_cards_request = ListDefaultWhiteCardsRequest {
            parent: pack_name,
            page_size: 1,
            page_token: String::from(""),
        };
        response = cardpack_service
            .list_default_white_cards(Request::new(list_default_white_cards_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_white_cards.len(), 1);
        assert_eq!(response.next_page_token.is_empty(), false);
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_white_cards.first().unwrap().text,
            "White Card 1"
        );
        list_default_white_cards_request.page_token = response.next_page_token;
        response = cardpack_service
            .list_default_white_cards(Request::new(list_default_white_cards_request.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.default_white_cards.len(), 1);
        assert_eq!(response.next_page_token.is_empty(), false);
        assert_eq!(response.total_size, 4);
        assert_eq!(
            response.default_white_cards.first().unwrap().text,
            "White Card 2"
        );
    }
}
