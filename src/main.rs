#![feature(async_closure)]

use std::{net::{Ipv4Addr, Ipv6Addr}, sync::Arc, collections::HashMap};

use data::{Config, DnsUpdate, Response, DnsRecord, ExtendedResponse};
use tokio::{fs::File, io::AsyncReadExt, time::{self, Duration}, sync::mpsc::{self, Sender}};
use error::ConfigError;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer, filter};

pub mod data;
pub mod error;

pub static ROOT : &str = "https://api.cloudflare.com/client/v4";

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let subscriber = tracing_subscriber::fmt::layer().pretty();
    tracing_subscriber::registry().with(
        subscriber.with_filter(tracing_subscriber::filter::LevelFilter::TRACE)
        .with_filter(filter::filter_fn(|a| {
            a.target().starts_with("cloudflareddns")
        }))).init();

    let config = read_to_config("./Config.toml").await;
    let config = Arc::new(match config {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(cause = %e);
            return;
        }
    });

    tracing::info!("Starting IP polling loop");

    let client = Arc::new(reqwest::Client::new());

    struct Channels<T>(Vec<Sender<T>>);
    impl<T : Clone> Channels<T> {
        async fn send(&self, value : &T) {
            for v in self.0.iter() {
                let _ = v.send(value.clone()).await;
            }
        }
    }

    let mut addrs = Arc::new((public_ip::addr_v4().await, public_ip::addr_v6().await));
    let mut channels = Vec::new();

    for domain in config.domains.iter() {
        let map : HashMap<String, String> = match get_dns_records(&client, &domain.zone_id, &domain.api_key).await {
            Ok(v) => {
                if v.response.success {
                    HashMap::from_iter(v.result.into_iter().filter(|v| v.record_type == "A" || v.record_type == "AAAA").map(|v| (v.name, v.id)))
                } else {
                    tracing::warn!("Erroneous message received from Cloudflare API when querying IDs of entries for zone id {}: {}", domain.zone_id, v.response.errors[0]);
                    continue;
                }
            }
            Err(e) => {
                tracing::error!("Unable to reach Cloudflare API while querying IDs with error {}", e);
                break;
            }
        };

        for entry in domain.entries.iter() {
            let id = match map.get(&entry.name) {
                Some(v) => {
                    v.clone()
                }
                None => {
                    tracing::warn!("Unable to find ID of entry {} for zone id {}. Please make sure the entry name in the config matches the entry in Cloudflare EXACTLY.", entry.name, domain.zone_id);
                    continue;
                }
            };
            let (send, mut recv) = mpsc::channel::<Arc<(Option<Ipv4Addr>, Option<Ipv6Addr>)>>(1);

            let domain = domain.clone();
            let entry = entry.clone();
            let client = client.clone();

            channels.push(send);

            tokio::task::spawn(async move {
                let ipv6 = if entry.record_type == "AAAA" {
                    true
                } else if entry.record_type == "A" {
                    false
                } else {
                    tracing::warn!("Entry {} of zone {} has an improper record type ({:?}). Ignoring entry.", entry.name, domain.zone_id, entry.record_type);
                    return;
                };

                let mut to_change : bool = false;
                let mut update = DnsUpdate { entry, content: String::new() };
                
                while let Some(v) = recv.recv().await {
                    if to_change {
                        match do_update(&client, &domain.zone_id, &id, &domain.api_key, update.clone()).await {
                            Ok(v) => {
                                if v.success {
                                    tracing::info!("Successfully changed the IP address of entry {} to {}.", update.entry.name, update.content);
                                } else {
                                    tracing::warn!("Erroneous message received from Cloudflare API: {}", v.errors[0]);
                                }

                                to_change = false;
                            }
                            Err(e) => {
                                tracing::warn!("Unable to reach Cloudflare API with error {}", e);
                            }
                        }

                        continue;
                    }

                    let addr = if ipv6 {
                        match v.1 {
                            Some(v) => {
                                v.to_string()
                            }
                            None => {
                                continue;
                            }
                        }
                    } else {
                        match v.0 {
                            Some(v) => {
                                v.to_string()
                            }
                            None => {
                                continue;
                            }
                        }
                    };

                    if addr != update.content {
                        update.content = addr;

                        match do_update(&client, &domain.zone_id, &id, &domain.api_key, update.clone()).await {
                            Ok(v) => {
                                if v.success {
                                    tracing::info!("Successfully changed the IP address of entry {} to {}.", update.entry.name, update.content);
                                } else {
                                    tracing::warn!("Erroneous message received from Cloudflare API: {}", v.errors[0]);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Unable to reach Cloudflare API with error {}", e);

                                to_change = true;
                            }
                        }

                        continue;
                    }

                }
            });
        }
    }

    let channels = Channels(channels);

    if config.settings.update_upon_start {
        channels.send(&addrs).await;
    }

    loop {
        time::sleep(Duration::from_millis(config.settings.ip_poll)).await;

        addrs = Arc::new((public_ip::addr_v4().await, public_ip::addr_v6().await));

        channels.send(&addrs).await;
    }
}

async fn read_to_config(path : &str) -> Result<Config, ConfigError> {
    let mut file = File::open(path).await?;
    let mut contents = String::new();

    file.read_to_string(&mut contents).await?;

    Ok(toml::from_str(&contents)?)
}
async fn do_update(client : &reqwest::Client, zone_id : &str, id : &str, auth : &str, update : DnsUpdate) -> Result<Response, reqwest::Error> {
    let v = client.put(format!("{}/zones/{}/dns_records/{}", ROOT, zone_id, id))
        .json(&update)
        .header("Authorization", format!("Bearer {}", auth))
        .send()
        .await?;

    Ok(v.json().await.unwrap())
}

async fn get_dns_records(client : &reqwest::Client, zone_id : &str, auth : &str) -> Result<ExtendedResponse<Vec<DnsRecord>>, reqwest::Error> {
    let v = client.get(format!("{}/zones/{}/dns_records", ROOT, zone_id))
        .header("Authorization", format!("Bearer {}", auth))
        .send()
        .await?;

    Ok(v.json().await.unwrap())
}