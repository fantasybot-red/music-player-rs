use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SigiState {
    #[serde(rename = "LiveRoom")]
    pub live_room: Option<LiveRoom>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRoom {
    pub need_login: bool,
    pub show_live_gate: bool,
    pub is_age_gate_room: bool,
    pub live_room_status: i64,
    pub live_room_user_info: LiveRoomUserInfo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRoomUserInfo {
    pub user: User,
    pub stats: UserStats,
    pub live_room: LiveRoomDetails,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub avatar_larger: String,
    pub avatar_medium: String,
    pub avatar_thumb: String,
    pub id: String,
    pub nickname: String,
    pub sec_uid: String,
    pub secret: bool,
    pub unique_id: String,
    pub verified: bool,
    pub room_id: String,
    pub signature: String,
    pub status: i64,
    pub follow_status: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserStats {
    pub following_count: u64,
    pub follower_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRoomDetails {
    pub cover_url: String,
    pub square_cover_img: String,
    pub title: String,
    pub start_time: u64,
    pub status: i64,
    #[serde(default)]
    pub stream_data: Option<LiveStreamData>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveStreamData {
    pub pull_data: PullData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PullData {
    pub options: PullOptions,
    #[serde(
        default,
        deserialize_with = "deserialize_embedded_json",
        serialize_with = "serialize_embedded_json"
    )]
    pub stream_data: Option<ParsedStreamData>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PullOptions {
    pub default_quality: Quality,
    pub qualities: Vec<Quality>,
    pub show_quality_button: bool,
    pub support_low_latency: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quality {
    pub icon_type: i64,
    pub level: i64,
    pub name: String,
    pub resolution: String,
    pub sdk_key: String,
    pub v_codec: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedStreamData {
    pub data: StreamVariants,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamVariants {
    pub ao: StreamQuality,
    pub hd: StreamQuality,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamQuality {
    pub main: StreamUrls,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamUrls {
    pub flv: String,
    pub hls: String,
    pub cmaf: String,
    pub dash: String,
    pub lls: String,
    pub tsl: String,
    pub tile: String,
    pub rtc: String,
    pub sdk_params: String,
}

fn deserialize_embedded_json<'de, D>(deserializer: D) -> Result<Option<ParsedStreamData>, D::Error>
where
    D: Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    if encoded.is_empty() {
        return Ok(None);
    }

    serde_json::from_str(&encoded)
        .map(Some)
        .map_err(serde::de::Error::custom)
}

fn serialize_embedded_json<S>(
    value: &Option<ParsedStreamData>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serde_json::to_string(value)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer),
        None => "".serialize(serializer),
    }
}
