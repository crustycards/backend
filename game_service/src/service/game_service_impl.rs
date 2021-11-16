use super::super::game::game::Game;
use super::super::game::game_indexer::GameIndexer;
use super::super::game::player_id::PlayerId;
use super::api_resource_fetcher::ApiResourceFetcher;
use crate::amqp::MessageQueue;
use clokwerk::{Interval, ScheduleHandle, Scheduler};
use shared::grpc_error::{
    empty_request_field_error, missing_request_field_error, negative_request_field_error,
};
use shared::proto::crusty_cards_api::{
    game_service_server::GameService, search_games_request::GameStageFilter,
    AddArtificialPlayerRequest, BanUserRequest, CreateChatMessageRequest, CreateGameRequest,
    GameInfo, GameView, GetGameViewRequest, JoinGameRequest, KickUserRequest, LeaveGameRequest,
    ListWhiteCardTextsRequest, ListWhiteCardTextsResponse, PlayCardsRequest,
    RemoveArtificialPlayerRequest, SearchGamesRequest, SearchGamesResponse, StartGameRequest,
    StopGameRequest, UnbanUserRequest, UnplayCardsRequest, VoteCardRequest,
    VoteStartNextRoundRequest,
};
use shared::proto::google::protobuf::Empty;
use shared::proto_validation::ValidatedGameConfig;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub struct GameServiceImpl {
    games: Arc<Mutex<GameIndexer>>,
    resource_fetcher: Box<dyn ApiResourceFetcher>,
    message_queue_or: Option<MessageQueue>,
    #[allow(dead_code)]
    // We only need the handle here to make sure that the recurring thread is dropped whenever this struct is dropped.
    schedule_handle: ScheduleHandle,
}

impl GameServiceImpl {
    pub fn new(
        resource_fetcher: Box<dyn ApiResourceFetcher>,
        message_queue_or: Option<MessageQueue>,
    ) -> GameServiceImpl {
        let games = Arc::new(Mutex::new(GameIndexer::new()));
        let games_scheduler_clone = games.clone();
        let mut scheduler = Scheduler::new();
        scheduler.every(Interval::Minutes(1)).run(move || {
            // Remove games that weren't used in the past 4 hours.
            games_scheduler_clone
                .clone()
                .lock()
                .unwrap()
                .remove_unused_games(Duration::from_secs(60 * 60 * 4));
        });
        let schedule_handle = scheduler.watch_thread(Duration::from_millis(100));
        GameServiceImpl {
            games,
            resource_fetcher,
            message_queue_or,
            schedule_handle,
        }
    }

    fn generate_game_id() -> String {
        Uuid::new_v4().to_simple().to_string()
    }

    fn try_send_amqp_game_update_message_to_users_in_game(&self, game: &Game) {
        match &self.message_queue_or {
            Some(message_queue) => {
                message_queue.game_updated_for_users(game.get_user_names_for_all_real_players());
            }
            None => {}
        };
    }
}

#[tonic::async_trait]
impl GameService for GameServiceImpl {
    async fn search_games(
        &self,
        request: Request<SearchGamesRequest>,
    ) -> Result<Response<SearchGamesResponse>, Status> {
        let game_stage_filter = match GameStageFilter::from_i32(request.get_ref().game_stage_filter)
        {
            Some(game_stage_filter) => {
                if game_stage_filter == GameStageFilter::Unspecified {
                    return Err(Status::invalid_argument(
                        "Request is missing GameStageFilter.",
                    ));
                }
                game_stage_filter
            }
            None => {
                return Err(Status::invalid_argument(
                    "Request contains invalid value for GameStageFilter.",
                ))
            }
        };

        if request.get_ref().min_available_player_slots < 0 {
            return Err(negative_request_field_error("min_available_player_slots"));
        }

        let mut game_info_list: Vec<GameInfo> = self
            .games
            .lock()
            .unwrap()
            .get_games_by_insert_time()
            .iter()
            .map(|game| game.get_game_info())
            .collect();

        game_info_list.retain(|game_info| match &game_info.config {
            Some(config) => {
                if !config.display_name.contains(&request.get_ref().query) {
                    return false;
                }

                let open_player_slots = config.max_players - game_info.player_count;
                if open_player_slots < request.get_ref().min_available_player_slots {
                    return false;
                }

                let passes_game_stage_filter = match game_stage_filter {
                    GameStageFilter::Unspecified => false,
                    GameStageFilter::FilterNone => true,
                    GameStageFilter::FilterStopped => !game_info.is_running,
                    GameStageFilter::FilterRunning => game_info.is_running,
                };
                if !passes_game_stage_filter {
                    return false;
                }

                true
            }
            None => false,
        });

        let response = SearchGamesResponse {
            games: game_info_list,
        };
        Ok(Response::new(response))
    }

    async fn create_game(
        &self,
        request: Request<CreateGameRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }

        let user_name = String::from(&request.get_ref().user_name);

        let game_config = match request.into_inner().game_config {
            Some(game_config) => game_config,
            None => return Err(missing_request_field_error("game_config")),
        };

        let validated_game_config = match ValidatedGameConfig::new(game_config) {
            Ok(validated_game_config) => validated_game_config,
            Err(err) => return Err(err),
        };

        let (black_cards, white_cards) = match self
            .resource_fetcher
            .get_custom_cards_from_multiple_custom_cardpacks(
                validated_game_config.get_custom_cardpack_names(),
            )
            .await
        {
            Ok(res) => res,
            Err(err) => return Err(err),
        };

        let (default_black_cards, default_white_cards) = match self
            .resource_fetcher
            .get_default_cards_from_multiple_default_cardpacks(
                validated_game_config.get_default_cardpack_names(),
            )
            .await
        {
            Ok(res) => res,
            Err(err) => return Err(err),
        };

        let user = match self.resource_fetcher.get_user(user_name.clone()).await {
            Ok(user) => user,
            Err(err) => return Err(err),
        };

        let mut games = self.games.lock().unwrap();
        if games
            .get_game_by_player_id(&PlayerId::RealUser(user_name.clone()))
            .is_some()
        {
            return Err(Status::invalid_argument(format!(
                "User {} is already in a game.",
                user_name
            )));
        }
        let game = match Game::new_with_owner(
            GameServiceImpl::generate_game_id(),
            validated_game_config,
            black_cards,
            white_cards,
            default_black_cards,
            default_white_cards,
            user,
        ) {
            Ok(game) => game,
            Err(err) => return Err(err),
        };
        let game_view = game.get_user_view(&user_name).unwrap();
        games.insert_game(game);
        return Ok(Response::new(game_view));
    }

    async fn start_game(
        &self,
        request: Request<StartGameRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }

        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        game.start(&request.get_ref().user_name)?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn stop_game(
        &self,
        request: Request<StopGameRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }

        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        game.stop(&request.get_ref().user_name)?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn join_game(
        &self,
        request: Request<JoinGameRequest>,
    ) -> Result<Response<GameView>, Status> {
        // TODO - If user is already in a game, then make them leave that game before joining the other one.
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }
        if request.get_ref().game_id.is_empty() {
            return Err(empty_request_field_error("game_id"));
        }

        let user = match self
            .resource_fetcher
            .get_user(String::from(&request.get_ref().user_name))
            .await
        {
            Ok(user) => user,
            Err(err) => return Err(err),
        };
        let mut games = self.games.lock().unwrap();
        if games
            .get_game_by_player_id(&PlayerId::RealUser(String::from(
                &request.get_ref().user_name,
            )))
            .is_some()
        {
            return Err(Status::invalid_argument("User is already in a game."));
        }
        let game = match games.get_game_by_game_id(&request.get_ref().game_id) {
            Some(game) => game,
            None => {
                return Err(Status::invalid_argument(format!(
                    "Game does not exist with id: `{}`.",
                    request.get_ref().game_id
                )))
            }
        };
        game.join(user)?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn leave_game(
        &self,
        request: Request<LeaveGameRequest>,
    ) -> Result<Response<Empty>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }

        let mut games = self.games.lock().unwrap();
        let (game_id, game_is_empty) = {
            let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
                &request.get_ref().user_name,
            ))) {
                Some(game) => game,
                None => return Err(Status::invalid_argument("User is not in a game.")),
            };
            game.leave(&request.get_ref().user_name)?;
            self.try_send_amqp_game_update_message_to_users_in_game(game);
            (String::from(game.get_game_id()), game.is_empty())
        };
        if game_is_empty {
            games.remove_game(&game_id);
        }
        Ok(Response::new(Empty {}))
    }

    async fn kick_user(
        &self,
        request: Request<KickUserRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }
        if request.get_ref().troll_user_name.is_empty() {
            return Err(empty_request_field_error("troll_user_name"));
        }

        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        game.kick_user(
            &request.get_ref().user_name,
            &request.get_ref().troll_user_name,
        )?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn ban_user(
        &self,
        request: Request<BanUserRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }
        if request.get_ref().troll_user_name.is_empty() {
            return Err(empty_request_field_error("troll_user_name"));
        }

        let troll_user = match self
            .resource_fetcher
            .get_user(String::from(&request.get_ref().troll_user_name))
            .await
        {
            Ok(user) => user,
            Err(err) => return Err(err),
        };
        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        game.ban_user(&request.get_ref().user_name, troll_user)?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn unban_user(
        &self,
        request: Request<UnbanUserRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }
        if request.get_ref().troll_user_name.is_empty() {
            return Err(empty_request_field_error("troll_user_name"));
        }

        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        game.unban_user(
            &request.get_ref().user_name,
            &request.get_ref().troll_user_name,
        )?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn play_cards(
        &self,
        request: Request<PlayCardsRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }

        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        game.play_cards(&request.get_ref().user_name, &request.get_ref().cards)?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn unplay_cards(
        &self,
        request: Request<UnplayCardsRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }

        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        game.unplay_cards(&request.get_ref().user_name)?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn vote_card(
        &self,
        request: Request<VoteCardRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }
        if request.get_ref().choice == 0 {
            return Err(empty_request_field_error("choice"));
        }
        if request.get_ref().choice < 0 {
            return Err(negative_request_field_error("choice"));
        }

        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        game.vote_card(&request.get_ref().user_name, request.get_ref().choice)?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn vote_start_next_round(
        &self,
        request: Request<VoteStartNextRoundRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }

        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        game.vote_start_next_round(&request.get_ref().user_name)?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn add_artificial_player(
        &self,
        request: Request<AddArtificialPlayerRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }

        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        game.add_artificial_player(
            &request.get_ref().user_name,
            String::from(&request.get_ref().display_name),
        )?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn remove_artificial_player(
        &self,
        request: Request<RemoveArtificialPlayerRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }

        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        game.remove_artificial_player(
            &request.get_ref().user_name,
            &request.get_ref().artificial_player_id,
        )?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn create_chat_message(
        &self,
        request: Request<CreateChatMessageRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }
        let chat_message = match &request.get_ref().chat_message {
            Some(chat_message) => chat_message,
            None => return Err(missing_request_field_error("chat_message")),
        };
        if chat_message.text.is_empty() {
            return Err(empty_request_field_error("chat_message.text"));
        }

        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        game.post_message(
            &request.get_ref().user_name,
            String::from(&chat_message.text),
        )?;
        self.try_send_amqp_game_update_message_to_users_in_game(game);
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    async fn get_game_view(
        &self,
        request: Request<GetGameViewRequest>,
    ) -> Result<Response<GameView>, Status> {
        if request.get_ref().user_name.is_empty() {
            return Err(empty_request_field_error("user_name"));
        }

        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_player_id(&PlayerId::RealUser(String::from(
            &request.get_ref().user_name,
        ))) {
            Some(game) => game,
            None => return Err(Status::invalid_argument("User is not in a game.")),
        };
        match game.get_user_view(&request.get_ref().user_name) {
            Ok(game_view) => Ok(Response::new(game_view)),
            Err(err) => Err(err),
        }
    }

    // TODO - Test this.
    async fn list_white_card_texts(
        &self,
        request: Request<ListWhiteCardTextsRequest>,
    ) -> Result<Response<ListWhiteCardTextsResponse>, Status> {
        let mut games = self.games.lock().unwrap();
        let game = match games.get_game_by_game_id(&request.get_ref().game_id) {
            Some(game) => game,
            None => return Err(Status::not_found("Game does not exist.")),
        };
        // TODO - We should make the page tokens for this RPC opaque. Right now it's just a stringified index.
        let skip = request.get_ref().page_token.parse::<usize>().unwrap();
        let (card_texts, has_next_page, total_size) = game.search_white_card_texts(
            request.get_ref().page_size as usize,
            skip,
            &request.get_ref().filter,
        );
        let next_page_token = if has_next_page {
            format!("{}", skip + request.get_ref().page_size as usize)
        } else {
            "".to_string()
        };
        Ok(Response::new(ListWhiteCardTextsResponse {
            card_texts,
            next_page_token,
            total_size: total_size as i64,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::super::api_resource_fetcher::MockApiResourceFetcher;
    use super::*;
    use shared::proto::crusty_cards_api::{
        CustomBlackCard, CustomWhiteCard, DefaultBlackCard, DefaultWhiteCard, User,
    };
    use shared::test_helper::get_valid_test_game_config;

    // The GameView proto contains fields such as player join_time that will be different for every test run.
    // This function validates that these fields contain some value and then removes them to allow for consistent
    // results in tests.
    fn validate_and_remove_changing_parameters_from_game_view(mut game_view: GameView) -> GameView {
        // TODO - Find a way to combine these two `for` loops. I've tried using `iter.chain` but that causes a second mutable borrow.
        for player in game_view.players.iter_mut() {
            assert_eq!(player.join_time.is_some(), true);
            player.join_time = None;
        }
        for player in game_view.queued_players.iter_mut() {
            assert_eq!(player.join_time.is_some(), true);
            player.join_time = None;
        }

        assert_eq!(game_view.create_time.is_some(), true);
        game_view.create_time = None;
        assert_eq!(game_view.last_activity_time.is_some(), true);
        game_view.last_activity_time = None;

        assert_eq!(game_view.game_id.is_empty(), false);
        game_view.game_id.clear();

        game_view
    }

    fn create_empty_user() -> User {
        User {
            name: String::from(""),
            display_name: String::from(""),
            create_time: None,
            update_time: None,
        }
    }

    fn create_empty_custom_black_card() -> CustomBlackCard {
        CustomBlackCard {
            name: String::from(""),
            text: String::from(""),
            answer_fields: 0,
            create_time: None,
            update_time: None,
            delete_time: None,
        }
    }

    fn create_empty_custom_white_card() -> CustomWhiteCard {
        CustomWhiteCard {
            name: String::from(""),
            text: String::from(""),
            create_time: None,
            update_time: None,
            delete_time: None,
        }
    }

    fn create_empty_default_black_card() -> DefaultBlackCard {
        DefaultBlackCard {
            name: String::from(""),
            text: String::from(""),
            answer_fields: 0,
        }
    }

    fn create_empty_default_white_card() -> DefaultWhiteCard {
        DefaultWhiteCard {
            name: String::from(""),
            text: String::from(""),
        }
    }

    #[tokio::test]
    async fn create_game() {
        let mut mock_api_resource_fetcher = MockApiResourceFetcher::new();
        mock_api_resource_fetcher
            .expect_get_user()
            .return_once(move |_| Ok(create_empty_user()));
        mock_api_resource_fetcher
            .expect_get_custom_cards_from_multiple_custom_cardpacks()
            .return_once(move |_| {
                Ok((
                    vec![create_empty_custom_black_card()],
                    vec![create_empty_custom_white_card()],
                ))
            });
        mock_api_resource_fetcher
            .expect_get_default_cards_from_multiple_default_cardpacks()
            .return_once(move |_| {
                Ok((
                    vec![create_empty_default_black_card()],
                    vec![create_empty_default_white_card()],
                ))
            });
        let game_service_impl = GameServiceImpl::new(Box::from(mock_api_resource_fetcher), None);

        let mut create_game_request = CreateGameRequest {
            user_name: String::from(""),
            game_config: None,
        };
        let mut game_view_or: Result<Response<GameView>, Status> = game_service_impl
            .create_game(Request::new(create_game_request.clone()))
            .await;
        assert_eq!(
            format!("{}", game_view_or.err().unwrap()),
            "status: InvalidArgument, message: \"Request field `user_name` must not be blank.\", details: [], metadata: MetadataMap { headers: {} }"
        );
        create_game_request.user_name = String::from("test_user_name");
        game_view_or = game_service_impl
            .create_game(Request::new(create_game_request.clone()))
            .await;
        assert_eq!(
            format!("{}", game_view_or.err().unwrap()),
            "status: InvalidArgument, message: \"Request is missing required field `game_config`.\", details: [], metadata: MetadataMap { headers: {} }"
        );
        create_game_request.game_config = Some(get_valid_test_game_config());
        game_view_or = game_service_impl
            .create_game(Request::new(create_game_request))
            .await;
        assert_eq!(game_view_or.is_ok(), true);
        assert_eq!(format!("{:?}", validate_and_remove_changing_parameters_from_game_view(game_view_or.unwrap().into_inner())), "GameView { game_id: \"\", config: Some(GameConfig { display_name: \"Test Game\", max_players: 3, hand_size: 3, custom_cardpack_names: [\"test_custom_cardpack_name\"], default_cardpack_names: [\"test_default_cardpack_name\"], blank_white_card_config: Some(BlankWhiteCardConfig { behavior: Disabled, blank_white_cards_added: None }), end_condition: Some(EndlessMode(Empty)) }), stage: NotRunning, hand: [], players: [Player { score: 0, join_time: None, identifier: Some(User(User { name: \"\", display_name: \"\", create_time: None, update_time: None })) }], queued_players: [], banned_users: [], judge: None, owner: Some(User { name: \"\", display_name: \"\", create_time: None, update_time: None }), white_played: [], current_black_card: None, winner: None, chat_messages: [], past_rounds: [], create_time: None, last_activity_time: None }");
    }

    #[tokio::test]
    async fn search_games() {
        let mut mock_api_resource_fetcher = MockApiResourceFetcher::new();
        let user = User {
            name: String::from("owner"),
            display_name: String::from(""),
            create_time: None,
            update_time: None,
        };
        mock_api_resource_fetcher
            .expect_get_user()
            .return_once(move |_| Ok(user));
        mock_api_resource_fetcher
            .expect_get_custom_cards_from_multiple_custom_cardpacks()
            .return_once(move |_| {
                Ok((
                    vec![create_empty_custom_black_card()],
                    vec![create_empty_custom_white_card()],
                ))
            });
        mock_api_resource_fetcher
            .expect_get_default_cards_from_multiple_default_cardpacks()
            .return_once(move |_| {
                Ok((
                    vec![create_empty_default_black_card()],
                    vec![create_empty_default_white_card()],
                ))
            });
        let game_service_impl = GameServiceImpl::new(Box::from(mock_api_resource_fetcher), None);

        // Should not contain any games on intialization.
        let mut search_games_request = SearchGamesRequest {
            query: String::from(""),
            min_available_player_slots: 0,
            game_stage_filter: GameStageFilter::Unspecified.into(),
        };
        let search_games_response_or: Result<Response<SearchGamesResponse>, Status> =
            game_service_impl
                .search_games(Request::new(search_games_request.clone()))
                .await;
        assert_eq!(
            format!("{}", search_games_response_or.err().unwrap()),
            "status: InvalidArgument, message: \"Request is missing GameStageFilter.\", details: [], metadata: MetadataMap { headers: {} }"
        );
        search_games_request.set_game_stage_filter(GameStageFilter::FilterNone);
        let search_games_response = game_service_impl
            .search_games(Request::new(search_games_request))
            .await
            .unwrap();
        assert_eq!(search_games_response.get_ref().games.len(), 0);

        // Create game.
        let create_game_request = CreateGameRequest {
            user_name: String::from("test_user_name"),
            game_config: Some(get_valid_test_game_config()),
        };
        let game_view_or = game_service_impl
            .create_game(Request::new(create_game_request))
            .await;
        assert_eq!(game_view_or.is_ok(), true);

        // Should contain 1 game.
        let mut search_games_request = SearchGamesRequest {
            query: String::from(""),
            min_available_player_slots: 0,
            game_stage_filter: GameStageFilter::Unspecified.into(),
        };
        let search_games_response_or: Result<Response<SearchGamesResponse>, Status> =
            game_service_impl
                .search_games(Request::new(search_games_request.clone()))
                .await;
        assert_eq!(
            format!("{}", search_games_response_or.err().unwrap()),
            "status: InvalidArgument, message: \"Request is missing GameStageFilter.\", details: [], metadata: MetadataMap { headers: {} }"
        );
        search_games_request.set_game_stage_filter(GameStageFilter::FilterNone);
        let search_games_response = game_service_impl
            .search_games(Request::new(search_games_request))
            .await
            .unwrap();
        assert_eq!(search_games_response.get_ref().games.len(), 1);
        let mut game_info: GameInfo = search_games_response
            .get_ref()
            .games
            .first()
            .unwrap()
            .clone();
        assert_eq!(game_info.game_id.is_empty(), false);
        assert_eq!(game_info.create_time.is_some(), true);
        assert_eq!(game_info.last_activity_time.is_some(), true);
        game_info.game_id.clear();
        game_info.create_time = None;
        game_info.last_activity_time = None;
        assert_eq!(format!("{:?}", game_info), "GameInfo { game_id: \"\", config: Some(GameConfig { display_name: \"Test Game\", max_players: 3, hand_size: 3, custom_cardpack_names: [\"test_custom_cardpack_name\"], default_cardpack_names: [\"test_default_cardpack_name\"], blank_white_card_config: Some(BlankWhiteCardConfig { behavior: Disabled, blank_white_cards_added: None }), end_condition: Some(EndlessMode(Empty)) }), player_count: 1, owner: Some(User { name: \"owner\", display_name: \"\", create_time: None, update_time: None }), is_running: false, create_time: None, last_activity_time: None }");
    }
}
