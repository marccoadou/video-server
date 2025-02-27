use crate::models::{CreateMediaItem, MediaInfo, MediaItem};
use crate::services::{
    codec_info, get_content_range, parse_opts, partial_media_content, SubtitleTranscoder,
    Transcoder, VideoTranscoder, DEFAULT_X264_OPTS,
};
use axum::body::Body;
use axum::http::{header, HeaderMap, Response, StatusCode};
use axum::Json;
use ffmpeg_next as ffmpeg;
use ffmpeg_next::codec::Parameters;
use ffmpeg_next::format::output;
use ffmpeg_next::{codec, encoder, format, log, media, packet, Rational};
use mime_guess::from_path;
use std::collections::HashMap;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

pub async fn transcode_subtitles(_headers: HeaderMap) -> Result<Response<Body>, StatusCode> {
    ffmpeg_next::init().unwrap();
    log::set_level(log::Level::Info);

    let input_file_path = "./medias/kingsman/Kingsman The Secret Service (2014) WEBDL-1080p.mkv";
    let mut input_context = format::input(&input_file_path).unwrap();
    format::context::input::dump(&input_context, 0, Some(&input_file_path));

    let (subtitle_stream_index, subtitle_params) = input_context
        .streams()
        .best(ffmpeg::media::Type::Subtitle)
        .ok_or(ffmpeg::Error::StreamNotFound)
        .map(|stream| (stream.index(), stream.parameters()))
        .unwrap();

    let decoder_ctx = codec::context::Context::from_parameters(subtitle_params).unwrap();
    let decoder = decoder_ctx.decoder().subtitle().unwrap();

    let mut output_ctx = output("./medias/kingsman/subtitle.srt").unwrap();
    let mut output_stream = output_ctx
        .add_stream(ffmpeg::encoder::find(codec::Id::SUBRIP))
        .unwrap();

    let mut output_params = Parameters::new();
    // output_params;

    Response::builder()
        .status(StatusCode::OK)
        .body(Body::empty())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn transcode_media(_headers: HeaderMap) -> Result<Response<Body>, StatusCode> {
    ffmpeg::init().unwrap();
    log::set_level(log::Level::Info);
    /* let input_file_name = "Kingsman The Secret Service (2014) WEBDL-1080p.mkv";
    let output_folder = "./medias/kingsman"; */

    let input_file_path = "./medias/Foundation.S02E03.MULTi.1080p.WEBRip.x264.AC3-MULTiViSiON.mkv";
    // let ouput_file_path = output_folder.to_string() + "/" + input_file_name;

    let mut ictx = format::input(&input_file_path).unwrap();
    let mut octx = format::output(&input_file_path).unwrap();

    // this dumps the info the err terminal
    // format::context::input::dump(&ictx, 0, Some(&input_file_path));

    let best_video_stream_index = ictx
        .streams()
        .best(media::Type::Video)
        .map(|stream| stream.index());

    let mut stream_mapping: Vec<isize> = vec![0; ictx.nb_streams() as _];
    /*     let mut subtitle_streams_mapping = vec![0; ictx.nb_streams() as _]; */
    let mut ist_time_bases = vec![Rational(0, 0); ictx.nb_streams() as _];
    let mut ost_time_bases = vec![Rational(0, 0); ictx.nb_streams() as _];
    let mut transcoders: HashMap<usize, Box<dyn Transcoder>> = HashMap::new();
    let mut ost_index = 0;

    // For each stream
    for (ist_index, ist) in ictx.streams().enumerate() {
        // Récupérer le medium du Codec : Video / Audio / Sous-Titres
        let ist_medium = ist.parameters().medium();

        dbg!(&ist_medium);

        // Si le medium ne fait pas partie de ces groupes, on ne l'utilise pas
        if ist_medium != media::Type::Video
            && ist_medium != media::Type::Audio
            && ist_medium != media::Type::Subtitle
        {
            stream_mapping[ist_index] = -1;
            continue;
        }

        // Mapper l'index
        stream_mapping[ist_index] = ost_index;

        // Mapper la time_base
        ist_time_bases[ist_index] = ist.time_base();
        let x264_opts = parse_opts(DEFAULT_X264_OPTS.to_string()).unwrap();

        match ist_medium {
            media::Type::Video => {
                transcoders.insert(
                    ist_index,
                    Box::new(
                        VideoTranscoder::new(
                            &ist,
                            &mut octx,
                            ost_index as _,
                            x264_opts,
                            Some(ist_index) == best_video_stream_index,
                        )
                        .unwrap(),
                    ),
                );
            }
            // media::Type::Audio => {
            //
            // }
            media::Type::Subtitle => {
                let mut sub_octx =
                    format::output(&format!("./medias/subtitle-{}.srt", ist_index)).unwrap();

                transcoders.insert(
                    ist_index,
                    Box::new(
                        SubtitleTranscoder::new(
                            &ist,
                            &mut sub_octx,
                            ost_index as _,
                            x264_opts,
                            Some(ist_index) == best_video_stream_index,
                        )
                        .unwrap(),
                    ),
                );
            }
            _ => {
                let mut ost = octx.add_stream(encoder::find(codec::Id::None)).unwrap();
                ost.set_parameters(ist.parameters());
                // We need to set codec_tag to 0 lest we run into incompatible codec tag
                // issues when muxing into a different container format. Unfortunately
                // there's no high level API to do this (yet).
                unsafe {
                    (*ost.parameters().as_mut_ptr()).codec_tag = 0;
                }
            }
        }
        ost_index += 1;
    }

    octx.set_metadata(ictx.metadata().to_owned());
    format::context::output::dump(&octx, 0, Some("./medias/output.mp4"));
    octx.write_header().unwrap();

    for (ost_index, _) in octx.streams().enumerate() {
        ost_time_bases[ost_index] = octx.stream(ost_index as _).unwrap().time_base();
    }

    for (stream, mut packet) in ictx.packets() {
        let ist_index = stream.index();
        let ost_index = stream_mapping[ist_index];

        if ost_index < 0 {
            continue;
        }

        let ost_time_base = ost_time_bases[ost_index as usize];
        match transcoders.get_mut(&ist_index) {
            // transcode what needs to be transcoded
            Some(transcoder) => {
                transcoder.send_packet_to_decoder(&packet);
                transcoder.receive_and_process_decoded_frames(&mut octx, ost_time_base);
            }
            // Just copy the streams
            None => {
                packet.rescale_ts(ist_time_bases[ist_index], ost_time_base);
                packet.set_position(-1);
                packet.set_stream(ost_index as _);
                packet.write_interleaved(&mut octx).unwrap()
            }
        }
    }

    // Flush encoders and decoders.
    for (ost_index, transcoder) in transcoders.iter_mut() {
        let ost_time_base = ost_time_bases[*ost_index];
        transcoder.send_eof_to_decoder();
        transcoder.receive_and_process_decoded_frames(&mut octx, ost_time_base);
        transcoder.send_eof_to_decoder();
        transcoder.receive_and_process_encoded_packets(&mut octx, ost_time_base);
    }

    octx.write_trailer().unwrap();

    let response = Response::builder()
        .status(StatusCode::OK)
        // .header("Content-Type", "video/mp4")
        // .header("Content-Length", file_size.to_string())
        .body(Body::empty())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response)
}

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
        path: "./medias/Kingsman The Secret Service (2014) WEBDL-1080p.mkv".to_string(),
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

    format::context::input::dump(&ictx, 0, Some(&media.path));

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
