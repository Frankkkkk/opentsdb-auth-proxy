use glob_match::glob_match;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;

const DEFAULT_LISTEN_PORT: &str = "8080";

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub clients: Vec<Client>,
    pub config: ConfigOptions,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Client {
    pub name: String,
    pub metrics: Vec<String>,
    pub auth: Auth,
}

impl Client {
    pub fn can_write(&self, metric: &str) -> bool {
        for m in &self.metrics {
            if glob_match(m, metric) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Auth {
    #[serde(rename = "type")]
    pub auth_type: String,
    pub hash: String,
}

impl Auth {
    pub fn is_valid_token(&self, token: &str) -> bool {
        match self.auth_type.as_str() {
            "sha256" => {
                let mut hasher = Sha256::new();
                hasher.update(token);
                let result = hasher.finalize();
                format!("{:x}", result) == self.hash
            }
            _ => false,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConfigOptions {
    pub opentsdb: Opentsdb,
    pub server: Server,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Opentsdb {
    #[serde(default = "default_opentsdb_url")]
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    #[serde(default = "default_listen_port")]
    pub port: String,
}

fn default_listen_port() -> String {
    DEFAULT_LISTEN_PORT.to_string()
}

fn default_opentsdb_url() -> String {
    env::var("OPENTSDB_URL")
        .expect("OPENTSDB_URL must be set or defined in config file")
        .to_string()
}

pub fn load_config_file(filename: &str) -> Config {
    let yaml_content = fs::read_to_string(filename)
        .unwrap_or_else(|_| panic!("Unable to read config file {}", filename));
    let config: Config = serde_yaml::from_str(&yaml_content).expect("Unable to parse YAML");
    config
}

pub fn try_authenticate_client<'a>(clients: &'a [Client], token: &str) -> Option<&'a Client> {
    clients
        .iter()
        .find(|client| client.auth.is_valid_token(token))
}
