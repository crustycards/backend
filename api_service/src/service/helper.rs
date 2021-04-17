use tonic::Status;

pub fn missing_request_field_error(field_name: &str) -> Status {
    Status::invalid_argument(format!(
        "Request is missing required field `{}`.",
        field_name
    ))
}

pub fn invalid_page_token_error() -> Status {
    Status::invalid_argument("Page token is invalid.")
}

pub fn batch_create_differing_parent_error() -> Status {
    Status::invalid_argument("Since the `parent` field is specified, each member in `requests` should either have the same parent or no parent specified.")
}

pub fn batch_request_exceeds_request_limit_error(limit: i32) -> Status {
    Status::invalid_argument(format!("Request exceeds limit of {} items.", limit))
}

#[cfg(test)]
pub mod test {
    use super::super::super::mongo::user_collection::MockUserCollection;
    use super::super::super::search_client::MockSearchClient;
    use super::super::user_service_impl::UserServiceImpl;
    use std::sync::Arc;
    pub async fn get_local_test_user_service(
        mutate_mock_search_client_or: Option<Box<dyn Fn(&mut MockSearchClient) -> ()>>,
    ) -> UserServiceImpl {
        let mut mock_search_client = MockSearchClient::new();
        match mutate_mock_search_client_or {
            Some(mutate_mock_search_client) => mutate_mock_search_client(&mut mock_search_client),
            None => {}
        };
        let user_service_impl = UserServiceImpl::new(
            Arc::from(MockUserCollection::new()),
            Arc::from(mock_search_client),
        );
        return user_service_impl;
    }
}
