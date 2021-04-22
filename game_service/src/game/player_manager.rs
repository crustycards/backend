use super::player_id::PlayerId;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rand::RngCore;
use shared::proto::{player::Identifier, Player, User};
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
    "Jason Borne",
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
        let player = self.get_mut_player(&player_id)?;
        let incremented_score = player.score + 1;
        player.score = incremented_score;
        Some(incremented_score)
    }

    pub fn get_player_score(&self, player_id: &PlayerId) -> Option<i32> {
        Some(self.get_player(&player_id)?.score)
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
                Some(identifier) => match identifier {
                    Identifier::User(user) => user.name == user_name,
                    _ => false,
                },
                None => false,
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
            Some(identifier) => match identifier {
                Identifier::User(user) => Some(user),
                _ => None,
            },
            None => None,
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
            Some(identifier) => match identifier {
                Identifier::User(user) => Some(user),
                _ => None,
            },
            None => None,
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
            .map(|player| player.clone())
            .collect();

        Self::sort_player_list_by_join_time(&mut all_players);

        all_players
    }

    pub fn clone_all_queued_players_sorted_by_join_time(&self) -> Vec<Player> {
        let mut all_queued_players: Vec<Player> = self
            .queued_real_players
            .iter()
            .chain(self.queued_artificial_players.iter())
            .map(|player| player.clone())
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
            match &player.identifier {
                Some(identifier) => {
                    match identifier {
                        Identifier::ArtificialUser(artificial_user) => {
                            if artificial_user.display_name == artificial_player_name {
                                return true;
                            }
                        }
                        _ => {}
                    };
                }
                None => {}
            };
        }

        return false;
    }

    pub fn get_unused_default_artificial_player_name(&self) -> String {
        loop {
            let display_name = *ARTIFICIAL_PLAYER_DEFAULT_NAMES
                .choose(&mut thread_rng())
                .unwrap();
            if !self.artificial_player_name_is_in_use(display_name) {
                return String::from(display_name);
            }
        }
    }

    pub fn user_is_in_game(&self, user_name: &str) -> bool {
        self.real_players
            .iter()
            .chain(&self.queued_real_players)
            .find(|player| match &player.identifier {
                Some(identifier) => match identifier {
                    Identifier::User(user) => user.name == user_name,
                    _ => false,
                },
                None => false,
            })
            .is_some()
    }

    pub fn artificial_player_is_in_game(&self, artificial_player_id: &str) -> bool {
        self.artificial_players
            .iter()
            .chain(&self.queued_artificial_players)
            .find(|player| match &player.identifier {
                Some(identifier) => match identifier {
                    Identifier::ArtificialUser(artificial_user) => {
                        artificial_user.id == artificial_player_id
                    }
                    _ => false,
                },
                None => false,
            })
            .is_some()
    }

    pub fn get_user_names_for_all_real_players(&self) -> Vec<&str> {
        let mut user_names: Vec<&str> = Vec::new();
        for player in &self.real_players {
            match &player.identifier {
                Some(identifier) => {
                    if let Identifier::User(user) = identifier {
                        if !user.name.is_empty() {
                            user_names.push(&user.name);
                        }
                    }
                }
                None => {}
            };
        }
        user_names
    }

    fn remove_real_player_from_vec_by_name(players: &mut Vec<Player>, user_name: &str) {
        players.retain(|player| match &player.identifier {
            Some(identifier) => match identifier {
                Identifier::User(user) => user.name != user_name,
                _ => true,
            },
            None => true,
        });
    }

    fn remove_artificial_player_from_vec_by_name(
        players: &mut Vec<Player>,
        artificial_player_id: &str,
    ) {
        players.retain(|player| match &player.identifier {
            Some(Identifier::ArtificialUser(artificial_user)) => artificial_user.id != artificial_player_id,
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
