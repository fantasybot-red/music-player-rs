use anyhow::{Context as _, Result};
use librespot::{
    core::{Session, SessionConfig, SpotifyUri, cache::Cache},
    metadata::{Metadata, Track},
    oauth::{DeviceAuthClient, DeviceAuthClientBuilder, DeviceAuthorization},
};
use std::sync::Arc;
use tokio::sync::RwLock;

const SPOTIFY_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";
// Scope read: https://github.com/librespot-org/librespot/blob/dev/core/src/token.rs#L4
const SPOTIFY_SCOPE: [&str; 1] = ["streaming"];
const MAX_SEARCH_RESULTS: usize = 25;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpotifySearchResult {
    pub name: String,
    pub artists: Vec<String>,
    pub uri: String,
}

#[derive(Clone)]
pub struct SpotifyClient {
    session: Arc<RwLock<Session>>,
    oauth_client: Arc<DeviceAuthClient>,
}

impl SpotifyClient {
    fn create_session() -> Result<Session> {
        let session_config: SessionConfig = SessionConfig::default();
        let cache_config = Cache::new(None::<String>, None, None, Some(1024 * 1024 * 1024))?;
        let session = Session::new(session_config, Some(cache_config));
        Ok(session)
    }

    pub fn new() -> Result<Self> {
        let session = Arc::new(RwLock::new(Self::create_session()?));
        let oauth_client = Arc::new(
            DeviceAuthClientBuilder::new(SPOTIFY_CLIENT_ID, SPOTIFY_SCOPE.to_vec()).build()?,
        );
        Ok(Self {
            session,
            oauth_client,
        })
    }

    async fn check_ok(&self) -> bool {
        let session = self.session.read().await;
        !session.is_invalid() && !session.username().is_empty()
    }

    pub async fn request_device_code(&self) -> Result<DeviceAuthorization> {
        Ok(self.oauth_client.request_device_code_async().await?)
    }

    pub async fn wait_for_authentication(&self, device_auth: &DeviceAuthorization) -> Result<()> {
        if !self.check_ok().await {
            self.session.read().await.shutdown();
            *self.session.write().await = Self::create_session()?;
        }
        let oauth_token = self.oauth_client.poll_for_token_async(device_auth).await?;
        let mut credentials_oauth = super::CredentialsOauth::from(oauth_token);
        credentials_oauth.save_to_disk()?;
        let credentials = credentials_oauth
            .get_access_token(&self.oauth_client)
            .await?;
        let session = self.session.read().await;
        session.connect(credentials, false).await?;
        Ok(())
    }

    pub async fn authenticate(&self) -> Result<()> {
        let credentials = super::CredentialsOauth::load_from_disk()?
            .get_access_token(&self.oauth_client)
            .await?;
        let session = self.session.read().await;
        session.connect(credentials, false).await?;
        Ok(())
    }

    pub async fn get_session(&self) -> Result<Session> {
        if !self.check_ok().await {
            self.session.read().await.shutdown();
            *self.session.write().await = Self::create_session()?;
            self.authenticate().await?;
        }
        Ok(self.session.read().await.to_owned())
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SpotifySearchResult>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let search_uri = format!(
            "spotify:search:{}",
            query.split_whitespace().collect::<Vec<_>>().join("+")
        );
        let session = self.get_session().await?;
        let context = session
            .spclient()
            .get_context(&search_uri)
            .await
            .context("Spotify search context request failed")?;

        let uris = context
            .pages
            .iter()
            .flat_map(|page| page.tracks.iter())
            .filter_map(|track| track.uri.as_deref())
            .filter_map(|uri| SpotifyUri::from_uri(uri).ok())
            .filter(|uri| matches!(uri, SpotifyUri::Track { .. }))
            .take(MAX_SEARCH_RESULTS)
            .collect::<Vec<_>>();

        let mut results = Vec::with_capacity(uris.len());
        for uri in uris {
            let track = Track::get(&session, &uri).await.with_context(|| {
                format!("Failed to load Spotify search result {}", uri.to_uri())
            })?;
            results.push(SpotifySearchResult {
                name: track.name,
                artists: track
                    .artists
                    .iter()
                    .map(|artist| artist.name.clone())
                    .collect(),
                uri: uri.to_uri(),
            });
        }

        Ok(results)
    }
}
