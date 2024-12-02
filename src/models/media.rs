use chrono::NaiveDateTime;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct MediaItem {
    pub id: u32,
    pub title: String,
    pub path: String,
    pub created_at: NaiveDateTime,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CreateMediaItem {
    pub title: String,
    pub path: String,
}

#[derive(serde::Serialize)]
pub struct MediaInfo {
    pub name: String,
    pub codecs: Vec<CodecInfo>,
}

#[derive(serde::Serialize)]
pub struct CodecInfo {
    pub codec_id: String,
    pub codec_medium: String,
    pub length: i64,
    // pub format: i    32,
    // pub bit_rate: i64,
}
