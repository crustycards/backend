use super::game::Game;
use super::player_id::PlayerId;
use std::time::Duration;

pub struct GameIndexer {
    // Games in this list are guaranteed to be sorted by their
    // internal `create_time`, even if they aren't inserted in that order.
    games_by_insert_time: Vec<Game>,
}

// TODO - Optimize each method in this struct, potentially using multiple data structures.
impl GameIndexer {
    pub fn new() -> GameIndexer {
        GameIndexer {
            games_by_insert_time: Vec::new(),
        }
    }

    pub fn get_games_by_insert_time(&self) -> &[Game] {
        &self.games_by_insert_time[..]
    }

    pub fn get_game_by_game_id(&mut self, game_id: &str) -> Option<&mut Game> {
        self.games_by_insert_time
            .iter_mut()
            .find(|game| game.get_game_id() == game_id)
    }

    pub fn get_game_by_player_id(&mut self, player_id: &PlayerId) -> Option<&mut Game> {
        self.games_by_insert_time
            .iter_mut()
            .find(|game| game.contains_player(player_id))
    }

    pub fn insert_game(&mut self, game: Game) {
        // Since real-world usage will result in games being inserted in
        // near-exact order, we're iterating starting at the end of the list
        // to prevent iterating over the entire list whenever a game is inserted.
        for (index, g) in self.games_by_insert_time.iter().enumerate().rev() {
            if g.get_create_time()
                .duration_since(game.get_create_time().clone())
                .is_err()
            {
                self.games_by_insert_time.insert(index + 1, game);
                return;
            }
        }
        self.games_by_insert_time.insert(0, game);
    }

    // TODO - Since `self.games_by_insert_time` is guaranteed to be ordered,
    // we can do a binary search here rather than iterating through all games.
    pub fn remove_game(&mut self, game_id: &str) {
        for (index, game) in self.games_by_insert_time.iter().enumerate() {
            if game.get_game_id() == game_id {
                self.games_by_insert_time.remove(index);
                break;
            }
        }
    }

    pub fn remove_unused_games(&mut self, duration: Duration) {
        self.games_by_insert_time
            .retain(|game| match game.get_last_activity_time().elapsed() {
                Ok(last_activity_duration) => last_activity_duration < duration,
                _ => true,
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::seq::SliceRandom;
    use shared::proto_validation::ValidatedGameConfig;
    use shared::test_helper::{
        generate_test_custom_black_cards, generate_test_custom_white_cards,
        generate_test_default_black_cards, generate_test_default_white_cards,
        get_valid_test_game_config,
    };
    use std::thread;
    use std::time::Duration;

    #[test]
    fn reorders_games_when_added_out_of_timestamp_order() {
        let mut games: Vec<Game> = Vec::new();
        for i in 0..100 {
            games.push(
                Game::new(
                    format!("Game {}", i),
                    ValidatedGameConfig::new(get_valid_test_game_config()).unwrap(),
                    generate_test_custom_black_cards(1),
                    generate_test_custom_white_cards(100),
                    generate_test_default_black_cards(1),
                    generate_test_default_white_cards(100),
                )
                .unwrap(),
            );
            thread::sleep(Duration::from_millis(1));
        }
        games.shuffle(&mut rand::thread_rng());

        let mut indexer = GameIndexer::new();
        for game in games {
            indexer.insert_game(game);
        }

        let games = indexer.get_games_by_insert_time();
        for i in 0..100 {
            assert_eq!(games.get(i).unwrap().get_game_id(), format!("Game {}", i));
        }
    }
}
