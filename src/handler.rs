use std::io::{Cursor, SeekFrom};
use std::sync::Arc;
use std::time;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};

use crate::AppState;

pub enum ResponseError {
    Forbidden,
    Internal(String),
    InvalidJson,
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Self::Internal(s) => {
                eprintln!("internal error: {s}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
            }
            Self::InvalidJson => (StatusCode::BAD_REQUEST, "invalid json"),
        }
        .into_response()
    }
}

/// Get number of guest accounts
pub async fn count(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<String, ResponseError> {
    verify_auth(&state.auth, &headers)?;

    let mut file = state
        .dest_file
        .lock()
        .await
        .open()
        .await
        .map_err(|e| ResponseError::Internal(e.to_string()))?;
    file.seek(SeekFrom::Start(0))
        .await
        .map_err(|e| ResponseError::Internal(e.to_string()))?;
    let mut lines = BufReader::new(&mut file).lines();
    let mut count = 0;
    while (lines
        .next_line()
        .await
        .map_err(|e| ResponseError::Internal(e.to_string()))?)
    .is_some()
    {
        count += 1;
    }

    Ok(count.to_string())
}

/// Append guest accounts to the guest accounts file
pub async fn append(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: String,
) -> Result<(), ResponseError> {
    verify_auth(&state.auth, &headers)?;

    // Read all lines in body and append them to the file
    let mut file = state
        .dest_file
        .lock()
        .await
        .open()
        .await
        .map_err(|e| ResponseError::Internal(e.to_string()))?;
    file.seek(SeekFrom::End(0))
        .await
        .map_err(|e| ResponseError::Internal(e.to_string()))?;
    let cursor = Cursor::new(body);
    let deserializer = serde_json::Deserializer::from_reader(cursor);
    let values: Vec<_> = deserializer.into_iter::<serde_json::Value>().collect();
    for value in values {
        let value = value.map_err(|_| ResponseError::InvalidJson)?;
        let line =
            serde_json::to_string(&value).map_err(|e| ResponseError::Internal(e.to_string()))?;
        let line = format!("{}\n", line);
        file.write_all(line.as_bytes())
            .await
            .map_err(|e| ResponseError::Internal(e.to_string()))?;
    }
    file.flush()
        .await
        .map_err(|e| ResponseError::Internal(e.to_string()))?;

    Ok(())
}

/// Remove all guest accounts older than a specified max age
pub async fn prune(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<(), ResponseError> {
    verify_auth(&state.auth, &headers)?;

    #[derive(Deserialize)]
    struct GuestAccount {
        user: User,
    }

    #[derive(Deserialize)]
    struct User {
        id_str: String,
    }

    // Get the current time
    let current_time = time::UNIX_EPOCH
        .elapsed()
        .map_err(|e| ResponseError::Internal(e.to_string()))?
        .as_secs() as i64;
    const MAX_AGE_DAYS: i64 = 25;
    const MAX_AGE_SECS: i64 = MAX_AGE_DAYS * 24 * 60 * 60;

    // Read the guest accounts file
    let mut file = state
        .dest_file
        .lock()
        .await
        .open()
        .await
        .map_err(|e| ResponseError::Internal(e.to_string()))?;
    file.seek(SeekFrom::Start(0))
        .await
        .map_err(|e| ResponseError::Internal(e.to_string()))?;
    let mut lines = BufReader::new(&mut file).lines();

    // Lines to keep
    let mut preserved_lines = Vec::new();

    // For each line, check its ID, convert it to a timestamp, and check its age.
    // Keep it if it's less than the specified age
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| ResponseError::Internal(e.to_string()))?
    {
        let account: GuestAccount =
            serde_json::from_str(&line).map_err(|e| ResponseError::Internal(e.to_string()))?;
        let id = account
            .user
            .id_str
            .parse::<u64>()
            .map_err(|e| ResponseError::Internal(e.to_string()))?;
        let ts = id_to_ts(id);
        if current_time - ts < MAX_AGE_SECS {
            preserved_lines.push(line);
        }
    }

    // Truncate the file and add all lines that should be preserved
    file.set_len(0)
        .await
        .map_err(|e| ResponseError::Internal(e.to_string()))?;
    file.seek(SeekFrom::Start(0))
        .await
        .map_err(|e| ResponseError::Internal(e.to_string()))?;
    for line in preserved_lines {
        let line = format!("{}\n", line);
        file.write_all(line.as_bytes())
            .await
            .map_err(|e| ResponseError::Internal(e.to_string()))?;
    }

    file.flush()
        .await
        .map_err(|e| ResponseError::Internal(e.to_string()))?;

    Ok(())
}

fn verify_auth(auth: &String, headers: &HeaderMap) -> Result<(), ResponseError> {
    (headers.get("x-auth").ok_or(ResponseError::Forbidden)? == auth)
        .then_some(())
        .ok_or(ResponseError::Forbidden)
}

fn id_to_ts(id: u64) -> i64 {
    (((id >> 22) + 1288834974657) / 1000) as i64
}
