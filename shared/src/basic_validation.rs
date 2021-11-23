use tonic::Status;
use super::grpc_error::empty_request_field_error;

/// A wrapped string used to represent sanitized user input.
/// Successful creation of a ValidatedStringField guarantees
/// that the string is non-empty and contains no whitespace
/// at the beginning or end.
#[derive(Clone)]
pub struct ValidatedStringField {
    string: String,
}

impl ValidatedStringField {
    pub fn new(s: &str, field_name: &str) -> Result<Self, Status> {
        let string = String::from(s.trim());
        if string.is_empty() {
            return Err(super::grpc_error::empty_request_field_error(field_name));
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

macro_rules! bounded_number_field {
    ($struct_name:ident, $lower_bound:expr, $upper_bound:expr, $allow_zero:expr) => {
        /// A wrapped number used to represent bounded user input.
        /// Successful creation of a BoundedNumberField guarantees
        /// that the number falls within a specified range.
        pub struct $struct_name {
            value: i32,
        }

        impl $struct_name {
            pub fn new(value: i32, field_name: &str) -> Result<Self, Status> {
                if (!$allow_zero) && value == 0 {
                    return Err(empty_request_field_error(field_name));
                }

                // TODO - Perform this check when expanding the macro, rather than when calling the generated code.
                if $lower_bound > $upper_bound {
                    return Err(Status::internal(
                        "Lower bound cannot not be greater than upper bound when instantiating BoundedNumberField.",
                    ));
                }
        
                if value < $lower_bound || value > $upper_bound {
                    return Err(Status::invalid_argument(format!(
                        "Request field `{}` must be between {} and {} (inclusive).",
                        field_name, $lower_bound, $upper_bound
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
    }
}

bounded_number_field!(AnswerFieldCount, 1, 3, false);
