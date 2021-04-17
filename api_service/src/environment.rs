pub struct EnvironmentVariables {
    mongo_uri: String,
    mongo_database: String,
    sonic_uri: String,
    sonic_password: String,
}

impl EnvironmentVariables {
    fn get_env_var_or_default(key: &str, default: &str) -> String {
        match std::env::var(key) {
            Ok(value) => value,
            _ => String::from(default),
        }
    }

    pub fn get_mongo_uri(&self) -> &str {
        &self.mongo_uri
    }

    pub fn get_mongo_database(&self) -> &str {
        &self.mongo_database
    }

    pub fn get_sonic_uri(&self) -> &str {
        &self.sonic_uri
    }

    pub fn get_sonic_password(&self) -> &str {
        &self.sonic_password
    }

    pub fn new() -> Self {
        Self {
            mongo_uri: Self::get_env_var_or_default("MONGO_URI", "mongodb://localhost:27017/"),
            mongo_database: Self::get_env_var_or_default("MONGO_DATABASE", "crustyCards"),
            sonic_uri: Self::get_env_var_or_default("SONIC_URI", "127.0.0.1:1491"),
            sonic_password: Self::get_env_var_or_default("SONIC_PASSWORD", "SecretPassword"),
        }
    }
}
