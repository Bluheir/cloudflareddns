use std::io::Error as IoError;
use serde::{Serialize, Deserialize};
use toml::de::Error as TomlError;

use thiserror::Error;

#[derive(Error, Debug)]
#[error(transparent)]
pub enum ConfigError {
    #[error("Config.toml file not found or cannot be read")]
    FileError(#[from]IoError),
    #[error("Config.toml file is in an invalid format")]
    ParsingError(#[from]TomlError)
}



#[derive(Error, Debug)]
pub enum UpdateError {
    #[error("cannot put ip address changes")]
    ReqError(#[from]reqwest::Error),
}

#[derive(Serialize, Deserialize, Clone, Error, Debug)]
#[error("{message:?} (code : {code:?})")]
pub struct CloudFlareError {
    pub code : u32,
    pub message : String,
}