use tonic::Status;

pub fn missing_request_field_error(field_name: &str) -> Status {
    Status::invalid_argument(format!(
        "Request is missing required field `{}`.",
        field_name
    ))
}

pub fn empty_request_field_error(field_name: &str) -> Status {
    Status::invalid_argument(format!("Request field `{}` must not be blank.", field_name))
}

pub fn negative_request_field_error(field_name: &str) -> Status {
    Status::invalid_argument(format!(
        "Request field `{}` must not be negative.",
        field_name
    ))
}
