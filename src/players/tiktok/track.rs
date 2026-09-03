use crate::players::{Ffmpeg, QueueResult, TikTokClient, TrackMetadata};
use anyhow::Result as AResult;
use async_trait::async_trait;
use songbird::{
    input::{AudioStream, AudioStreamError, AuxMetadata, Compose, Input},
    tracks::Track,
};
use std::sync::Arc;
use symphonia::core::io::MediaSource;
use wreq::Client as WreqClient;
use wreq_util::Emulation;

#[derive(Clone)]
pub struct TikTokLive {
    pub metadata: TrackMetadata,
    pub username: String,
    pub client: TikTokClient,
}

impl TikTokLive {
    pub fn check_url(url: &str) -> bool {
        let Ok(parsed) = url::Url::parse(url) else {
            return false;
        };

        parsed
            .host_str()
            .map(|host| host == "tiktok.com" || host.ends_with(".tiktok.com"))
            .unwrap_or(false)
    }

    pub async fn new(client: &TikTokClient, username: String) -> AResult<Self> {
        let sigi_state = client.fetch_stream_url(&username).await?;
        if sigi_state.live_room.is_none() {
            return Err(anyhow::anyhow!("No live room user info available"));
        }
        let live_room_user_info = sigi_state.live_room.unwrap().live_room_user_info;
        let cover_url = if !live_room_user_info.live_room.cover_url.is_empty() {
            Some(live_room_user_info.live_room.cover_url)
        } else if !live_room_user_info.user.avatar_medium.is_empty() {
            Some(live_room_user_info.user.avatar_medium)
        } else {
            None
        };
        let metadata = TrackMetadata::tiktok_live(
            live_room_user_info.live_room.title,
            format!(
                "https://www.tiktok.com/@{}/live",
                live_room_user_info.user.unique_id
            ),
            cover_url,
            vec![live_room_user_info.user.nickname],
        );
        let obj = Self {
            username,
            metadata,
            client: client.clone(),
        };
        obj.fetch_stream_url().await?;
        Ok(obj)
    }

    pub async fn from_url(client: &TikTokClient, url: &str) -> AResult<QueueResult> {
        let original_url = url::Url::parse(url)?;
        let resolved_url = if original_url
            .domain()
            .and_then(|d| Some(d == "tiktok.com" || d == "www.tiktok.com"))
            .unwrap_or(false)
        {
            original_url
        } else if original_url
            .domain()
            .map(|d| d.ends_with(".tiktok.com"))
            .unwrap_or(false)
        {
            let client = WreqClient::builder()
                .emulation(Emulation::Chrome149)
                .build()?;
            let redirected_url = client.get(url).send().await?.uri().to_string();
            println!("Redirected URL: {}", redirected_url);
            url::Url::parse(&redirected_url)?
        } else {
            return Err(anyhow::anyhow!("Invalid TikTok URL"));
        };

        if resolved_url.domain() != Some("www.tiktok.com")
            && resolved_url.domain() != Some("tiktok.com")
        {
            return Err(anyhow::anyhow!("Invalid TikTok URL"));
        }

        let username = resolved_url
            .path_segments()
            .and_then(|mut segments| segments.next())
            .and_then(|segment| segment.strip_prefix('@'))
            .filter(|username| !username.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Invalid TikTok URL"))?;

        let tiktok_live = Self::new(client, username.to_owned()).await?;
        Ok(QueueResult::Track(tiktok_live.try_into()?))
    }

    pub fn url(&self) -> String {
        format!("https://www.tiktok.com/@{}/live", self.username)
    }

    pub async fn fetch_stream_url(&self) -> Result<String, AudioStreamError> {
        let sigi_state = self
            .client
            .fetch_stream_url(&self.username)
            .await
            .map_err(|e| AudioStreamError::Fail(e.into()))?;

        if sigi_state.live_room.is_none() {
            return Err(AudioStreamError::Fail(
                anyhow::anyhow!("No live room user info available").into(),
            ));
        }

        let stream_data = sigi_state
            .live_room
            .unwrap()
            .live_room_user_info
            .live_room
            .stream_data;
        if stream_data.is_none() {
            return Err(AudioStreamError::Fail(
                anyhow::anyhow!("No stream data available").into(),
            ));
        }
        let stream_data = stream_data.unwrap().pull_data.stream_data;
        if stream_data.is_none() {
            return Err(AudioStreamError::Fail(
                anyhow::anyhow!("No stream data available").into(),
            ));
        }
        let flv_audio = stream_data.unwrap().data.ao.main.flv;
        let status = self
            .client
            .check_stream(&flv_audio)
            .await
            .map_err(|e| AudioStreamError::Fail(e.into()))?;
        if !status {
            return Err(AudioStreamError::Fail(
                anyhow::anyhow!("Stream is not available").into(),
            ));
        }
        Ok(flv_audio.to_owned())
    }
}

#[async_trait]
impl Compose for TikTokLive {
    fn create(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        Err(AudioStreamError::Unsupported)
    }

    async fn create_async(
        &mut self,
    ) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        let stream_url = self.fetch_stream_url().await?;
        let mut ffmpeg_input =
            Ffmpeg::new(&stream_url).map_err(|e| AudioStreamError::Fail(e.into()))?;
        ffmpeg_input.create_async().await
    }

    fn should_create_async(&self) -> bool {
        true
    }

    async fn aux_metadata(&mut self) -> Result<AuxMetadata, AudioStreamError> {
        return Ok(AuxMetadata {
            artist: Some(self.metadata.artists.join(", ")),
            title: Some(self.metadata.title.clone()),
            thumbnail: self.metadata.artwork.clone(),
            ..Default::default()
        });
    }
}

impl TryFrom<TikTokLive> for Track {
    fn try_from(value: TikTokLive) -> Result<Self, Self::Error> {
        let metadata = value.metadata.clone();
        let input = Input::Lazy(Box::new(value));
        let track = Track::new_with_data(input, Arc::new(metadata));
        Ok(track)
    }

    type Error = anyhow::Error;
}
