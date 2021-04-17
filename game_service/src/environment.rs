pub struct EnvironmentVariables {
    api_uri: String,
    amqp_uri: String,
}

impl EnvironmentVariables {
    pub fn get_api_uri(&self) -> &str {
        &self.api_uri
    }

    pub fn get_amqp_uri(&self) -> &str {
        &self.amqp_uri
    }

    fn get_env_var_or_panic(key: &str) -> String {
        match std::env::var(key) {
            Ok(value) => value,
            _ => panic!(format!("Unable to load environment variable `{}`.", key)),
        }
    }

    pub fn new() -> EnvironmentVariables {
        EnvironmentVariables {
            api_uri: EnvironmentVariables::get_env_var_or_panic("API_URI"),
            amqp_uri: EnvironmentVariables::get_env_var_or_panic("AMQP_URI"),
        }
    }
}
