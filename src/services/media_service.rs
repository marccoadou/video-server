// use axum::body::Body;
// use axum::http::{header, HeaderValue, Response, StatusCode};
// use std::io::SeekFrom;
// use tokio::fs::File;
// use tokio::io::{AsyncReadExt, AsyncSeekExt};
//
// pub async fn full_media_response(file: &mut File, file_size: u64) -> Result<Response<Body>, _> {
//     let mut buffer = Vec::new();
//
//     file.read_to_end(&mut buffer)
//         .await
//         .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//
//     let body = Body::from(buffer);
//     let response = Response::builder()
//         .status(StatusCode::OK)
//         .header("Content-Type", "video/mp4")
//         .header("Content-Length", file_size.to_string())
//         .body(body)
//         .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//
//     Ok(response)
// }
//
// pub async fn partial_media_response(
//     file: &mut File,
//     range_header: &HeaderValue,
//     file_size: u64,
// ) -> Result<Response<Body>, StatusCode> {
//     let range_str = range_header.to_str().map_err(|_| StatusCode::BAD_REQUEST);
//
//     if let Some((start, end)) = parse_range_header(range_str, file_size) {
//         file.seek(SeekFrom::Start(start))
//             .await
//             .map_err(|_| StatusCode::BAD_REQUEST)?;
//
//         let end = end.unwrap_or(file_size - 1);
//         let chunk_size = (end - start) as usize;
//
//         let mut buffer = vec![0; chunk_size];
//         file.read_exact(&mut buffer)
//             .await
//             .map_err(|_| StatusCode::BAD_REQUEST)?;
//         let body = Body::from(buffer);
//         let response = Response::builder()
//             .status(StatusCode::PARTIAL_CONTENT)
//             .header(header::CONTENT_TYPE, "video/mp4")
//             .header(
//                 header::CONTENT_RANGE,
//                 format!("bytes {}-{}/{}", start, end, file_size),
//             )
//             .header(header::CONTENT_LENGTH, (end - start).to_string())
//             .body(body)
//             .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//         Ok(response)
//     }
//
//     Err(StatusCode::BAD_REQUEST)
// }
//
// pub fn parse_range_header(
//     range: Result<&str, StatusCode>,
//     file_size: u64,
// ) -> Option<(u64, Option<u64>)> {
//     if !range.starts_with("bytes=") {
//         return None;
//     }
//
//     let range = &range["bytes=".len()..];
//     let parts: Vec<&str> = range.split("-").collect();
//
//     if let (Some(start), Some(end)) = (parts.get(0), parts.get(1)) {
//         let start = start.parse::<u64>().ok()?;
//
//         let end = if end.is_empty() {
//             None
//         } else {
//             Some(end.parse::<u64>().ok()?)
//         };
//
//         if start < file_size {
//             return Some((start, end));
//         }
//     }
//
//     None
// }
