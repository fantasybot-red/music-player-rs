use crate::players::{Ffmpeg, QueueResult, TrackMetadata};
use anyhow::Result as AResult;
use async_trait::async_trait;
use songbird::{
    input::{AudioStream, AudioStreamError, AuxMetadata, Compose, Input},
    tracks::Track,
};
use soundcloud_rs::{Client as SoundCloudClient, Identifier, ResolvedResource, Track as SCTrack};
use std::sync::Arc;
use symphonia::core::io::MediaSource;

#[derive(Clone)]
pub struct SoundCloudTrack {
    pub track: SCTrack,
    pub client: Arc<SoundCloudClient>,
}

impl SoundCloudTrack {
    pub fn check_url(url: &str) -> bool {
        let Ok(parsed) = url::Url::parse(url) else {
            return false;
        };

        matches!(
            parsed.host_str(),
            Some("soundcloud.com") | Some("www.soundcloud.com") | Some("m.soundcloud.com")
        )
    }

    pub async fn new(client: &Arc<SoundCloudClient>, track: SCTrack) -> AResult<Self> {
        Ok(Self {
            track,
            client: client.clone(),
        })
    }

    pub async fn from_url(client: &Arc<SoundCloudClient>, url: &str) -> AResult<QueueResult> {
        let resolved = client.resolve_url(url).await?;
        match resolved {
            ResolvedResource::Track(track) => {
                let sc_track = Self::new(client, track).await?;
                Ok(QueueResult::Track(sc_track.try_into()?))
            }
            ResolvedResource::Playlist(playlist) => {
                let mut tracks = Vec::new();
                for track in playlist.tracks.unwrap_or_default() {
                    let Some(id) = track.id else {
                        continue;
                    };
                    let loaded_track = client.get_track(&Identifier::Id(id)).await?;
                    let sc_track = Self::new(client, loaded_track).await?;
                    tracks.push(sc_track.try_into()?);
                }
                Ok(QueueResult::Playlist(crate::players::PlaylistQueue {
                    name: playlist.title.unwrap_or_default(),
                    link: playlist.permalink_url.unwrap_or_default(),
                    tracks,
                    artwork: playlist.artwork_url,
                }))
            }
            ResolvedResource::User(_user) => {
                return Err(anyhow::anyhow!("Link SoundCloud không hợp lệ"));
            }
            ResolvedResource::SystemPlaylist(system_playlist) => {
                let mut tracks = Vec::new();
                for track in system_playlist.tracks.unwrap_or_default() {
                    let Some(id) = track.id else {
                        continue;
                    };
                    let loaded_track = client.get_track(&Identifier::Id(id)).await?;
                    let sc_track = Self::new(client, loaded_track).await?;
                    tracks.push(sc_track.try_into()?);
                }
                Ok(QueueResult::Playlist(crate::players::PlaylistQueue {
                    name: system_playlist.title.unwrap_or_default(),
                    link: system_playlist.permalink_url.unwrap_or_default(),
                    tracks,
                    artwork: system_playlist.artwork_url,
                }))
            }
        }
    }

    pub fn url(&self) -> Option<&str> {
        self.track.permalink_url.as_deref()
    }

    pub async fn fetch_stream_url(&self) -> Result<String, AudioStreamError> {
        let id = self
            .track
            .id
            .ok_or_else(|| AudioStreamError::Fail(anyhow::anyhow!("Track ID is None").into()))?;
        self.client
            .get_stream_url(&Identifier::Id(id), None)
            .await
            .map_err(|error| AudioStreamError::Fail(error.into()))
    }
}

#[async_trait]
impl Compose for SoundCloudTrack {
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
            artist: Some(
                self.track
                    .user
                    .clone()
                    .and_then(|user| user.username)
                    .unwrap_or_default(),
            ),
            title: Some(self.track.title.clone().unwrap_or_default()),
            thumbnail: self.track.artwork_url.clone(),
            ..Default::default()
        });
    }
}

impl TryFrom<SoundCloudTrack> for Track {
    fn try_from(value: SoundCloudTrack) -> Result<Self, Self::Error> {
        let metadata = TrackMetadata::soundcloud(
            value.track.title.clone().unwrap_or_default(),
            value.track.permalink_url.clone().unwrap_or_default(),
            value.track.artwork_url.clone(),
            value.track.duration.clone().unwrap_or(0) as u32,
            vec![
                value
                    .track
                    .user
                    .clone()
                    .and_then(|user| user.username)
                    .unwrap_or_default(),
            ],
        );
        let input = Input::Lazy(Box::new(value));
        let track = Track::new_with_data(input, Arc::new(metadata));
        Ok(track)
    }

    type Error = anyhow::Error;
}
