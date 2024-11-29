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
