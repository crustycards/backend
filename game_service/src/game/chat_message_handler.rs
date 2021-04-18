use shared::proto::ChatMessage;

pub struct ChatMessageHandler {
    messages: Vec<ChatMessage>,
    last_message_index: Option<usize>, // This is only `None` when no messages have been added yet.
    max_len: usize,
}

impl ChatMessageHandler {
    pub fn new(max_len: usize) -> ChatMessageHandler {
        ChatMessageHandler {
            messages: Vec::new(),
            last_message_index: None,
            max_len,
        }
    }

    pub fn add_new_message(&mut self, message: ChatMessage) {
        if self.max_len == 0 {
            return;
        }

        match &mut self.last_message_index {
            Some(last_message_index) => {
                *last_message_index += 1;
                if *last_message_index == self.max_len {
                    *last_message_index = 0;
                }
            }
            None => {
                self.last_message_index = Some(0);
            }
        };

        match self.last_message_index {
            Some(last_message_index) => {
                if self.messages.len() < self.max_len {
                    self.messages.push(message);
                } else {
                    let _old_message =
                        std::mem::replace(&mut self.messages[last_message_index], message);
                }
            }
            None => {} // Should never happen because of the first match statement in this function.
        };
    }

    pub fn clone_message_list(&self) -> Vec<ChatMessage> {
        match self.last_message_index {
            Some(last_message_index) => {
                let mut cloned_messages = self.messages.clone();
                cloned_messages.rotate_left(last_message_index + 1);
                cloned_messages
            }
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_zero_max_len() {
        let mut message_handler = ChatMessageHandler::new(0);
        assert_eq!(message_handler.clone_message_list().is_empty(), true);

        let message = ChatMessage {
            user: None,
            text: String::from("message"),
            create_time: None,
        };
        message_handler.add_new_message(message);

        assert_eq!(message_handler.clone_message_list().is_empty(), true);
    }

    #[test]
    fn handles_empty_message_list() {
        let message_handler = ChatMessageHandler::new(10);
        assert_eq!(message_handler.clone_message_list().is_empty(), true);
    }

    #[test]
    fn can_always_read_messages() {
        let mut message_handler = ChatMessageHandler::new(10);
        for i in 0..10 {
            let message = ChatMessage {
                user: None,
                text: format!("message_{}", i),
                create_time: None,
            };
            message_handler.add_new_message(message);
            let messages = message_handler.clone_message_list();
            assert_eq!(messages.len(), i + 1);
        }
        for i in 0..100 {
            let message = ChatMessage {
                user: None,
                text: format!("message_{}", i),
                create_time: None,
            };
            message_handler.add_new_message(message);
            let messages = message_handler.clone_message_list();
            assert_eq!(messages.len(), 10);
        }
    }

    #[test]
    fn wraps_message_overflow() {
        let mut message_handler = ChatMessageHandler::new(10);
        for i in 0..10 {
            let message = ChatMessage {
                user: None,
                text: format!("message_{}", i),
                create_time: None,
            };
            message_handler.add_new_message(message);
        }

        let messages = message_handler.clone_message_list();
        assert_eq!(messages.len(), 10);
        assert_eq!(messages.first().unwrap().text, "message_0");
        assert_eq!(messages.last().unwrap().text, "message_9");

        for i in 10..100 {
            let message = ChatMessage {
                user: None,
                text: format!("message_{}", i),
                create_time: None,
            };
            message_handler.add_new_message(message);

            let messages = message_handler.clone_message_list();
            assert_eq!(messages.len(), 10);
            assert_eq!(messages.first().unwrap().text, format!("message_{}", i - 9));
            assert_eq!(messages.last().unwrap().text, format!("message_{}", i));
        }
    }
}
