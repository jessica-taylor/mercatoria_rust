use async_trait::*;
use chrono::prelude::*;
use serde::{de::DeserializeOwned, *};

#[async_trait]
pub trait Network {
    type Pid: Send + Sync + Serialize + DeserializeOwned + Eq + PartialEq + Clone + 'static;
    fn get_network_pid(&self) -> Self::Pid;
    async fn get_network_time(&self) -> Result<DateTime<Utc>, anyhow::Error>;
    async fn send(&self, to: &Self::Pid, msg: Vec<u8>) -> Result<(), anyhow::Error>;
}
