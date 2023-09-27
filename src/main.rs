use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::routing::{post, get};
use axum::Router;
use handler::{append, prune, count};
use tokio::sync::Mutex;
use working_file::WorkingFile;

mod handler;
mod working_file;

pub struct AppState {
    auth: String,
    dest_file: Mutex<WorkingFile>,
}

#[tokio::main]
async fn main() {
    // Read config from env vars
    let port = std::env::var("PORT").unwrap().parse().unwrap();
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
    let dest_file_path = std::env::var_os("DEST_FILE").unwrap();
    let state = Arc::new(AppState {
        auth: std::env::var("AUTH").unwrap(),
        dest_file: Mutex::new(WorkingFile::new(dest_file_path)),
    });

    // Start web server
    let app = Router::new()
        .route("/count", get(count))
        .route("/append", post(append))
        .route("/prune", post(prune))
        .with_state(state);
    axum::Server::bind(&socket)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
