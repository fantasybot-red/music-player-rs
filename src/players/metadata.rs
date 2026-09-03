use songbird::tracks::Track;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TrackPlatform {
    Spotify,
    SoundCloud,
    TikTokLive,
}

#[derive(Debug, Clone)]
pub struct TrackMetadata {
    pub title: String,
    pub url: String,
    pub artwork: Option<String>,
    pub platform: TrackPlatform,
    pub duration: u32,
    pub artists: Vec<String>,
}

impl TrackMetadata {
    pub fn spotify(
        title: String,
        url: String,
        artwork: Option<String>,
        duration: u32,
        artists: Vec<String>,
    ) -> Self {
        Self {
            title,
            url,
            artwork,
            platform: TrackPlatform::Spotify,
            duration,
            artists,
        }
    }

    pub fn soundcloud(
        title: String,
        url: String,
        artwork: Option<String>,
        duration: u32,
        artists: Vec<String>,
    ) -> Self {
        Self {
            title,
            url,
            artwork,
            platform: TrackPlatform::SoundCloud,
            duration,
            artists,
        }
    }

    pub fn tiktok_live(
        title: String,
        url: String,
        artwork: Option<String>,
        artists: Vec<String>,
    ) -> Self {
        let title_real = if title.is_empty() {
            format!("Live stream của {}", artists.join(", "))
        } else {
            title.clone()
        };
        Self {
            title: title_real,
            url,
            artwork,
            platform: TrackPlatform::TikTokLive,
            duration: 0,
            artists,
        }
    }
}

pub enum QueueResult {
    Track(Track),
    Playlist(PlaylistQueue),
}

pub struct PlaylistQueue {
    pub name: String,
    pub link: String,
    pub tracks: Vec<Track>,
    pub artwork: Option<String>,
}
