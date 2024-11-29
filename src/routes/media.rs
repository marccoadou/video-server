use crate::models::{CreateMediaItem, MediaItem};
use axum::body::Body;
use axum::http::{header, HeaderMap, Response, StatusCode};
use axum::Json;
use mime_guess::from_path;
use std::io::{BufRead, SeekFrom};
use tokio::fs;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub async fn get_media() -> Json<Vec<MediaItem>> {
    let media = vec![MediaItem {
        id: 1,
        title: "Another cool movie".to_string(),
        path: "/somewhere/movie.mp4".to_string(),
        created_at: chrono::Utc::now().naive_utc(),
    }];

    Json(media)
}

pub async fn post_media(Json(payload): Json<CreateMediaItem>) -> Json<MediaItem> {
    let media = MediaItem {
        id: 1,
        title: payload.title,
        path: payload.path,
        created_at: chrono::Utc::now().naive_utc(),
    };

    Json(media)
}

pub async fn get_file(
    Json(_payload): Json<MediaItem>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let media = MediaItem {
        id: 1,
        title: "Enregistrement.mp4".to_string(),
        path: "./medias/Enregistrement 2024-10-23 165821.mp4".to_string(),
        created_at: chrono::Utc::now().naive_utc(),
    };

    let file_contents = fs::read(&media.path)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Could not read file: {}", e)))?;

    let mime_type = from_path(&media.path).first_or_octet_stream();

    Ok(Response::builder()
        .header("Content-Type", mime_type.as_ref())
        .header("Content-Length", file_contents.len().to_string())
        .body(Body::from(file_contents))
        .unwrap())
}

pub async fn stream_media(headers: HeaderMap) -> Result<Response<Body>, StatusCode> {
    let media = MediaItem {
        id: 1,
        title: "Enregistrement.mp4".to_string(),
        path: "./medias/Enregistrement 2024-10-23 165821.mp4".to_string(),
        created_at: chrono::Utc::now().naive_utc(),
    };

    let mut file = File::open(&media.path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let metadata = file.metadata().await.map_err(|_| StatusCode::NOT_FOUND)?;
    let file_size = metadata.len();

    if let Some(range_header) = headers.get(header::RANGE) {
        let range_str = range_header.to_str().map_err(|_| StatusCode::BAD_REQUEST)?;

        if let Some((start, end)) = parse_range_header(range_str, file_size) {
            file.seek(SeekFrom::Start(start))
                .await
                .map_err(|_| StatusCode::BAD_REQUEST)?;

            let end = end.unwrap_or(file_size - 1);
            let chunk_size = (end - start + 1) as usize;

            let mut buffer = vec![0; chunk_size];

            file.read_exact(&mut buffer)
                .await
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            let body = Body::from(buffer);

            let response = Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, "video/mp4")
                .header(
                    header::CONTENT_RANGE,
                    format!("bytes {}-{}/{}", start, end, file_size),
                )
                .header(header::CONTENT_LENGTH, chunk_size.to_string())
                .body(body)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            return Ok(response);
        }
    }

    let mut buffer = Vec::new();

    file.read_to_end(&mut buffer)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let body = Body::from(buffer);
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "video/mp4")
        .header("Content-Length", file_size.to_string())
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response)
}

pub fn parse_range_header(range: &str, file_size: u64) -> Option<(u64, Option<u64>)> {
    if !range.starts_with("bytes=") {
        return None;
    }

    let range = &range["bytes=".len()..];
    let parts: Vec<&str> = range.split("-").collect();

    if let (Some(start), Some(end)) = (parts.get(0), parts.get(1)) {
        let start = start.parse::<u64>().ok()?;

        let end = if end.is_empty() {
            None
        } else {
            Some(end.parse::<u64>().ok()?)
        };

        if start < file_size {
            return Some((start, end));
        }
    }

    None
}
