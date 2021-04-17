use tonic::Status;

#[derive(Clone)]
pub struct ValidatedStringField {
    string: String,
}

impl ValidatedStringField {
    pub fn new(s: &str, field_name: &str) -> Result<Self, Status> {
        let string = String::from(s.trim());
        if string.is_empty() {
            return Err(grpc_error::empty_request_field_error(field_name));
        }
        Ok(Self { string })
    }

    pub fn get_string(&self) -> &str {
        &self.string
    }

    pub fn take_string(self) -> String {
        self.string
    }
}

pub struct BoundedNumberField {
    value: i32,
}

impl BoundedNumberField {
    pub fn new(value: i32, min: i32, max: i32, field_name: &str) -> Result<Self, Status> {
        if min > max {
            return Err(Status::internal(
                "Min cannot not be greater than max when instantiating BoundedNumberField.",
            ));
        }

        if value < min || value > max {
            return Err(Status::invalid_argument(format!(
                "Request field `{}` must be between {} and {} (inclusive).",
                field_name, min, max
            )));
        }

        Ok(Self { value })
    }

    pub fn get_value(&self) -> &i32 {
        &self.value
    }

    pub fn take_value(self) -> i32 {
        self.value
    }
}
