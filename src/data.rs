use serde::{Deserialize, Serialize};

use crate::error::CloudFlareError;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub settings : Settings,
    pub domains : Vec<DomainInfo>
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Settings {
    /// The interval of rechecking the public ip address in milliseconds
    pub ip_poll : u64,
    pub update_upon_start : bool
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DomainInfo {
    pub zone_id : String,
    pub api_key : String,
    pub entries : Vec<Entry>
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Entry {
    pub name : String,
    #[serde(default = "default_ttl")]
    pub ttl : usize,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxied : Option<bool>,
    #[serde(default = "default_type")]
    #[serde(rename = "type")]
    pub record_type : String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags : Option<Vec<String>>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment : Option<String>
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DnsUpdate {
    #[serde(flatten)]
    pub entry : Entry,
    pub content : String
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Response {
    pub success : bool,
    pub errors : Vec<CloudFlareError>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExtendedResponse<T> {
    #[serde(flatten)]
    pub response : Response,
    pub result : T
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DnsRecord {
    pub content : String,
    pub id : String,
    pub name : String,
    #[serde(rename = "type")]
    pub record_type : String,
}

pub fn default_type() -> String {
    "A".to_owned()
}

pub fn default_ttl() -> usize {
    1
}
pub fn default_tags() -> Vec<String> {
    Vec::new()
}
