use super::environment::EnvironmentVariables;
use mockall::automock;
use shared::proto::User;
use shared::resource_name::{ParseNameError, UserName};
use sonic_channel::*;

#[automock]
pub trait SearchClient: Send + Sync {
    fn index_user(&self, user: &User, overwrite: bool) -> Result<(), IndexUserError>;
    fn index_user_or_print_to_std_err(&self, user: &User, overwrite: bool);
    fn wipe_user_index(&self) -> Result<(), sonic_channel::result::Error>;
    fn search_users(&self, query: &str) -> Result<Vec<UserName>, result::Error>;
    fn autocomplete_search_users(&self, query: &str) -> Result<Vec<String>, result::Error>;
}

pub enum IndexUserError {
    ParseNameError(ParseNameError),
    SonicError(sonic_channel::result::Error),
}

impl std::fmt::Display for IndexUserError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseNameError(parse_name_error) => {
                formatter.write_str(&format!("{}", parse_name_error))
            }
            Self::SonicError(sonic_error) => formatter.write_str(&format!("{}", sonic_error)),
        }
    }
}

pub struct SonicSearchClient {
    sonic_search_channel: SearchChannel,
    sonic_ingest_channel: IngestChannel,
}

impl SonicSearchClient {
    const SONIC_USER_COLLECTION: &'static str = "users";
    const SONIC_USER_BUCKET_DEFAULT: &'static str = "default";

    pub fn new(env_vars: &EnvironmentVariables) -> Result<Self, result::Error> {
        Ok(Self {
            sonic_search_channel: SearchChannel::start(
                env_vars.get_sonic_uri(),
                env_vars.get_sonic_password(),
            )?,
            sonic_ingest_channel: IngestChannel::start(
                env_vars.get_sonic_uri(),
                env_vars.get_sonic_password(),
            )?,
        })
    }

    fn get_user_text_tokens(user: &User) -> Result<String, ParseNameError> {
        let user_name_hex_string = UserName::new_from_str(&user.name)?
            .take_object_id()
            .to_hex();
        Ok(format!("{} {}", &user.display_name, user_name_hex_string))
    }
}

impl SearchClient for SonicSearchClient {
    fn index_user(&self, user: &User, overwrite: bool) -> Result<(), IndexUserError> {
        let user_text_tokens = match Self::get_user_text_tokens(&user) {
            Ok(tokens) => tokens,
            Err(err) => return Err(IndexUserError::ParseNameError(err)),
        };
        if overwrite {
            match self.sonic_ingest_channel.flusho(
                Self::SONIC_USER_COLLECTION,
                Self::SONIC_USER_BUCKET_DEFAULT,
                &user.name,
            ) {
                Ok(_) => {}
                Err(err) => return Err(IndexUserError::SonicError(err)),
            };
        }
        match self.sonic_ingest_channel.push(
            Self::SONIC_USER_COLLECTION,
            Self::SONIC_USER_BUCKET_DEFAULT,
            &user.name,
            &user_text_tokens,
        ) {
            Ok(_) => {}
            Err(err) => return Err(IndexUserError::SonicError(err)),
        };
        Ok(())
    }

    fn index_user_or_print_to_std_err(&self, user: &User, overwrite: bool) {
        match self.index_user(user, overwrite) {
            Ok(_) => {}
            Err(err) => eprintln!("Error updating sonic search index: {}", err),
        };
    }

    fn wipe_user_index(&self) -> Result<(), sonic_channel::result::Error> {
        self.sonic_ingest_channel
            .flushc(Self::SONIC_USER_COLLECTION)?;
        Ok(())
    }

    fn search_users(&self, query: &str) -> Result<Vec<UserName>, result::Error> {
        let user_name_strings: Vec<String> = self.sonic_search_channel.query(
            Self::SONIC_USER_COLLECTION,
            Self::SONIC_USER_BUCKET_DEFAULT,
            query,
        )?;
        let mut user_names: Vec<UserName> = Vec::new();
        for user_name_string in &user_name_strings {
            match UserName::new_from_str(user_name_string) {
                Ok(user_name) => user_names.push(user_name),
                _ => {} // Ignore invalid returned user names.
            };
        }
        Ok(user_names)
    }

    fn autocomplete_search_users(&self, query: &str) -> Result<Vec<String>, result::Error> {
        self.sonic_search_channel.suggest(
            Self::SONIC_USER_COLLECTION,
            Self::SONIC_USER_BUCKET_DEFAULT,
            query,
        )
    }
}
