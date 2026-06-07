use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub password: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Config {
    pub servers: Vec<ServerConfig>,
}
