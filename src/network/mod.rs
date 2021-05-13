use async_trait::*;
use chrono::prelude::*;
use futures_lite::prelude::*;
use serde::{de::DeserializeOwned, *};

pub mod graph;

#[async_trait]
pub trait Network: Sized {
    type Pid: Send + Sync + Serialize + DeserializeOwned + Eq + PartialEq + Clone + 'static;
    fn get_network_pid(&self) -> Self::Pid;
    async fn get_network_time(&self) -> Result<DateTime<Utc>, anyhow::Error>;
    async fn send(&self, to: &Self::Pid, msg: Vec<u8>) -> Result<(), anyhow::Error>;

    type Incoming: Stream<Item = (Self::Pid, Vec<u8>)> + Unpin + Send + 'static;
    type InitParams: Send;
    async fn bootstrap(params: Self::InitParams) -> Result<(Self, Self::Incoming), anyhow::Error>;
}
