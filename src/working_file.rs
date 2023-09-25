use std::path::{Path, PathBuf};

use tokio::fs::{File, OpenOptions};

pub struct WorkingFile {
    path: PathBuf,
}

impl WorkingFile {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub async fn open(&self) -> Result<File, std::io::Error> {
        OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&self.path)
            .await
    }
}

