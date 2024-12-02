use crate::models::{CreateMediaItem, MediaInfo, MediaItem};
use crate::services::{codec_info, get_content_range, partial_media_content};
use axum::body::Body;
use axum::http::{header, HeaderMap, Response, StatusCode};
use axum::Json;
use ffmpeg_next as ffmpeg;
use mime_guess::from_path;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

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

    let response = Response::builder()
        .header(header::CONTENT_TYPE, mime_type.as_ref())
        .header(header::CONTENT_LENGTH, file_contents.len().to_string())
        .body(Body::from(file_contents))
        .unwrap();

    Ok(response)
}

pub async fn get_media_info() -> Result<Response<Body>, (StatusCode, String)> {
    let media = MediaItem {
        id: 1,
        title: "Nightcrawler".to_string(),
        path: "./medias/Nightcrawler 2014 MULTi VFF 1080p BluRay AC3 x265-Winks.mkv".to_string(),
        created_at: chrono::Utc::now().naive_utc(),
    };

    if let Err(_e) = ffmpeg::init() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "ffmpeg init failed".to_string(),
        ));
    }

    let ictx = match ffmpeg::format::input(&media.path) {
        Ok(ictx) => ictx,
        Err(_e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                String::from("Internal server error"),
            ))
        }
    };

    let mut codecs = Vec::new();

    // Access each streams individually with their information
    for (_index, input_stream) in ictx.streams().enumerate() {
        let codec_info = codec_info(input_stream);
        codecs.push(codec_info);
    }

    let info = MediaInfo {
        name: media.title,
        codecs,
    };

    let response_body = serde_json::to_string(&info).unwrap();

    let response = Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(response_body))
        .unwrap();

    Ok(response)
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

    // If the request contains Content Range, serve a ranged stream
    if let Some(range_header) = headers.get(header::RANGE) {
        let (body, start, end, chunk_size) =
            partial_media_content(&mut file, range_header, file_size).await?;

        dbg!(start, end, file_size);

        let response = Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_TYPE, "video/mp4")
            .header(
                header::CONTENT_RANGE,
                get_content_range(start, end, file_size),
            )
            .header(header::CONTENT_LENGTH, chunk_size.to_string())
            .body(body)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        dbg!(&response);

        return Ok(response);
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
