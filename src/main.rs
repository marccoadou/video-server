mod media;

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct MediaItem {
    id: u32,
    title: String,
    path: String,
}

async fn get_media() -> Json<Vec<MediaItem>> {
    let media = vec![MediaItem {
        id: 1,
        title: "Another cool movie".to_string(),
        path: "/somewhere/movie.mp4".to_string(),
    }];

    Json(media)
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(get_media));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
