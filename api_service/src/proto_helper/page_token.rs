use super::super::service::helper::*;
use bson::oid::ObjectId;
use prost::Message;
use prost_types::Any;
use sha2::{Digest, Sha256};
use tonic::Status;

fn get_string_hash(data: &str) -> Vec<u8> {
    let hash: [u8; 32] = Sha256::digest(data.as_bytes()).into();
    hash.to_vec()
}

fn serialize_proto_message_to_hex_string(message: &impl Message) -> String {
    let mut buf: Vec<u8> = Vec::new();
    // Unwrap is safe here. The only reason that this encoding can fail
    // according to prost documentation is if the buffer reference given
    // doesn't have sufficient capacity. But since we're using a vector,
    // this can never happen since it can dynamically resize itself.
    message.encode(&mut buf).unwrap();
    hex::encode(buf)
}

fn get_message_hash(message: &impl Message) -> Vec<u8> {
    get_string_hash(&serialize_proto_message_to_hex_string(message))
}

fn parse_proto_any_from_hex_string(proto_hex_string: &str) -> Option<Any> {
    let mut any = Any {
        type_url: String::from(""),
        value: Vec::new(),
    };
    match hex::decode(proto_hex_string) {
        Ok(byte_vec) => {
            let bytes: prost::bytes::Bytes = byte_vec.into();
            match any.merge(bytes) {
                Ok(_) => {}
                _ => return None,
            };
        }
        _ => return None,
    };
    Some(any)
}

pub fn create_page_token(request_message: &impl Message, item_marker: String) -> String {
    let any = Any {
        type_url: item_marker,
        value: get_message_hash(request_message),
    };
    serialize_proto_message_to_hex_string(&any)
}

pub fn parse_page_token_string(
    request_message: &impl Message,
    request_page_token: &str,
) -> Result<String, Status> {
    let encoded_any = match parse_proto_any_from_hex_string(request_page_token) {
        Some(encoded_any) => encoded_any,
        _ => return Err(invalid_page_token_error()),
    };

    let current_request_message_hash = get_message_hash(request_message);

    let previous_request_message_hash = encoded_any.value;

    if current_request_message_hash != previous_request_message_hash {
        return Err(Status::invalid_argument(
            "All request fields must match the previous request.",
        ));
    }

    let item_marker = encoded_any.type_url;
    Ok(item_marker)
}

pub fn parse_page_token_object_id(
    request_message: &impl Message,
    request_page_token: &str,
) -> Result<ObjectId, Status> {
    let page_token = parse_page_token_string(request_message, request_page_token)?;
    match ObjectId::with_string(&page_token) {
        Ok(object_id) => Ok(object_id),
        Err(_) => Err(invalid_page_token_error()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::proto::crusty_cards_api::User;

    #[test]
    fn test_serialize_proto_message_to_hex_string() {
        let mut user = User {
            name: String::from(""),
            display_name: String::from(""),
            create_time: None,
            update_time: None,
        };
        assert_eq!(serialize_proto_message_to_hex_string(&user), "");
        user.display_name = String::from("Test");
        assert_eq!(serialize_proto_message_to_hex_string(&user), "120454657374");
    }

    #[test]
    fn test_valid_page_token() {
        let user = User {
            name: String::from(""),
            display_name: String::from("Hello"),
            create_time: None,
            update_time: None,
        };
        let page_token = create_page_token(&user, String::from("1234567890ABCDEF12345678"));

        // This is only here as a snapshot test to prevent
        // accidental change to this module's API surface.
        assert_eq!(
            page_token,
            "0a183132333435363738393041424344454631323334353637381220dc84\
                            3bca2b2b64308a7d8df9cdf8698063d82a1eb42b04e16f1b2d7b3f8c5bbd"
        );

        let item_marker = parse_page_token_string(&user, &page_token).unwrap();
        assert_eq!(item_marker, "1234567890ABCDEF12345678");
    }

    #[test]
    fn test_invalid_page_token() {
        let user = User {
            name: String::from(""),
            display_name: String::from("Hello"),
            create_time: None,
            update_time: None,
        };
        assert_eq!(
            format!(
                "{}",
                parse_page_token_string(&user, "Invalid page token").err().unwrap()
            ),
            "status: InvalidArgument, message: \"Page token is invalid.\", details: [], metadata: MetadataMap { headers: {} }"
        );
    }

    #[test]
    fn test_changing_request_message() {
        let mut user = User {
            name: String::from(""),
            display_name: String::from("Hello"),
            create_time: None,
            update_time: None,
        };
        let page_token = create_page_token(&user, String::from("1234567890ABCDEF12345678"));
        user.display_name = "World".to_string();
        assert_eq!(
            format!("{}", parse_page_token_string(&user, &page_token).err().unwrap()),
            "status: InvalidArgument, message: \"All request fields must match the previous request.\", details: [], metadata: MetadataMap { headers: {} }"
        );
    }

    #[test]
    fn test_conversion_to_object_id() {
        let user = User {
            name: String::from(""),
            display_name: String::from("Hello"),
            create_time: None,
            update_time: None,
        };
        // Can parse page token created from valid object id.
        let page_token = create_page_token(
            &user,
            ObjectId::with_string("5fd07c5000eb828800a3bfa5")
                .unwrap()
                .to_hex(),
        );
        assert_eq!(
            parse_page_token_object_id(&user, &page_token)
                .unwrap()
                .to_hex(),
            "5fd07c5000eb828800a3bfa5"
        );
        // Can parse page token created from valid object id.
        assert_eq!(format!("{}", parse_page_token_object_id(&user, "1234").unwrap_err()), "status: InvalidArgument, message: \"Page token is invalid.\", details: [], metadata: MetadataMap { headers: {} }");
    }
}
