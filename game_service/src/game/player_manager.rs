use super::player_id::PlayerId;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rand::RngCore;
use shared::proto::crusty_cards_api::{player::Identifier, Player, User};
use shared::time::{system_time_to_timestamp_proto, timestamp_proto_to_system_time};
use std::time::SystemTime;

const ARTIFICIAL_PLAYER_DEFAULT_NAMES: [&str; 30] = [
    // Greek Gods
    "Dionysus",
    "Asclepius",
    "Hephæstus",
    // My Little Pony Characters
    "Rainbow Dash",
    "Twilight Sparkle",
    "Fluttershy",
    // German Names
    "Hans",
    "Günter",
    "Klaus",
    // Transformers
    "Megatron",
    "Ultra Magnus",
    "Wheeljack",
    // Spies
    "James Bond",
    "Ethan Hunt",
    "Jason Bourne",
    // Star Wars Characters
    "Salacious B. Crumb",
    "Logray",
    "HK-47",
    // Ratchet and Clank
    "Captain Quark",
    "Chairman Drek",
    "Mr. Zurkon",
    // Monsters Inc. Characters
    "Mike Wazowski",
    "Henry J. Waternoose III",
    "George Sanderson",
    // Weird Monarch Nicknames
    "Æthelred the Unready",
    "Edward Longshanks",
    "Henry The Accountant",
    // Spongebob Characters
    "Monty P. Moneybags",
    "The Hash Slinging Slasher",
    "Perch Perkins",
];

pub struct PlayerManager {
    real_players: Vec<Player>,
    artificial_players: Vec<Player>,
    queued_real_players: Vec<Player>,
    queued_artificial_players: Vec<Player>,
    judge_player_index: Option<usize>, // Points to an index in the `real_players` property (or is None if the game isn't running).
}

impl PlayerManager {
    pub fn new() -> PlayerManager {
        PlayerManager {
            real_players: Vec::new(),
            artificial_players: Vec::new(),
            queued_real_players: Vec::new(),
            queued_artificial_players: Vec::new(),
            judge_player_index: None,
        }
    }

    // Increments a player's score and returns their updated post-increment
    // score, or returns None if passed a PlayerId that's invalid or that
    // belongs to a user that is not in the game.
    pub fn increment_player_score(&mut self, player_id: &PlayerId) -> Option<i32> {
        let player = self.get_mut_player(player_id)?;
        let incremented_score = player.score + 1;
        player.score = incremented_score;
        Some(incremented_score)
    }

    pub fn get_player_score(&self, player_id: &PlayerId) -> Option<i32> {
        Some(self.get_player(player_id)?.score)
    }

    fn get_mut_player(&mut self, player_id: &PlayerId) -> Option<&mut Player> {
        self.real_players
            .iter_mut()
            .chain(self.artificial_players.iter_mut())
            .find(|player| match PlayerId::from_player_proto(player) {
                Some(proto_player_id) => &proto_player_id == player_id,
                None => false,
            })
    }

    pub fn get_player(&self, player_id: &PlayerId) -> Option<&Player> {
        self.real_players
            .iter()
            .chain(&self.artificial_players)
            .find(|player| match PlayerId::from_player_proto(player) {
                Some(proto_player_id) => &proto_player_id == player_id,
                None => false,
            })
    }

    pub fn get_real_player(&self, user_name: &str) -> Option<&Player> {
        self.real_players
            .iter()
            .find(|player| match &player.identifier {
                Some(Identifier::User(user)) => user.name == user_name,
                _ => false,
            })
    }

    pub fn get_owner(&self) -> Option<&User> {
        // The owner of the game is always the first person to join.
        // If that person leaves, the next person that joined is the
        // new owner.
        let first_player = match self.real_players.first() {
            Some(player) => player,
            None => return None,
        };

        match &first_player.identifier {
            Some(Identifier::User(user)) => Some(user),
            _ => None,
        }
    }

    pub fn is_owner(&self, user_name: &str) -> bool {
        match self.get_owner() {
            Some(owner) => owner.name == user_name,
            None => false,
        }
    }

    pub fn set_random_judge(&mut self) {
        self.judge_player_index = Some(thread_rng().next_u32() as usize % self.real_players.len());
    }

    pub fn increment_judge(&mut self) {
        self.judge_player_index = match self.judge_player_index {
            Some(index) => {
                if index + 1 < self.real_players.len() {
                    Some(index + 1)
                } else {
                    Some(0)
                }
            }
            None => None,
        };
    }

    pub fn get_judge(&self) -> Option<&User> {
        let judge_player_index = match self.judge_player_index {
            Some(index) => index,
            None => return None,
        };

        let player = match self.real_players.get(judge_player_index) {
            Some(player) => player,
            None => return None,
        };

        match &player.identifier {
            Some(Identifier::User(user)) => Some(user),
            _ => None,
        }
    }

    pub fn is_judge(&self, user_name: &str) -> bool {
        match self.get_judge() {
            Some(judge) => judge.name == user_name,
            None => false,
        }
    }

    fn sort_player_list_by_join_time(players: &mut Vec<Player>) {
        players.sort_by(|a, b| {
            let a_system = match &a.join_time {
                Some(join_time) => timestamp_proto_to_system_time(join_time),
                None => std::time::UNIX_EPOCH,
            };
            let b_system = match &b.join_time {
                Some(join_time) => timestamp_proto_to_system_time(join_time),
                None => std::time::UNIX_EPOCH,
            };
            if a_system > b_system {
                return std::cmp::Ordering::Greater;
            }
            if a_system < b_system {
                return std::cmp::Ordering::Less;
            }
            std::cmp::Ordering::Equal
        });
    }

    pub fn clone_all_players_sorted_by_join_time(&self) -> Vec<Player> {
        let mut all_players: Vec<Player> = self
            .real_players
            .iter()
            .chain(self.artificial_players.iter())
            .cloned()
            .collect();

        Self::sort_player_list_by_join_time(&mut all_players);

        all_players
    }

    pub fn clone_all_queued_players_sorted_by_join_time(&self) -> Vec<Player> {
        let mut all_queued_players: Vec<Player> = self
            .queued_real_players
            .iter()
            .chain(self.queued_artificial_players.iter())
            .cloned()
            .collect();

        Self::sort_player_list_by_join_time(&mut all_queued_players);

        all_queued_players
    }

    pub fn reset_player_scores(&mut self) {
        for player in self
            .real_players
            .iter_mut()
            .chain(self.artificial_players.iter_mut())
        {
            player.score = 0;
        }
    }

    pub fn drain_queued_real_and_artificial_players(&mut self) {
        self.real_players.append(&mut self.queued_real_players);
        self.artificial_players
            .append(&mut self.queued_artificial_players);
    }

    fn artificial_player_name_is_in_use(&self, artificial_player_name: &str) -> bool {
        for player in &self.artificial_players {
            if let Some(Identifier::ArtificialUser(artificial_user)) = &player.identifier {
                if artificial_user.display_name == artificial_player_name {
                    return true;
                }
            }
        }
        false
    }

    pub fn get_unused_default_artificial_player_name(&self) -> Option<String> {
        ARTIFICIAL_PLAYER_DEFAULT_NAMES
            .iter()
            .filter(|name| !self.artificial_player_name_is_in_use(name))
            .collect::<Vec<&&str>>()
            .choose(&mut thread_rng())
            .map(|name| String::from(**name))
    }

    pub fn user_is_in_game(&self, user_name: &str) -> bool {
        self.real_players
            .iter()
            .chain(&self.queued_real_players)
            .any(|player| match &player.identifier {
                Some(Identifier::User(user)) => user.name == user_name,
                _ => false,
            })
    }

    pub fn artificial_player_is_in_game(&self, artificial_player_id: &str) -> bool {
        self.artificial_players
            .iter()
            .chain(&self.queued_artificial_players)
            .any(|player| match &player.identifier {
                Some(Identifier::ArtificialUser(artificial_user)) => {
                    artificial_user.id == artificial_player_id
                }
                _ => false,
            })
    }

    pub fn get_user_names_for_all_real_players(&self) -> Vec<&str> {
        let mut user_names: Vec<&str> = Vec::new();
        for player in &self.real_players {
            if let Some(Identifier::User(user)) = &player.identifier {
                if !user.name.is_empty() {
                    user_names.push(&user.name);
                }
            }
        }
        user_names
    }

    fn remove_real_player_from_vec_by_name(players: &mut Vec<Player>, user_name: &str) {
        players.retain(|player| match &player.identifier {
            Some(Identifier::User(user)) => user.name != user_name,
            _ => true,
        });
    }

    fn remove_artificial_player_from_vec_by_name(
        players: &mut Vec<Player>,
        artificial_player_id: &str,
    ) {
        players.retain(|player| match &player.identifier {
            Some(Identifier::ArtificialUser(artificial_user)) => {
                artificial_user.id != artificial_player_id
            }
            _ => true,
        });
    }

    pub fn add_player(&mut self, player_identifier: Identifier) {
        match player_identifier {
            Identifier::User(_) => {
                let player = Player {
                    score: 0,
                    join_time: Some(system_time_to_timestamp_proto(&SystemTime::now())),
                    identifier: Some(player_identifier),
                };
                self.real_players.push(player);
            }
            Identifier::ArtificialUser(_) => {
                let player = Player {
                    score: 0,
                    join_time: Some(system_time_to_timestamp_proto(&SystemTime::now())),
                    identifier: Some(player_identifier),
                };
                self.artificial_players.push(player);
            }
        };
    }

    pub fn add_queued_player(&mut self, player_identifier: Identifier) {
        match player_identifier {
            Identifier::User(_) => {
                let player = Player {
                    score: 0,
                    join_time: Some(system_time_to_timestamp_proto(&SystemTime::now())),
                    identifier: Some(player_identifier),
                };
                self.queued_real_players.push(player);
            }
            Identifier::ArtificialUser(_) => {
                let player = Player {
                    score: 0,
                    join_time: Some(system_time_to_timestamp_proto(&SystemTime::now())),
                    identifier: Some(player_identifier),
                };
                self.queued_artificial_players.push(player);
            }
        };
    }

    pub fn remove_player(&mut self, player_id: &PlayerId) {
        match player_id {
            PlayerId::RealUser(user_name) => {
                Self::remove_real_player_from_vec_by_name(&mut self.real_players, user_name);
                Self::remove_real_player_from_vec_by_name(&mut self.queued_real_players, user_name);
                if let Some(judge_player_index) = self.judge_player_index {
                    if self.real_players.is_empty() {
                        self.judge_player_index = None;
                    } else if self.real_players.len() == judge_player_index {
                        self.judge_player_index = Some(0);
                    }
                }
            }
            PlayerId::ArtificialPlayer(artificial_player_id) => {
                Self::remove_artificial_player_from_vec_by_name(
                    &mut self.artificial_players,
                    artificial_player_id,
                );
                Self::remove_artificial_player_from_vec_by_name(
                    &mut self.queued_artificial_players,
                    artificial_player_id,
                );
            }
        };
    }

    pub fn get_last_artificial_player(&mut self) -> Option<PlayerId> {
        if self.queued_artificial_players.is_empty() {
            return PlayerId::from_player_proto(self.artificial_players.last()?);
        } else {
            return PlayerId::from_player_proto(self.queued_artificial_players.last()?);
        }
    }

    pub fn get_real_players(&self) -> &Vec<Player> {
        &self.real_players
    }

    pub fn get_artificial_players(&self) -> &Vec<Player> {
        &self.artificial_players
    }

    pub fn get_queued_real_players(&self) -> &Vec<Player> {
        &self.queued_real_players
    }

    pub fn get_queued_artificial_players(&self) -> &Vec<Player> {
        &self.queued_artificial_players
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::proto::crusty_cards_api::ArtificialUser;

    #[test]
    fn add_too_many_artificial_players() {
        let mut player_manager = PlayerManager::new();
        for _ in 0..ARTIFICIAL_PLAYER_DEFAULT_NAMES.len() {
            let name = player_manager
                .get_unused_default_artificial_player_name()
                .unwrap();
            player_manager.add_player(Identifier::ArtificialUser(ArtificialUser {
                id: name.clone(),
                display_name: name,
            }));
        }
        assert_eq!(
            player_manager
                .get_unused_default_artificial_player_name()
                .is_none(),
            true
        );
    }

    #[test]
    fn judge_is_reassigned_when_current_judge_leaves() {
        let mut player_manager = PlayerManager::new();

        player_manager.add_player(Identifier::User(User {
            name: "users/1".to_string(),
            display_name: "User 1".to_string(),
            create_time: None,
            update_time: None,
        }));
        player_manager.add_player(Identifier::User(User {
            name: "users/2".to_string(),
            display_name: "User 2".to_string(),
            create_time: None,
            update_time: None,
        }));
        player_manager.add_player(Identifier::User(User {
            name: "users/3".to_string(),
            display_name: "User 3".to_string(),
            create_time: None,
            update_time: None,
        }));
        player_manager.add_player(Identifier::User(User {
            name: "users/4".to_string(),
            display_name: "User 4".to_string(),
            create_time: None,
            update_time: None,
        }));

        // Set judge to user 1.
        player_manager.judge_player_index = Some(0);
        // Sanity check.
        assert_eq!(player_manager.get_judge().unwrap().name, "users/1");
        // Remove user 1.
        player_manager.remove_player(&PlayerId::RealUser("users/1".to_string()));
        // Judge should now be user 2.
        assert_eq!(player_manager.get_judge().unwrap().name, "users/2");

        // Set judge to last player (reminder, judge index is 0-based).
        player_manager.judge_player_index = Some(2);
        // Sanity check.
        assert_eq!(player_manager.get_judge().unwrap().name, "users/4");
        // Remove last player.
        player_manager.remove_player(&PlayerId::RealUser("users/4".to_string()));
        // Judge should wrap around to first player (user 2 since we already removed user 1).
        assert_eq!(player_manager.get_judge().unwrap().name, "users/2");

        // Remove all other players
        player_manager.remove_player(&PlayerId::RealUser("users/2".to_string()));
        player_manager.remove_player(&PlayerId::RealUser("users/3".to_string()));
        assert!(player_manager.get_judge().is_none());
    }

    #[test]
    fn judge_is_reassigned_when_all_players_leave() {
        let mut player_manager = PlayerManager::new();

        player_manager.add_player(Identifier::User(User {
            name: "users/1".to_string(),
            display_name: "User 1".to_string(),
            create_time: None,
            update_time: None,
        }));

        // Set judge to user 1.
        player_manager.judge_player_index = Some(0);
        // Sanity check.
        assert_eq!(player_manager.get_judge().unwrap().name, "users/1");
        // Remove user 1.
        player_manager.remove_player(&PlayerId::RealUser("users/1".to_string()));
        // Judge should now be None since there are no users in the game.
        assert!(player_manager.get_judge().is_none());

        // A new user who joins should not be the judge since everyone left previously.
        player_manager.add_player(Identifier::User(User {
            name: "users/2".to_string(),
            display_name: "User 2".to_string(),
            create_time: None,
            update_time: None,
        }));
        assert!(player_manager.get_judge().is_none());
    }
}
