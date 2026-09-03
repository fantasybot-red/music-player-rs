use anyhow::Result;
use librespot::{
    core::authentication::Credentials,
    oauth::{DeviceAuthClient, OAuthToken},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialsOauth {
    /// Bearer token used for authenticated Spotify API requests
    pub access_token: String,
    /// Long-lived token used to obtain new access tokens
    pub refresh_token: String,

    /// Expiration time of the access token
    #[serde(with = "instant_unix")]
    pub expires_at: Instant,
    /// Type of token
    pub token_type: String,
    /// Permission scopes granted by this token
    pub scopes: Vec<String>,
}

impl CredentialsOauth {
    pub fn load_from_disk() -> Result<Self> {
        let env_credentials_path = std::env::var("CREDENTIALS_PATH")?;
        if !std::path::Path::new(&env_credentials_path).exists() {
            return Err(anyhow::anyhow!(
                "Credentials file not found at path: {}",
                env_credentials_path
            ));
        }
        let file = std::fs::File::open(&env_credentials_path)?;
        let credentials: CredentialsOauth = serde_json::from_reader(file)?;
        Ok(credentials)
    }

    pub fn save_to_disk(&self) -> Result<()> {
        let env_credentials_path = std::env::var("CREDENTIALS_PATH")?;
        let file = std::fs::File::create(&env_credentials_path)?;
        serde_json::to_writer_pretty(file, &self)?;
        Ok(())
    }

    pub async fn get_access_token(
        &mut self,
        device_auth_client: &DeviceAuthClient,
    ) -> Result<Credentials> {
        if self.is_expired() {
            let oauth_token_status = device_auth_client
                .refresh_token_async(&self.refresh_token)
                .await;
            if oauth_token_status.is_err() {
                // If refreshing the token fails, remove the credentials file to force re-authentication
                std::fs::remove_file(std::env::var("CREDENTIALS_PATH")?)?;
            }
            *self = Self::from(oauth_token_status?);
            self.save_to_disk()?;
        }
        Ok(Credentials::with_access_token(self.access_token.clone()))
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

impl From<OAuthToken> for CredentialsOauth {
    fn from(value: OAuthToken) -> Self {
        CredentialsOauth {
            access_token: value.access_token,
            refresh_token: value.refresh_token,
            expires_at: value.expires_at,
            token_type: value.token_type,
            scopes: value.scopes,
        }
    }
}

impl From<CredentialsOauth> for OAuthToken {
    fn from(value: CredentialsOauth) -> Self {
        OAuthToken {
            access_token: value.access_token,
            refresh_token: value.refresh_token,
            expires_at: value.expires_at,
            token_type: value.token_type,
            scopes: value.scopes,
        }
    }
}

mod instant_unix {
    use super::*;

    pub fn serialize<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let now_instant = Instant::now();
        let now_system = SystemTime::now();

        // Calculate the corresponding SystemTime
        let system_time = if *instant >= now_instant {
            now_system + instant.duration_since(now_instant)
        } else {
            now_system - now_instant.duration_since(*instant)
        };

        let unix = system_time
            .duration_since(UNIX_EPOCH)
            .map_err(serde::ser::Error::custom)?
            .as_secs();

        serializer.serialize_u64(unix)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unix = u64::deserialize(deserializer)?;

        let target = UNIX_EPOCH + Duration::from_secs(unix);
        let now_system = SystemTime::now();
        let now_instant = Instant::now();

        let instant = match target.duration_since(now_system) {
            Ok(duration) => now_instant + duration,
            Err(error) => now_instant - error.duration(),
        };

        Ok(instant)
    }
}
