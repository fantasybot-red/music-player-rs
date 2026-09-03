use crate::players::{Ffmpeg, PlaylistQueue, QueueResult, SpotifyClient, TrackMetadata};
use anyhow::Result as AResult;
use async_trait::async_trait;
use librespot::{
    audio::{AudioDecrypt, AudioFile},
    core::{SpotifyId, SpotifyUri},
    metadata::audio::{AudioFileFormat, AudioFiles, AudioItem},
    metadata::{Album, Episode, Metadata, Playlist, Show, Track},
};
use songbird::{
    input::{AudioStream, AudioStreamError, AuxMetadata, Compose, Input},
    tracks::Track as SongbirdTrack,
};
use std::{
    io::{Read, Seek, SeekFrom},
    sync::Arc,
};
use symphonia::core::io::MediaSource;

#[derive(Clone, Debug)]
pub enum SpotifyEntity {
    Track(SpotifyUri),
    Album(SpotifyUri),
    Playlist(SpotifyUri),
    Episode(SpotifyUri),
    Show(SpotifyUri),
}

#[derive(Clone)]
pub struct SpotifyTrack {
    pub client: SpotifyClient,
    pub uri: SpotifyUri,
    pub metadata: Option<TrackMetadata>,
}

impl SpotifyTrack {
    pub fn check_url(url: &str) -> bool {
        let Ok(parsed) = url::Url::parse(url) else {
            return false;
        };

        parsed
            .host_str()
            .map(|host| host == "spotify.com" || host.ends_with(".spotify.com"))
            .unwrap_or(false)
    }

    pub async fn new(client: &SpotifyClient, uri: SpotifyUri) -> AResult<Self> {
        Ok(Self {
            client: client.clone(),
            uri,
            metadata: None,
        })
    }

    pub async fn from_url(client: &SpotifyClient, url: &str) -> AResult<QueueResult> {
        let entity = Self::parse_spotify_url(url)?;
        let session = client.get_session().await?;

        match entity {
            SpotifyEntity::Track(uri) => {
                let track = Track::get(&session, &uri).await?;
                let metadata = Self::track_to_metadata(&track, &uri)?;
                let spotify_track = Self {
                    client: client.clone(),
                    uri,
                    metadata: Some(metadata.clone()),
                };
                let track_handle: SongbirdTrack = spotify_track.try_into()?;
                Ok(QueueResult::Track(track_handle))
            }
            SpotifyEntity::Album(uri) => {
                let album = Album::get(&session, &uri).await?;
                let tracks = Self::expand_album(client, &session, &album).await?;
                let link = match &uri {
                    SpotifyUri::Album { id, .. } => {
                        format!("https://open.spotify.com/album/{}", id.to_base62())
                    }
                    _ => String::new(),
                };
                let playlist = PlaylistQueue {
                    name: album.name.clone(),
                    link,
                    artwork: album
                        .covers
                        .first()
                        .map(|img| format!("https://i.scdn.co/image/{}", img.id.to_base16())),
                    tracks,
                };
                Ok(QueueResult::Playlist(playlist))
            }
            SpotifyEntity::Playlist(uri) => {
                let playlist = Playlist::get(&session, &uri).await?;
                let tracks = Self::expand_playlist(client, &session, &playlist).await?;
                let link = match &uri {
                    SpotifyUri::Playlist { id, .. } => {
                        format!("https://open.spotify.com/playlist/{}", id.to_base62())
                    }
                    _ => String::new(),
                };
                let playlist_queue = PlaylistQueue {
                    name: playlist.name().to_string(),
                    link,
                    artwork: playlist
                        .attributes
                        .picture_sizes
                        .first()
                        .map(|ps| ps.url.clone()),
                    tracks,
                };
                Ok(QueueResult::Playlist(playlist_queue))
            }
            SpotifyEntity::Episode(uri) => {
                let episode = Episode::get(&session, &uri).await?;
                let metadata = Self::episode_to_metadata(&episode, &uri)?;
                let spotify_track = Self {
                    client: client.clone(),
                    uri,
                    metadata: Some(metadata.clone()),
                };
                let track_handle: SongbirdTrack = spotify_track.try_into()?;
                Ok(QueueResult::Track(track_handle))
            }
            SpotifyEntity::Show(uri) => {
                let show = Show::get(&session, &uri).await?;
                let tracks = Self::expand_show(client, &session, &show).await?;
                let link = match &uri {
                    SpotifyUri::Show { id, .. } => {
                        format!("https://open.spotify.com/show/{}", id.to_base62())
                    }
                    _ => String::new(),
                };
                let playlist = PlaylistQueue {
                    name: show.name.clone(),
                    link,
                    artwork: show
                        .covers
                        .first()
                        .map(|img| format!("https://i.scdn.co/image/{}", img.id.to_base16())),
                    tracks,
                };
                Ok(QueueResult::Playlist(playlist))
            }
        }
    }

    pub fn parse_spotify_url(url: &str) -> AResult<SpotifyEntity> {
        let parsed = url::Url::parse(url)?;

        let host = parsed
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid Spotify URL: missing host"))?;
        if !host.ends_with(".spotify.com") && host != "spotify.com" {
            return Err(anyhow::anyhow!("Invalid Spotify URL"));
        }

        let segments = parsed
            .path_segments()
            .ok_or_else(|| anyhow::anyhow!("Invalid Spotify URL: no path segments"))?
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        if segments.len() != 2 {
            return Err(anyhow::anyhow!(
                "Invalid Spotify URL: expected an entity type and ID"
            ));
        }

        let entity_type = segments[0];
        let id = segments[1];
        let uri = SpotifyUri::from_uri(&format!("spotify:{}:{}", entity_type, id))?;

        match entity_type {
            "track" => Ok(SpotifyEntity::Track(uri)),
            "album" => Ok(SpotifyEntity::Album(uri)),
            "playlist" => Ok(SpotifyEntity::Playlist(uri)),
            "episode" => Ok(SpotifyEntity::Episode(uri)),
            "show" => Ok(SpotifyEntity::Show(uri)),
            _ => Err(anyhow::anyhow!(
                "Unsupported Spotify entity type: {}",
                entity_type
            )),
        }
    }

    fn track_to_metadata(track: &Track, uri: &SpotifyUri) -> AResult<TrackMetadata> {
        let url = match uri {
            SpotifyUri::Track { id, .. } => {
                format!("https://open.spotify.com/track/{}", id.to_base62())
            }
            _ => String::new(),
        };

        Ok(TrackMetadata::spotify(
            track.name.clone(),
            url,
            track
                .album
                .covers
                .first()
                .map(|img| format!("https://i.scdn.co/image/{}", img.id.to_base16())),
            track.duration as u32,
            track.artists.iter().map(|a| a.name.clone()).collect(),
        ))
    }

    fn episode_to_metadata(episode: &Episode, uri: &SpotifyUri) -> AResult<TrackMetadata> {
        let url = match uri {
            SpotifyUri::Episode { id, .. } => {
                format!("https://open.spotify.com/episode/{}", id.to_base62())
            }
            _ => String::new(),
        };

        Ok(TrackMetadata::spotify(
            episode.name.clone(),
            url,
            episode
                .covers
                .first()
                .map(|img| format!("https://i.scdn.co/image/{}", img.id.to_base16())),
            episode.duration as u32,
            vec![episode.show_name.clone()],
        ))
    }

    async fn expand_album(
        client: &SpotifyClient,
        session: &librespot::core::Session,
        album: &Album,
    ) -> AResult<Vec<SongbirdTrack>> {
        let track_uris = album.tracks().cloned().collect::<Vec<_>>();
        let mut tracks = Vec::with_capacity(track_uris.len());
        for uri in track_uris {
            let track = Track::get(session, &uri).await?;
            tracks.push(Self::songbird_track(
                client,
                uri.clone(),
                Self::track_to_metadata(&track, &uri)?,
            )?);
        }
        Ok(tracks)
    }

    async fn expand_playlist(
        client: &SpotifyClient,
        session: &librespot::core::Session,
        playlist: &Playlist,
    ) -> AResult<Vec<SongbirdTrack>> {
        let track_uris = playlist.tracks().cloned().collect::<Vec<_>>();
        let mut tracks = Vec::with_capacity(track_uris.len());
        for uri in track_uris {
            let track = Track::get(session, &uri).await?;
            tracks.push(Self::songbird_track(
                client,
                uri.clone(),
                Self::track_to_metadata(&track, &uri)?,
            )?);
        }
        Ok(tracks)
    }

    async fn expand_show(
        client: &SpotifyClient,
        session: &librespot::core::Session,
        show: &Show,
    ) -> AResult<Vec<SongbirdTrack>> {
        let mut tracks = Vec::with_capacity(show.episodes.len());
        for uri in show.episodes.iter() {
            let episode = Episode::get(session, uri).await?;
            tracks.push(Self::songbird_track(
                client,
                uri.clone(),
                Self::episode_to_metadata(&episode, uri)?,
            )?);
        }
        Ok(tracks)
    }

    fn songbird_track(
        client: &SpotifyClient,
        uri: SpotifyUri,
        metadata: TrackMetadata,
    ) -> AResult<SongbirdTrack> {
        SpotifyTrack {
            client: client.clone(),
            uri,
            metadata: Some(metadata),
        }
        .try_into()
    }

    pub fn url(&self) -> String {
        match &self.uri {
            SpotifyUri::Track { id, .. } => {
                format!("https://open.spotify.com/track/{}", id.to_base62())
            }
            _ => String::new(),
        }
    }
}

pub struct SpotifyReader {
    decrypted: AudioDecrypt<AudioFile>,
    length: Option<usize>,
    offset: usize,
}

impl Read for SpotifyReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.decrypted.read(buf)
    }
}

impl Seek for SpotifyReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let pos = match pos {
            SeekFrom::Start(offset) => SeekFrom::Start(offset + self.offset as u64),
            SeekFrom::End(offset) => {
                if (self.length.unwrap_or(0) as i64 - offset) < self.offset as i64 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "newpos would be < self.offset",
                    ));
                }
                pos
            }
            _ => pos,
        };

        let newpos = self.decrypted.seek(pos)?;
        Ok(newpos - self.offset as u64)
    }
}

impl MediaSource for SpotifyReader {
    fn is_seekable(&self) -> bool {
        self.length.is_some()
    }

    fn byte_len(&self) -> Option<u64> {
        self.length.map(|len| len as u64)
    }
}

const SPOTIFY_OGG_HEADER_END: u64 = 0xa7;

fn stream_data_rate(format: AudioFileFormat) -> usize {
    let kb_per_sec = match format {
        AudioFileFormat::OGG_VORBIS_96 => 12,
        AudioFileFormat::OGG_VORBIS_160 => 20,
        AudioFileFormat::OGG_VORBIS_320 => 40,
        _ => 40,
    };
    kb_per_sec * 1024
}

#[async_trait]
impl Compose for SpotifyTrack {
    fn create(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        Err(AudioStreamError::Unsupported)
    }

    async fn create_async(
        &mut self,
    ) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        let session = self.client.get_session().await.map_err(|e| {
            AudioStreamError::Fail(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        let audio_item = AudioItem::get_file(&session, self.uri.clone())
            .await
            .map_err(|e| {
                AudioStreamError::Fail(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))
            })?;

        let (file_format, file_id) = audio_item
            .files
            .iter()
            .max_by_key(|(format, _)| stream_data_rate(**format))
            .ok_or_else(|| {
                AudioStreamError::Fail(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "No audio files found",
                )))
            })?;

        let bytes_per_second = stream_data_rate(*file_format);
        let encrypted_file = AudioFile::open(&session, *file_id, bytes_per_second)
            .await
            .map_err(|e| {
                AudioStreamError::Fail(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))
            })?;

        let spotify_id = SpotifyId::from_base62(&self.uri.to_id()).map_err(|e| {
            AudioStreamError::Fail(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )))
        })?;

        let file_key = session
            .audio_key()
            .request(spotify_id, *file_id)
            .await
            .map_err(|e| {
                AudioStreamError::Fail(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))
            })?;
        let length = encrypted_file
            .get_stream_loader_controller()
            .map(|loader| Some(loader.len()))
            .unwrap_or(None);

        let mut decrypted_file = AudioDecrypt::new(Some(file_key), encrypted_file);
        let offset = if AudioFiles::is_ogg_vorbis(*file_format) {
            SPOTIFY_OGG_HEADER_END
        } else {
            0
        };
        if offset != 0 {
            let mut header = vec![0_u8; offset as usize];
            decrypted_file
                .read_exact(&mut header)
                .map_err(|e| AudioStreamError::Fail(Box::new(e)))?;
        }

        let reader = SpotifyReader {
            decrypted: decrypted_file,
            length: length,
            offset: offset as usize,
        };

        let mut ffmpeg = Ffmpeg::new_pipe(reader)
            .map_err(|e| AudioStreamError::Fail(Box::new(std::io::Error::other(e.to_string()))))?;
        ffmpeg.create_async().await
    }

    fn should_create_async(&self) -> bool {
        true
    }

    async fn aux_metadata(&mut self) -> Result<AuxMetadata, AudioStreamError> {
        if let Some(metadata) = &self.metadata {
            return Ok(AuxMetadata {
                track: Some(metadata.title.clone()),
                artist: Some(metadata.artists.join(", ")),
                duration: Some(std::time::Duration::from_millis(u64::from(
                    metadata.duration,
                ))),
                source_url: Some(metadata.url.clone()),
                thumbnail: metadata.artwork.clone(),
                ..Default::default()
            });
        }
        Ok(AuxMetadata::default())
    }
}

impl TryFrom<SpotifyTrack> for SongbirdTrack {
    fn try_from(value: SpotifyTrack) -> Result<Self, Self::Error> {
        let metadata = value
            .metadata
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Spotify track metadata is required before queueing"))?;
        let input = Input::Lazy(Box::new(value));
        let track = SongbirdTrack::new_with_data(input, Arc::new(metadata));
        Ok(track)
    }

    type Error = anyhow::Error;
}
