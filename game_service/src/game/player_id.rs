use shared::proto::{player::Identifier, Player};
use std::hash::Hash;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PlayerId {
    RealUser(String),
    ArtificialPlayer(String),
}

impl PlayerId {
    pub fn from_player_proto(player: &Player) -> Option<PlayerId> {
        match &player.identifier {
            Some(identifier) => match identifier {
                Identifier::User(user) => Some(PlayerId::RealUser(String::from(&user.name))),
                Identifier::ArtificialUser(artificial_user) => Some(PlayerId::ArtificialPlayer(
                    String::from(&artificial_user.id),
                )),
            },
            None => None,
        }
    }
}
