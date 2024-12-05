extern crate core;

mod models;
mod routes;
mod services;

use crate::routes::media::{
    get_file, get_media, get_media_info, post_media, stream_media, transcode_media,
};
use axum::http::Method;
use axum::routing::{get, post};
use axum::Router;
use dotenv::dotenv;
use futures::executor::block_on;
use sea_orm::{Database, DbErr};
use std::env;
use tower_http::cors::{Any, CorsLayer};

pub fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::HEAD])
        .allow_headers(Any)
        .allow_origin(Any)
}

async fn run() -> Result<(), DbErr> {
    let database_url =
        env::var("DATABASE_URL").expect("Environment variable DATABASE_URL is required");

    let _db = Database::connect(database_url);

    Ok(())
}

pub fn create_routes() -> Router {
    Router::new()
        .route("/medias", get(get_media))
        .route("/medias", post(post_media))
        .route("/medias/get_file", post(get_file))
        .route("/medias/stream", get(stream_media))
        .route("/medias/info", get(get_media_info))
        .route("/medias/transcode", get(transcode_media))
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    if let Err(err) = block_on(run()) {
        panic!("{:?}", err);
    };

    let app = Router::new().merge(create_routes()).layer(cors());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}
