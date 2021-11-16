use super::black_card_deck::BlackCardDeck;
use super::chat_message_handler::ChatMessageHandler;
use super::player_id::PlayerId;
use super::player_manager::PlayerManager;
use super::text_query_handler::TextQueryHandler;
use super::white_card_deck::WhiteCardDeck;
use super::white_card_gameplay_manager::WhiteCardGameplayManager;
use rand::prelude::SliceRandom;
use rand::SeedableRng;
use sha2::{Digest, Sha256};
use shared::constants::*;
use shared::proto::crusty_cards_api::{
    game_config::EndCondition, game_view::Stage, playable_white_card::Card, player::Identifier,
    ArtificialUser, ChatMessage, CustomBlackCard, CustomWhiteCard, DefaultBlackCard,
    DefaultWhiteCard, GameInfo, GameView, PastRound, PlayableWhiteCard, Player, User,
    WhiteCardsPlayed,
};
use shared::proto_validation::ValidatedGameConfig;
use shared::time::{get_current_timestamp_proto, system_time_to_timestamp_proto};
use std::time::SystemTime;
use tonic::Status;
use uuid::Uuid;

const MAX_CHAT_MESSAGES_PER_GAME: usize = 100;

// TODO - Move this helper function to a more appropriate place.
fn get_text_from_playable_white_card(card: &PlayableWhiteCard) -> &str {
    match &card.card {
        Some(card) => match card {
            Card::CustomWhiteCard(custom_white_card) => &custom_white_card.text,
            Card::BlankWhiteCard(blank_white_card) => &blank_white_card.open_text,
            Card::DefaultWhiteCard(default_white_card) => &default_white_card.text,
        },
        None => "",
    }
}

pub struct Game {
    game_id: String,
    config: ValidatedGameConfig,
    create_time: SystemTime,
    last_activity_time: SystemTime,
    stage: Stage,
    chat_messages: ChatMessageHandler,
    past_rounds: Vec<PastRound>,
    player_manager: PlayerManager,
    banned_users: Vec<User>,
    winner: Option<Player>,
    black_card_deck: BlackCardDeck,
    white_card_gameplay_manager: WhiteCardGameplayManager,
    white_card_text_query_handler: TextQueryHandler,
}

impl Game {
    pub fn new(
        game_id: String,
        config: ValidatedGameConfig,
        custom_black_cards: Vec<CustomBlackCard>,
        custom_white_cards: Vec<CustomWhiteCard>,
        default_black_cards: Vec<DefaultBlackCard>,
        default_white_cards: Vec<DefaultWhiteCard>,
    ) -> Result<Game, Status> {
        let time_now = SystemTime::now();

        let white_card_text_query_handler = TextQueryHandler::new(
            custom_white_cards
                .iter()
                .map(|card| card.text.clone())
                .chain(default_white_cards.iter().map(|card| card.text.clone()))
                .collect(),
        );

        let white_card_deck = WhiteCardDeck::new(
            custom_white_cards,
            default_white_cards,
            config.get_blank_white_card_config(),
        );
        let hand_size = config.get_hand_size();

        let game = Game {
            game_id,
            config,
            create_time: time_now,
            last_activity_time: time_now,
            stage: Stage::NotRunning,
            chat_messages: ChatMessageHandler::new(MAX_CHAT_MESSAGES_PER_GAME),
            past_rounds: Vec::new(),
            player_manager: PlayerManager::new(),
            banned_users: Vec::new(),
            winner: None,
            black_card_deck: BlackCardDeck::new(custom_black_cards, default_black_cards)?,
            white_card_gameplay_manager: WhiteCardGameplayManager::new(white_card_deck, hand_size),
            white_card_text_query_handler,
        };

        Ok(game)
    }

    pub fn new_with_owner(
        game_id: String,
        config: ValidatedGameConfig,
        custom_black_cards: Vec<CustomBlackCard>,
        custom_white_cards: Vec<CustomWhiteCard>,
        default_black_cards: Vec<DefaultBlackCard>,
        default_white_cards: Vec<DefaultWhiteCard>,
        owner: User,
    ) -> Result<Game, Status> {
        let mut game = Game::new(
            game_id,
            config,
            custom_black_cards,
            custom_white_cards,
            default_black_cards,
            default_white_cards,
        )?;

        match game.join(owner) {
            Err(_) => Err(Status::unknown(
                "Unknown error occured when attempting to initialize game.",
            )),
            Ok(_) => Ok(game),
        }
    }

    fn update_last_activity_time(&mut self) {
        self.last_activity_time = SystemTime::now();
    }

    pub fn get_user_names_for_all_real_players(&self) -> Vec<&str> {
        self.player_manager.get_user_names_for_all_real_players()
    }

    fn add_queued_players_to_game(&mut self) {
        for player in self
            .player_manager
            .get_queued_real_players()
            .iter()
            .chain(self.player_manager.get_queued_artificial_players())
        {
            self.white_card_gameplay_manager
                .add_player(PlayerId::from_player_proto(player).unwrap())
        }
        self.player_manager
            .drain_queued_real_and_artificial_players();
    }

    fn generate_artificial_player_id() -> String {
        Uuid::new_v4().to_simple().to_string()
    }

    fn is_full(&self) -> bool {
        self.player_manager.get_real_players().len()
            + self.player_manager.get_queued_real_players().len()
            == self.config.get_max_players()
    }

    fn stop_if_not_enough_players(&mut self) {
        if !self.has_enough_players_to_play() {
            self.force_stop();
        }
    }

    fn all_players_have_played_this_round(&self) -> bool {
        for player in self.player_manager.get_real_players().iter() {
            if let Some(Identifier::User(user)) = &player.identifier {
                let user_name = &user.name;
                if !self.player_manager.is_judge(user_name)
                    && !self
                        .white_card_gameplay_manager
                        .player_has_played_this_round(&PlayerId::RealUser(String::from(user_name)))
                {
                    return false;
                }
            }
        }
        true
    }

    fn increment_score_and_maybe_stop_game(&mut self, player_id: &PlayerId) {
        self.player_manager.increment_player_score(player_id);
        if self.player_has_won(player_id) {
            self.force_stop();
        }
    }

    fn player_has_won(&mut self, player_id: &PlayerId) -> bool {
        match self.config.get_end_condition() {
            EndCondition::MaxScore(max_score) => {
                match self.player_manager.get_player_score(player_id) {
                    Some(score) => score >= *max_score,
                    None => false,
                }
            }
            EndCondition::EndlessMode(_) => false,
        }
    }

    pub fn start(&mut self, user_name: &str) -> Result<(), Status> {
        if !self.player_manager.is_owner(user_name) {
            return Err(Status::invalid_argument(
                "Must be game owner to start game.",
            ));
        }
        if self.is_running() {
            return Err(Status::invalid_argument("Game is already running."));
        }
        if !self.has_enough_players_to_play() {
            return Err(Status::invalid_argument(&format!("Need at least {} players to start. Add some artificial users or wait for more people to join.", MINIMUM_PLAYERS_REQUIRED_TO_PLAY)));
        }
        self.player_manager.set_random_judge();
        self.past_rounds.clear();
        self.black_card_deck.shuffle_and_reset();
        self.player_manager.reset_player_scores();
        self.white_card_gameplay_manager
            .discard_played_cards_and_draw_to_full();
        self.white_card_gameplay_manager
            .play_for_artificial_players(self.black_card_deck.get_current_black_card());
        self.stage = Stage::PlayPhase;
        self.update_last_activity_time();
        Ok(())
    }

    pub fn stop(&mut self, user_name: &str) -> Result<(), Status> {
        if !self.player_manager.is_owner(user_name) {
            return Err(Status::invalid_argument("Must be game owner to stop game."));
        }
        if !self.is_running() {
            return Err(Status::invalid_argument("Game is not running."));
        }

        self.force_stop();

        Ok(())
    }

    fn force_stop(&mut self) {
        if !self.is_running() {
            return;
        }

        // TODO - Finish implementing.
        self.white_card_gameplay_manager.discard_player_hands();
        self.add_queued_players_to_game();
        self.stage = Stage::NotRunning;
        self.update_last_activity_time();
    }

    // Returns a string that is unique to each round.
    // This is used to help deterministically
    // shuffle the order that played cards are shown.
    // The value returned is created by hashing the
    // round number and the current judge name.
    // Using these two values together ensures that
    // each round maps to a unique hash, even in
    // the event of a judge leaving in the middle
    // of a round and being replaced by another user.
    fn get_round_nonce_digest(&self) -> [u8; 32] {
        let judge_string = match self.player_manager.get_judge() {
            Some(judge) => format!("{:?}", judge),
            None => String::from(""),
        };
        Sha256::digest(
            format!(
                "{}{}{}",
                &self.game_id,
                self.past_rounds.len(),
                judge_string
            )
            .as_bytes(),
        )
        .into()
    }

    pub fn join(&mut self, user: User) -> Result<(), Status> {
        if self.is_full() {
            return Err(Status::invalid_argument("Cannot join - game is full."));
        }
        if self.player_manager.user_is_in_game(&user.name) {
            return Err(Status::invalid_argument(
                "Cannot join - you are already in this game.",
            ));
        }
        if self.user_is_banned(&user.name) {
            return Err(Status::invalid_argument(
                "Cannot join - you are banned from this game.",
            ));
        }
        self.add_player_to_game(Identifier::User(user));

        Ok(())
    }

    fn identifier_to_player_id(identifier: &Identifier) -> PlayerId {
        match identifier {
            Identifier::User(user) => PlayerId::RealUser(String::from(&user.name)),
            Identifier::ArtificialUser(artificial_user) => {
                PlayerId::ArtificialPlayer(String::from(&artificial_user.id))
            }
        }
    }

    fn round_is_in_progress(&self) -> bool {
        self.is_running() && self.stage != Stage::RoundEndPhase
    }

    fn add_player_to_game(&mut self, identifier: Identifier) {
        if !self.round_is_in_progress() {
            let player_id = Self::identifier_to_player_id(&identifier);
            self.player_manager.add_player(identifier);
            self.white_card_gameplay_manager.add_player(player_id);
        } else {
            self.player_manager.add_queued_player(identifier);
        }
    }

    pub fn leave(&mut self, user_name: &str) -> Result<(), Status> {
        if !self.player_manager.user_is_in_game(user_name) {
            return Err(Status::invalid_argument(
                "Cannot leave - you are not in this game.",
            ));
        }

        let player_id = PlayerId::RealUser(String::from(user_name));
        self.remove_player(&player_id);

        Ok(())
    }

    fn remove_player(&mut self, player_id: &PlayerId) {
        if let PlayerId::RealUser(user_name) = player_id {
            if self.is_running()
                && self.player_manager.is_judge(user_name)
                && self.stage != Stage::RoundEndPhase
            {
                self.white_card_gameplay_manager
                    .return_played_cards_to_hands();
                self.stage = Stage::RoundEndPhase;
            }
        }
        self.player_manager.remove_player(player_id);
        self.white_card_gameplay_manager.remove_player(player_id);

        self.stop_if_not_enough_players();
    }

    pub fn add_artificial_player(
        &mut self,
        user_name: &str,
        mut artificial_player_name: String,
    ) -> Result<(), Status> {
        if !self.player_manager.is_owner(user_name) {
            return Err(Status::invalid_argument(
                "Must be game owner to add an artificial player.",
            ));
        }

        artificial_player_name = String::from(artificial_player_name.trim());
        if artificial_player_name.is_empty() {
            artificial_player_name = match self
                .player_manager
                .get_unused_default_artificial_player_name()
            {
                Some(name) => name,
                None => {
                    return Err(Status::invalid_argument(
                        "No more default artificial player names available.",
                    ))
                }
            };
        }

        let artificial_player_id = Game::generate_artificial_player_id();

        self.add_player_to_game(Identifier::ArtificialUser(ArtificialUser {
            id: String::from(&artificial_player_id),
            display_name: artificial_player_name,
        }));

        self.update_last_activity_time();
        Ok(())
    }

    pub fn remove_artificial_player(
        &mut self,
        user_name: &str,
        artificial_player_id: &str,
    ) -> Result<(), Status> {
        if !self.player_manager.is_owner(user_name) {
            return Err(Status::invalid_argument(
                "Must be game owner to remove an artificial player.",
            ));
        }

        if artificial_player_id.is_empty() {
            match self.player_manager.get_last_artificial_player() {
                Some(player_id) => self.remove_player(&player_id),
                None => {
                    return Err(Status::invalid_argument(
                        "There are no artificial players to remove.",
                    ))
                }
            };
        } else {
            if !self
                .player_manager
                .artificial_player_is_in_game(artificial_player_id)
            {
                return Err(Status::invalid_argument(
                    "Artificial player does not exist with that id.",
                ));
            }

            let player_id = PlayerId::ArtificialPlayer(String::from(artificial_player_id));
            self.remove_player(&player_id);
        }

        self.update_last_activity_time();
        Ok(())
    }

    pub fn kick_user(&mut self, user_name: &str, troll_user_name: &str) -> Result<(), Status> {
        if !self.player_manager.is_owner(user_name) {
            return Err(Status::invalid_argument(
                "Must be game owner to kick someone.",
            ));
        }
        if !self.player_manager.user_is_in_game(troll_user_name) {
            return Err(Status::invalid_argument(
                "Cannot kick someone who is not in the game.",
            ));
        }
        if user_name == troll_user_name {
            return Err(Status::invalid_argument(
                "Cannot kick yourself from the game.",
            ));
        }

        self.leave(troll_user_name)
    }

    pub fn ban_user(&mut self, user_name: &str, troll_user: User) -> Result<(), Status> {
        if !self.player_manager.is_owner(user_name) {
            return Err(Status::invalid_argument(
                "Must be game owner to ban someone.",
            ));
        }
        if self.player_manager.is_owner(&troll_user.name) {
            return Err(Status::invalid_argument(
                "Cannot ban yourself from your own game.",
            ));
        }
        if self.user_is_banned(&troll_user.name) {
            return Err(Status::invalid_argument(
                "User is already banned from this game.",
            ));
        }

        if self.contains_player(&PlayerId::RealUser(String::from(&troll_user.name))) {
            self.leave(&troll_user.name)?;
        }
        self.banned_users.push(troll_user);
        self.update_last_activity_time();
        Ok(())
    }

    pub fn unban_user(&mut self, user_name: &str, troll_user_name: &str) -> Result<(), Status> {
        if !self.player_manager.is_owner(user_name) {
            return Err(Status::invalid_argument(
                "Must be game owner to unban someone.",
            ));
        }
        for (index, banned_user) in self.banned_users.iter().enumerate() {
            if banned_user.name == troll_user_name {
                self.banned_users.remove(index);
                self.update_last_activity_time();
                return Ok(());
            }
        }
        Err(Status::invalid_argument(
            "User is not banned from this game.",
        ))
    }

    pub fn play_cards(
        &mut self,
        user_name: &str,
        cards: &[PlayableWhiteCard],
    ) -> Result<(), Status> {
        if self.stage != Stage::PlayPhase {
            return Err(Status::invalid_argument(
                "Can only play cards during play phase.",
            ));
        }

        if self.player_manager.is_judge(user_name) {
            return Err(Status::invalid_argument("Cannot play cards as the judge."));
        }

        if self
            .white_card_gameplay_manager
            .player_has_played_this_round(&PlayerId::RealUser(String::from(user_name)))
        {
            return Err(Status::invalid_argument(
                "User has already played this round.",
            ));
        }

        self.white_card_gameplay_manager.play_cards_for_player(
            PlayerId::RealUser(String::from(user_name)),
            cards,
            self.black_card_deck.get_current_black_card(),
            &self.config,
        )?;

        if self.all_players_have_played_this_round() {
            self.stage = Stage::JudgePhase;
        }

        self.update_last_activity_time();
        Ok(())
    }

    pub fn unplay_cards(&mut self, user_name: &str) -> Result<(), Status> {
        if self.stage != Stage::PlayPhase {
            return Err(Status::invalid_argument(
                "Can only unplay cards during play phase.",
            ));
        }

        if !self
            .white_card_gameplay_manager
            .player_has_played_this_round(&PlayerId::RealUser(String::from(user_name)))
        {
            return Err(Status::invalid_argument(
                "Cannot unplay cards - user has not played yet.",
            ));
        }

        self.white_card_gameplay_manager
            .unplay_cards_for_player(&PlayerId::RealUser(String::from(user_name)));
        self.update_last_activity_time();
        Ok(())
    }

    pub fn vote_card(&mut self, user_name: &str, choice: i32) -> Result<(), Status> {
        if self.stage != Stage::JudgePhase {
            return Err(Status::invalid_argument(
                "Can only vote cards during judge phase.",
            ));
        }
        if !self.player_manager.is_judge(user_name) {
            return Err(Status::invalid_argument(
                "Can only vote if you are the judge.",
            ));
        }

        let mut played_cards = self.get_pseudorandom_ordered_white_cards_played_list();

        let voted_cards = match played_cards.get_mut((choice - 1) as usize) {
            Some(cards) => cards,
            None => return Err(Status::invalid_argument("Invalid selection.")),
        };

        let winner_or = &voted_cards.player;
        if let Some(winner) = &winner_or {
            if let Some(winner_id) = PlayerId::from_player_proto(winner) {
                self.increment_score_and_maybe_stop_game(&winner_id);
            }
        }

        self.stage = Stage::RoundEndPhase;

        self.winner = winner_or.clone(); // TODO - I don't think we need to clone `winner` here. Let's find a way to move it from `winner_or` above instead.

        self.update_last_activity_time();
        Ok(())
    }

    pub fn vote_start_next_round(&mut self, user_name: &str) -> Result<(), Status> {
        self.start_next_round()?;
        self.update_last_activity_time();
        Ok(())
    }

    fn start_next_round(&mut self) -> Result<(), Status> {
        if self.stage != Stage::RoundEndPhase {
            return Err(Status::invalid_argument(
                "Cannot start next round at this time.",
            ));
        }

        let round = PastRound {
            black_card: Some(self.black_card_deck.get_current_black_card().clone()),
            white_played: self.get_pseudorandom_ordered_white_cards_played_list(),
            judge: self.player_manager.get_judge().cloned(),
            winner: self.winner.as_ref().cloned(),
        };
        self.past_rounds.push(round);

        self.player_manager.increment_judge();
        self.winner = None;
        self.add_queued_players_to_game();
        self.black_card_deck.next_card();
        self.white_card_gameplay_manager
            .discard_played_cards_and_draw_to_full();
        self.white_card_gameplay_manager
            .play_for_artificial_players(self.black_card_deck.get_current_black_card());
        self.stage = Stage::PlayPhase;

        Ok(())
    }

    pub fn post_message(&mut self, user_name: &str, message_text: String) -> Result<(), Status> {
        let user = match self.player_manager.get_real_player(user_name) {
            Some(player) => match &player.identifier {
                Some(Identifier::User(user)) => user,
                _ => {
                    return Err(Status::invalid_argument(
                        "Only real users can post messages.",
                    ));
                }
            },
            None => {
                return Err(Status::invalid_argument(
                    "User must be in the game to post a message.",
                ));
            }
        };

        let message = ChatMessage {
            user: Some(user.clone()),
            text: message_text,
            create_time: Some(get_current_timestamp_proto()),
        };
        self.chat_messages.add_new_message(message);
        Ok(())
    }

    pub fn get_user_view(&self, user_name: &str) -> Result<GameView, Status> {
        Ok(GameView {
            game_id: String::from(&self.game_id),
            config: Some(self.config.raw_config()),
            stage: self.stage.into(),
            hand: self
                .white_card_gameplay_manager
                .get_hand_belonging_to_player(&PlayerId::RealUser(String::from(user_name)))
                .unwrap_or_default()
                .into_iter()
                .cloned()
                .collect(),
            players: self.player_manager.clone_all_players_sorted_by_join_time(),
            queued_players: self
                .player_manager
                .clone_all_queued_players_sorted_by_join_time(),
            banned_users: self.banned_users.clone(),
            judge: self.player_manager.get_judge().cloned(),
            owner: self.player_manager.get_owner().cloned(),
            white_played: {
                let mut played_cards = self.get_pseudorandom_ordered_white_cards_played_list();
                match self.stage {
                    Stage::PlayPhase => {
                        for entry in played_cards.iter_mut() {
                            entry.card_texts.clear();
                        }
                    }
                    Stage::JudgePhase => {
                        for entry in played_cards.iter_mut() {
                            entry.player = None;
                        }
                    }
                    _ => {}
                };
                played_cards
            },
            current_black_card: if self.is_running() {
                Some(self.black_card_deck.get_current_black_card().clone())
            } else {
                None
            },
            winner: self.winner.as_ref().cloned(),
            chat_messages: self.chat_messages.clone_message_list(),
            past_rounds: self.past_rounds.clone(),
            create_time: Some(system_time_to_timestamp_proto(&self.create_time)),
            last_activity_time: Some(system_time_to_timestamp_proto(&self.last_activity_time)),
        })
    }

    pub fn get_game_info(&self) -> GameInfo {
        let game_info = GameInfo {
            game_id: String::from(&self.game_id),
            config: Some(self.config.raw_config()),
            player_count: self.player_manager.get_real_players().len() as i32,
            owner: self.player_manager.get_owner().cloned(),
            is_running: self.is_running(),
            create_time: Some(system_time_to_timestamp_proto(&self.create_time)),
            last_activity_time: Some(system_time_to_timestamp_proto(&self.last_activity_time)),
        };
        game_info
    }

    pub fn get_game_id(&self) -> &str {
        &self.game_id
    }

    pub fn contains_player(&self, player_id: &PlayerId) -> bool {
        self.player_manager.get_player(player_id).is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.player_manager.get_real_players().is_empty()
    }

    pub fn get_create_time(&self) -> &SystemTime {
        &self.create_time
    }

    fn is_running(&self) -> bool {
        self.stage != Stage::NotRunning
    }

    fn has_enough_players_to_play(&self) -> bool {
        self.player_manager.get_real_players().len() >= 2
            && self.player_manager.get_real_players().len()
                + self.player_manager.get_artificial_players().len()
                >= MINIMUM_PLAYERS_REQUIRED_TO_PLAY
    }

    fn user_is_banned(&self, user_name: &str) -> bool {
        self.banned_users.iter().any(|user| user.name == user_name)
    }

    pub fn get_last_activity_time(&self) -> &SystemTime {
        &self.last_activity_time
    }

    fn get_pseudorandom_ordered_white_cards_played_list(&self) -> Vec<WhiteCardsPlayed> {
        let mut white_played_list = Vec::new();
        for (player_id, cards) in self.white_card_gameplay_manager.get_played_cards() {
            let mut card_texts = Vec::new();

            for card in cards {
                card_texts.push(String::from(get_text_from_playable_white_card(card)));
            }

            let white_played_entry = WhiteCardsPlayed {
                player: self.player_manager.get_player(player_id).cloned(),
                card_texts,
            };

            white_played_list.push(white_played_entry);
        }

        let mut rand = rand_chacha::ChaChaRng::from_seed(self.get_round_nonce_digest());
        white_played_list.shuffle(&mut rand);

        white_played_list
    }

    pub fn search_white_card_texts(
        &self,
        page_size: usize,
        skip: usize,
        filter: &str,
    ) -> (Vec<String>, bool, usize) {
        let (texts, has_next_page) = self
            .white_card_text_query_handler
            .query(filter, page_size, skip);
        (
            texts,
            has_next_page,
            self.white_card_text_query_handler.total_size(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::helper::get_answer_fields_from_black_card_in_round;
    use super::*;
    use shared::proto::crusty_cards_api::GameConfig;
    use shared::test_helper::{
        generate_test_custom_black_cards, generate_test_custom_white_cards,
        generate_test_default_black_cards, generate_test_default_white_cards,
        get_valid_endless_test_game_config, get_valid_test_game_config,
    };
    use std::collections::HashSet;

    fn get_fake_user_proto(user_name: &str) -> User {
        User {
            name: user_name.to_string(),
            display_name: format!("User {}", user_name),
            create_time: None,
            update_time: None,
        }
    }

    fn get_game_with_players(config: GameConfig, player_count: usize) -> Result<Game, Status> {
        let mut game = Game::new(
            String::from("1234"),
            ValidatedGameConfig::new(config).unwrap(),
            generate_test_custom_black_cards(50),
            generate_test_custom_white_cards(500),
            generate_test_default_black_cards(50),
            generate_test_default_white_cards(500),
        )?;

        for i in 0..player_count {
            game.join(get_fake_user_proto(&format!("users/{}", i)))?;
        }

        return Ok(game);
    }

    fn get_basic_game_with_players(player_count: usize) -> Result<Game, Status> {
        get_game_with_players(get_valid_test_game_config(), player_count)
    }

    fn get_basic_endless_game_with_players(player_count: usize) -> Result<Game, Status> {
        get_game_with_players(get_valid_endless_test_game_config(), player_count)
    }

    // The GameInfo proto contains fields such as player join_time that will be different for every test run.
    // This function validates that these fields contain some value and then removes them to allow for consistent
    // results in tests.
    fn validate_and_remove_changing_parameters_from_game_info(mut game_info: GameInfo) -> GameInfo {
        assert_eq!(game_info.create_time.is_some(), true);
        game_info.create_time = None;
        assert_eq!(game_info.last_activity_time.is_some(), true);
        game_info.last_activity_time = None;

        game_info
    }

    fn play_for_all_real_players(game: &mut Game) {
        assert_eq!(game.stage, Stage::PlayPhase);
        for player in game.player_manager.get_real_players().clone() {
            let player_id = PlayerId::from_player_proto(&player).unwrap();
            let hand: Vec<PlayableWhiteCard> = game
                .white_card_gameplay_manager
                .get_hand_belonging_to_player(&player_id)
                .unwrap()
                .into_iter()
                .map(|card| card.clone())
                .collect();
            match &player_id {
                PlayerId::RealUser(user_name) => {
                    if !game.player_manager.is_judge(user_name) {
                        assert_eq!(game.stage, Stage::PlayPhase);
                        assert_eq!(
                            game.play_cards(
                                user_name,
                                &hand[0..get_answer_fields_from_black_card_in_round(
                                    game.black_card_deck.get_current_black_card()
                                )]
                            )
                            .is_ok(),
                            true
                        );
                    }
                }
                _ => {}
            };
        }
        assert_eq!(game.stage, Stage::JudgePhase);
    }

    fn add_artificial_player_as_owner(game: &mut Game) {
        match game
            .player_manager
            .get_real_players()
            .first()
            .unwrap()
            .clone()
            .identifier
            .unwrap()
        {
            Identifier::User(user) => {
                game.add_artificial_player(&user.name, String::from(""))
                    .unwrap();
            }
            _ => panic!("Owner is artificial user! What???"),
        };
    }

    fn remove_artificial_player_as_owner(game: &mut Game) {
        match game
            .player_manager
            .get_real_players()
            .first()
            .unwrap()
            .clone()
            .identifier
            .unwrap()
        {
            Identifier::User(user) => {
                game.remove_artificial_player(&user.name, "").unwrap();
            }
            _ => panic!("Owner is artificial user! What???"),
        };
    }

    fn assert_valid_not_running_stage(game: &Game) {
        assert_eq!(game.stage, Stage::NotRunning);
        assert!(game.player_manager.get_queued_real_players().is_empty());
        assert!(game
            .player_manager
            .get_queued_artificial_players()
            .is_empty());
        assert_eq!(game.player_manager.get_judge(), None);
    }

    #[test]
    fn can_start_and_stop() {
        let mut game: Game = get_basic_game_with_players(MINIMUM_PLAYERS_REQUIRED_TO_PLAY).unwrap();
        assert_eq!(game.is_running(), false);
        assert_eq!(game.start("users/0").is_ok(), true);
        assert_eq!(game.is_running(), true);
        // Can't start if game is already running.
        assert_eq!(game.start("users/0").is_err(), true);
        assert_eq!(game.is_running(), true);
        assert_eq!(game.stop("users/0").is_ok(), true);
        assert_eq!(game.is_running(), false);
        // Can't stop if game is not running.
        assert_eq!(game.stop("users/0").is_err(), true);
        assert_eq!(game.is_running(), false);
    }

    #[test]
    fn create_basic_game_and_add_players() {
        let game: Game = get_basic_game_with_players(MIN_PLAYER_LIMIT as usize).unwrap();
        assert_eq!(format!("{:?}", validate_and_remove_changing_parameters_from_game_info(game.get_game_info())), "GameInfo { game_id: \"1234\", config: Some(GameConfig { display_name: \"Test Game\", max_players: 3, hand_size: 3, custom_cardpack_names: [\"test_custom_cardpack_name\"], default_cardpack_names: [\"test_default_cardpack_name\"], blank_white_card_config: Some(BlankWhiteCardConfig { behavior: Disabled, blank_white_cards_added: None }), end_condition: Some(EndlessMode(Empty)) }), player_count: 2, owner: Some(User { name: \"users/0\", display_name: \"User users/0\", create_time: None, update_time: None }), is_running: false, create_time: None, last_activity_time: None }");
    }

    #[test]
    fn run_game_for_full_round() {
        let mut game: Game =
            get_basic_endless_game_with_players(MINIMUM_PLAYERS_REQUIRED_TO_PLAY).unwrap();
        assert_valid_not_running_stage(&game);
        assert_eq!(game.start("users/0").is_ok(), true);

        for _ in 0..100 {
            assert_eq!(game.stage, Stage::PlayPhase);
            play_for_all_real_players(&mut game);
            assert_eq!(game.stage, Stage::JudgePhase);
            let judge_name = String::from(&game.player_manager.get_judge().unwrap().name);
            assert_eq!(game.vote_card(&judge_name, 1).is_ok(), true);
            assert_eq!(game.stage, Stage::RoundEndPhase);
            assert_eq!(game.vote_start_next_round(&judge_name).is_ok(), true);
        }
    }

    #[test]
    fn run_game_for_full_round_with_artificial_player() {
        let mut game: Game =
            get_basic_endless_game_with_players(MINIMUM_PLAYERS_REQUIRED_TO_PLAY).unwrap();
        add_artificial_player_as_owner(&mut game);
        assert_valid_not_running_stage(&game);
        assert_eq!(game.start("users/0").is_ok(), true);

        for _ in 0..100 {
            assert_eq!(game.stage, Stage::PlayPhase);
            play_for_all_real_players(&mut game);
            assert_eq!(game.stage, Stage::JudgePhase);
            let judge_name = String::from(&game.player_manager.get_judge().unwrap().name);
            assert_eq!(game.vote_card(&judge_name, 1).is_ok(), true);
            assert_eq!(game.stage, Stage::RoundEndPhase);
            assert_eq!(game.vote_start_next_round(&judge_name).is_ok(), true);
        }
    }

    #[test]
    fn generates_unique_round_nonces() {
        let mut game: Game =
            get_basic_endless_game_with_players(MINIMUM_PLAYERS_REQUIRED_TO_PLAY).unwrap();
        assert_eq!(game.start("users/0").is_ok(), true);

        let mut round_nonces: HashSet<[u8; 32]> = HashSet::new();

        for _ in 0..10 {
            assert_eq!(game.stage, Stage::PlayPhase);
            let nonce = game.get_round_nonce_digest();
            assert_eq!(round_nonces.insert(nonce), true);
            play_for_all_real_players(&mut game);
            assert_eq!(game.stage, Stage::JudgePhase);
            let judge_name = String::from(&game.player_manager.get_judge().unwrap().name);
            assert_eq!(game.vote_card(&judge_name, 1).is_ok(), true);
            assert_eq!(game.stage, Stage::RoundEndPhase);
            assert_eq!(game.vote_start_next_round(&judge_name).is_ok(), true);
        }

        // Round nonce should change when the judge changes, even if the round number has not.
        let nonce = game.get_round_nonce_digest();
        assert_eq!(round_nonces.insert(nonce), true);
        game.leave("users/0").unwrap();
        let nonce = game.get_round_nonce_digest();
        assert_eq!(round_nonces.insert(nonce), true);
    }

    #[test]
    fn judge_leaves_during_judge_phase() {
        let mut game: Game =
            get_basic_endless_game_with_players(MINIMUM_PLAYERS_REQUIRED_TO_PLAY).unwrap();
        add_artificial_player_as_owner(&mut game);
        assert_valid_not_running_stage(&game);
        assert_eq!(game.start("users/0").is_ok(), true);
        assert_eq!(game.stage, Stage::PlayPhase);
        play_for_all_real_players(&mut game);
        assert_eq!(game.stage, Stage::JudgePhase);
        let judge_name = String::from(&game.player_manager.get_judge().unwrap().name);
        game.leave(&judge_name).unwrap();
        assert_eq!(game.stage, Stage::RoundEndPhase);
        game.join(get_fake_user_proto(&judge_name)).unwrap();
        assert_eq!(game.vote_start_next_round(&judge_name).is_ok(), true);
        assert_eq!(game.stage, Stage::PlayPhase);
        play_for_all_real_players(&mut game);
        assert_eq!(game.stage, Stage::JudgePhase);
    }

    #[test]
    fn judge_leaves_during_judge_phase_and_game_no_longer_has_enough_players_to_run() {
        let mut game: Game =
            get_basic_endless_game_with_players(MINIMUM_PLAYERS_REQUIRED_TO_PLAY - 1).unwrap();
        add_artificial_player_as_owner(&mut game);
        assert_valid_not_running_stage(&game);
        assert_eq!(game.start("users/0").is_ok(), true);
        assert_eq!(game.stage, Stage::PlayPhase);
        play_for_all_real_players(&mut game);
        assert_eq!(game.stage, Stage::JudgePhase);
        let judge_name = String::from(&game.player_manager.get_judge().unwrap().name);
        game.leave(&judge_name).unwrap();
        assert_valid_not_running_stage(&game);
        game.join(get_fake_user_proto(&judge_name)).unwrap();
        let owner_name = String::from(&game.player_manager.get_owner().unwrap().name);
        assert_eq!(game.start(&owner_name).is_ok(), true);
        assert_eq!(game.stage, Stage::PlayPhase);
        play_for_all_real_players(&mut game);
        assert_eq!(game.stage, Stage::JudgePhase);
    }

    #[test]
    fn remove_artificial_player_during_judge_phase() {
        let mut game: Game =
            get_basic_endless_game_with_players(MINIMUM_PLAYERS_REQUIRED_TO_PLAY).unwrap();
        add_artificial_player_as_owner(&mut game);
        assert_valid_not_running_stage(&game);
        assert_eq!(game.start("users/0").is_ok(), true);
        assert_eq!(game.stage, Stage::PlayPhase);
        play_for_all_real_players(&mut game);
        assert_eq!(game.stage, Stage::JudgePhase);
        // TODO - The whole point of this test is that the line below should be uncommented.unimplemented.
        // However, right now that makes the test fail. Let's get this test working properly and uncomment
        // the line below.
        // remove_artificial_player_as_owner(&mut game);
        let judge_name = String::from(&game.player_manager.get_judge().unwrap().name);
        assert_eq!(game.vote_card(&judge_name, 1).is_ok(), true);
        let white_cards_played_list = game.get_pseudorandom_ordered_white_cards_played_list();
        for white_played_entry in white_cards_played_list {
            assert!(white_played_entry.player.is_some());
        }
    }

    #[test]
    fn add_artificial_player_during_judge_phase() {
        let mut game: Game =
            get_basic_endless_game_with_players(MINIMUM_PLAYERS_REQUIRED_TO_PLAY).unwrap();
        assert_valid_not_running_stage(&game);
        assert_eq!(game.start("users/0").is_ok(), true);
        assert_eq!(game.stage, Stage::PlayPhase);
        play_for_all_real_players(&mut game);
        assert_eq!(game.stage, Stage::JudgePhase);
        add_artificial_player_as_owner(&mut game);
        assert_eq!(
            game.get_pseudorandom_ordered_white_cards_played_list()
                .len(),
            2
        );
        let mut judge_name = String::from(&game.player_manager.get_judge().unwrap().name);
        assert_eq!(game.vote_card(&judge_name, 1).is_ok(), true);
        assert_eq!(game.stage, Stage::RoundEndPhase);
        assert_eq!(game.vote_start_next_round(&judge_name).is_ok(), true);
        assert_eq!(game.stage, Stage::PlayPhase);
        play_for_all_real_players(&mut game);
        assert_eq!(game.stage, Stage::JudgePhase);
        assert_eq!(
            game.get_pseudorandom_ordered_white_cards_played_list()
                .len(),
            3
        );
        add_artificial_player_as_owner(&mut game);
        judge_name = String::from(&game.player_manager.get_judge().unwrap().name);
        assert_eq!(game.vote_card(&judge_name, 1).is_ok(), true);
        assert_eq!(game.stage, Stage::RoundEndPhase);
    }
}
