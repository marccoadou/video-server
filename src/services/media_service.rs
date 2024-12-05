use crate::models::CodecInfo;
use axum::body::Body;
use axum::http::{HeaderValue, StatusCode};
use ffmpeg_next::codec::Parameters;
use ffmpeg_next::Stream;
use std::io::SeekFrom;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub async fn full_media_content(file: &mut File) -> Result<Body, StatusCode> {
    let mut buffer = Vec::new();

    file.read_to_end(&mut buffer)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    Ok(Body::from(buffer))
}

pub fn get_content_range(start: u64, end: u64, total: u64) -> String {
    format!("bytes {}-{}/{:?}", start, end, total)
}

pub async fn partial_media_content(
    file: &mut File,
    range_header: &HeaderValue,
    file_size: u64,
) -> Result<(Body, u64, u64, usize), StatusCode> {
    let range_str = range_header.to_str().map_err(|_| StatusCode::BAD_REQUEST)?;

    let (start, end) = parse_range_header(range_str, file_size).ok_or(StatusCode::BAD_REQUEST)?;

    file.seek(SeekFrom::Start(start))
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let end = end.unwrap_or(file_size - 1);
    let chunk_size = ((end - start) + 1) as usize;

    let mut buffer = vec![0; chunk_size];
    file.read_exact(&mut buffer)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    Ok((Body::from(buffer), start, end, chunk_size))
}

pub fn get_medium_type(parameters: Parameters) -> String {
    match parameters.medium() {
        ffmpeg_next::media::Type::Video => "Video".to_string(),
        ffmpeg_next::media::Type::Audio => "Audio".to_string(),
        ffmpeg_next::media::Type::Subtitle => "Subtitle".to_string(),
        ffmpeg_next::media::Type::Data => "Data".to_string(),
        ffmpeg_next::media::Type::Attachment => "Attachment".to_string(),
        ffmpeg_next::media::Type::Unknown => "Unknown".to_string(),
    }
}

pub fn codec_info(stream: Stream) -> CodecInfo {
    let parameters = stream.parameters();

    CodecInfo {
        codec_id: parameters.id().name().to_string(),
        codec_medium: get_medium_type(parameters),
        length: stream.duration(),
    }
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
