use lapin::{
    options::*, types::FieldTable, BasicProperties, Channel, Connection, ConnectionProperties,
};

static GAME_QUEUE_NAME: &str = "GAME";

pub struct MessageQueue {
    channel: Channel,
}

impl MessageQueue {
    pub fn new(amqp_uri: &str) -> MessageQueue {
        let connection = Connection::connect(
            amqp_uri,
            ConnectionProperties::default().with_default_executor(4),
        )
        .wait()
        .unwrap();
        let channel = connection.create_channel().wait().unwrap();
        channel
            .queue_declare(
                GAME_QUEUE_NAME,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .wait()
            .unwrap();
        MessageQueue { channel }
    }

    fn construct_game_update_message(user_names: Vec<&str>) -> String {
        let user_names_with_surrounding_quotes: Vec<String> = user_names
            .into_iter()
            .map(|name| format!("\"{}\"", name))
            .collect();
        return format!(
            "{{\"type\": \"GAME_UPDATED\", \"payload\": [{}]}}",
            user_names_with_surrounding_quotes.join(", ")
        );
    }

    pub fn game_updated_for_users(
        &self,
        user_names: Vec<&str>,
    ) -> Result<lapin::publisher_confirm::Confirmation, lapin::Error> {
        match self
            .channel
            .basic_publish(
                "",
                GAME_QUEUE_NAME,
                BasicPublishOptions::default(),
                MessageQueue::construct_game_update_message(user_names).into_bytes(),
                BasicProperties::default(),
            )
            .wait()
        {
            Ok(mut res) => match res.wait() {
                Ok(confirmation) => Ok(confirmation),
                Err(err) => Err(err),
            },
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_game_update_message() {
        assert_eq!(
            MessageQueue::construct_game_update_message(vec!("users/1234")),
            "{\"type\": \"GAME_UPDATED\", \"payload\": [\"users/1234\"]}"
        );
        assert_eq!(
            MessageQueue::construct_game_update_message(vec!("users/1234", "users/5678")),
            "{\"type\": \"GAME_UPDATED\", \"payload\": [\"users/1234\", \"users/5678\"]}"
        );
    }
}
