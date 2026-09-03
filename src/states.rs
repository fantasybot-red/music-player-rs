use std::sync::Arc;

use crate::players::{SpotifyClient, TikTokClient};
use anyhow::Result;
use soundcloud_rs::{Client as SoundCloudClient, ClientBuilder};

#[derive(Clone)]
pub struct BotState {
    pub soundcloud_client: Arc<SoundCloudClient>,
    pub tiktok_client: TikTokClient,
    pub spotify_client: SpotifyClient,
}

impl BotState {
    pub async fn new() -> Result<Self> {
        let client = ClientBuilder::new()
            .with_max_retries(3)
            .with_retry_on_401(true)
            .build()
            .await?;
        let tiktok_client = TikTokClient::new()?;
        let spotify_client = SpotifyClient::new()?;
        Ok(Self {
            soundcloud_client: Arc::new(client),
            tiktok_client,
            spotify_client,
        })
    }
}
