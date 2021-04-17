use super::super::mongo::user_collection::UserCollection;
use shared::resource_name::*;
use shared::basic_validation::ValidatedStringField;
use shared::proto_validation::{
    OptionalField, ValidatedColorScheme, ValidatedGameConfig, ValidatedOAuthCredentials,
};
use super::super::search_client::SearchClient;
use super::helper::*;
use super::profile_image_handler::ProfileImageHandler;
use shared::proto::user_service_server::UserService;
use shared::proto::*;
use std::collections::HashSet;
use std::iter::FromIterator;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct UserServiceImpl {
    user_collection: Arc<dyn UserCollection>,
    search_client: Arc<dyn SearchClient>,
    profile_image_handler: ProfileImageHandler,
}

impl UserServiceImpl {
    pub fn new(
        user_collection: Arc<dyn UserCollection>,
        search_client: Arc<dyn SearchClient>,
    ) -> UserServiceImpl {
        let profile_image_handler = ProfileImageHandler::new();
        let user_service_impl = UserServiceImpl {
            user_collection,
            search_client,
            profile_image_handler,
        };
        return user_service_impl;
    }
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn get_user(&self, request: Request<GetUserRequest>) -> Result<Response<User>, Status> {
        let user_name =
            match UserName::new(&ValidatedStringField::new(&request.get_ref().name, "name")?) {
                Ok(user_name) => user_name,
                Err(err) => return Err(err.to_status()),
            };

        Ok(Response::new(
            self.user_collection.get_user(user_name).await?,
        ))
    }

    async fn update_user(
        &self,
        request: Request<UpdateUserRequest>,
    ) -> Result<Response<User>, Status> {
        let update_mask = match &request.get_ref().update_mask {
            Some(update_mask) => update_mask,
            None => return Err(missing_request_field_error("update_mask")),
        };

        let user = match &request.get_ref().user {
            Some(user) => user,
            None => return Err(missing_request_field_error("user")),
        };

        let user_name = match UserName::new(&ValidatedStringField::new(&user.name, "user.name")?) {
            Ok(user_name) => user_name,
            Err(err) => return Err(err.to_status()),
        };

        let update_fields: HashSet<String> = HashSet::from_iter(update_mask.paths.iter().cloned());
        let updated_display_name_or = if update_fields.contains("display_name") {
            Some(ValidatedStringField::new(
                &user.display_name,
                "user.display_name",
            )?)
        } else {
            None
        };

        let updated_user = match updated_display_name_or {
            Some(updated_display_name) => {
                self.user_collection
                    .update_user(user_name, updated_display_name)
                    .await?
            }
            None => self.user_collection.get_user(user_name).await?,
        };

        self.search_client
            .index_user_or_print_to_std_err(&updated_user, true);

        Ok(Response::new(updated_user))
    }

    async fn get_user_settings(
        &self,
        request: Request<GetUserSettingsRequest>,
    ) -> Result<Response<UserSettings>, Status> {
        let user_settings_name = match UserSettingsName::new(&ValidatedStringField::new(
            &request.get_ref().name,
            "name",
        )?) {
            Ok(user_settings_name) => user_settings_name,
            Err(err) => return Err(err.to_status()),
        };

        Ok(Response::new(
            self.user_collection
                .get_user_settings(user_settings_name)
                .await?,
        ))
    }

    async fn update_user_settings(
        &self,
        request: Request<UpdateUserSettingsRequest>,
    ) -> Result<Response<UserSettings>, Status> {
        let update_mask = match &request.get_ref().update_mask {
            Some(update_mask) => update_mask,
            None => return Err(missing_request_field_error("update_mask")),
        };

        let user_settings = match &request.get_ref().user_settings {
            Some(user_settings) => user_settings,
            None => return Err(missing_request_field_error("user_settings")),
        };

        let user_settings_name = match UserSettingsName::new(&ValidatedStringField::new(
            &user_settings.name,
            "user_settings.name",
        )?) {
            Ok(user_settings_name) => user_settings_name,
            Err(err) => return Err(err.to_status()),
        };

        let update_fields: HashSet<String> = HashSet::from_iter(update_mask.paths.iter().cloned());

        let color_scheme_or = if update_fields.contains("color_scheme") {
            Some(ValidatedColorScheme::new(user_settings.color_scheme)?)
        } else {
            None
        };

        let quick_start_game_config_or = if update_fields.contains("quick_start_game_config") {
            match &user_settings.quick_start_game_config {
                Some(quick_start_game_config) => Some(OptionalField::Set(
                    ValidatedGameConfig::new(quick_start_game_config.clone())?,
                )),
                None => Some(OptionalField::Unset),
            }
        } else {
            None
        };

        Ok(Response::new(
            self.user_collection
                .update_user_settings(
                    user_settings_name,
                    color_scheme_or,
                    quick_start_game_config_or,
                )
                .await?,
        ))
    }

    async fn get_user_profile_image(
        &self,
        request: Request<GetUserProfileImageRequest>,
    ) -> Result<Response<UserProfileImage>, Status> {
        let user_profile_image_name = match UserProfileImageName::new(&ValidatedStringField::new(
            &request.get_ref().name,
            "name",
        )?) {
            Ok(user_profile_image_name) => user_profile_image_name,
            Err(err) => return Err(err.to_status()),
        };

        self.user_collection
            .assert_user_exists(user_profile_image_name.clone().to_user_name())
            .await?;

        Ok(Response::new(
            self.profile_image_handler
                .get_profile_image(&user_profile_image_name),
        ))
    }

    async fn update_user_profile_image(
        &self,
        request: Request<UpdateUserProfileImageRequest>,
    ) -> Result<Response<UserProfileImage>, Status> {
        let update_mask = match &request.get_ref().update_mask {
            Some(update_mask) => update_mask,
            None => return Err(missing_request_field_error("update_mask")),
        };

        let user_profile_image = match &request.get_ref().user_profile_image {
            Some(user_profile_image) => user_profile_image,
            None => return Err(missing_request_field_error("user_profile_image")),
        };

        let user_profile_image_name = match UserProfileImageName::new(&ValidatedStringField::new(
            &user_profile_image.name,
            "user_profile_image.name",
        )?) {
            Ok(user_profile_image_name) => user_profile_image_name,
            Err(err) => return Err(err.to_status()),
        };

        self.user_collection
            .assert_user_exists(user_profile_image_name.clone().to_user_name())
            .await?;

        let update_fields: HashSet<String> = HashSet::from_iter(update_mask.paths.iter().cloned());

        let updated_user_profile_image = if update_fields.contains("image_data") {
            if user_profile_image.image_data.is_empty() {
                self.profile_image_handler
                    .clear_profile_image(&user_profile_image_name);
                UserProfileImage {
                    name: user_profile_image_name.clone_str(),
                    image_data: Vec::new(),
                }
            } else {
                self.profile_image_handler.set_profile_image(
                    &user_profile_image_name,
                    user_profile_image.image_data.to_vec(),
                )
            }
        } else {
            self.profile_image_handler
                .get_profile_image(&user_profile_image_name)
        };

        Ok(Response::new(updated_user_profile_image))
    }

    async fn get_or_create_user(
        &self,
        request: Request<GetOrCreateUserRequest>,
    ) -> Result<Response<User>, Status> {
        let oauth_credentials = match &request.get_ref().oauth_credentials {
            Some(oauth_credentials) => oauth_credentials,
            None => return Err(missing_request_field_error("oauth_credentials")),
        };

        let validated_oauth_credentials =
            ValidatedOAuthCredentials::new(oauth_credentials, "oauth_credentials")?;

        let user = match &request.get_ref().user {
            Some(user) => user,
            None => return Err(missing_request_field_error("user")),
        };

        let display_name = ValidatedStringField::new(&user.display_name, "user.display_name")?;

        let updated_user = self
            .user_collection
            .get_or_create_user(validated_oauth_credentials, display_name)
            .await?;

        self.search_client
            .index_user_or_print_to_std_err(&updated_user, true);

        Ok(Response::new(updated_user))
    }

    async fn user_search(
        &self,
        request: Request<UserSearchRequest>,
    ) -> Result<Response<UserSearchResponse>, Status> {
        let user_names = match self.search_client.search_users(&request.get_ref().query) {
            Ok(user_names) => user_names,
            _ => return Err(Status::unknown("Failed to fetch search results.")),
        };
        let users = match self.user_collection.get_users_from_names(user_names).await {
            Ok(users) => users,
            _ => return Err(Status::unknown("Failed to fetch search results.")),
        }
        .into_iter()
        .filter_map(|user_or| user_or)
        .collect();
        Ok(Response::new(UserSearchResponse { users }))
    }

    async fn autocomplete_user_search(
        &self,
        request: Request<AutocompleteUserSearchRequest>,
    ) -> Result<Response<AutocompleteUserSearchResponse>, Status> {
        let autocomplete_entries: Vec<String> = match self
            .search_client
            .autocomplete_search_users(&request.get_ref().query)
        {
            Ok(autocomplete_entries) => autocomplete_entries,
            _ => return Err(Status::unknown("Failed to fetch autocomplete results.")),
        };
        Ok(Response::new(AutocompleteUserSearchResponse {
            autocomplete_entries,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::super::helper::test::get_local_test_user_service;
    use super::*;

    #[tokio::test]
    async fn create_and_retrieve_nonexistent_user() {
        let user_service = get_local_test_user_service(None).await;

        // Call get_or_create for user that doesn't exist yet without providing fallback user data.
        {
            let oauth_credentials = OAuthCredentials {
                oauth_provider: String::from("google"),
                oauth_id: String::from("1234"),
            };
            let get_or_create_user_request = GetOrCreateUserRequest {
                oauth_credentials: Some(oauth_credentials),
                user: None,
            };
            let create_err = user_service
                .get_or_create_user(Request::new(get_or_create_user_request))
                .await
                .unwrap_err();
            assert_eq!(
                create_err.to_string(),
                "status: InvalidArgument, message: \"Request is missing required field `user`.\", details: [], metadata: MetadataMap { headers: {} }"
            );
        }

        // Retrieve user by name.
        {
            let get_user_request = GetUserRequest {
                name: String::from("users/fake_user_name"),
            };
            let get_by_name_err = user_service
                .get_user(Request::new(get_user_request))
                .await
                .unwrap_err();
            assert_eq!(
                get_by_name_err.to_string(),
                "status: NotFound, message: \"Resource with name `users/fake_user_name` does not exist.\", details: [], metadata: MetadataMap { headers: {} }"
            );
        }
    }
}
