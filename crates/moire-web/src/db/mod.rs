use std::path::{Path, PathBuf};

use rusqlite::Connection;

#[derive(Debug, Clone)]
pub struct Db {
    path: PathBuf,
}

impl Db {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn open(&self) -> Result<Connection, String> {
        Connection::open(&self.path).map_err(|error| format!("open sqlite: {error}"))
    }
}
