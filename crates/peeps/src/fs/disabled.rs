use std::io;
use std::path::{Path, PathBuf};

#[inline]
pub async fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    tokio::fs::create_dir_all(path).await
}

#[inline]
pub async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    tokio::fs::write(path, contents).await
}

#[inline]
pub async fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    tokio::fs::read_to_string(path).await
}

#[inline]
pub async fn metadata(path: impl AsRef<Path>) -> io::Result<std::fs::Metadata> {
    tokio::fs::metadata(path).await
}

#[inline]
pub async fn set_permissions(
    path: impl AsRef<Path>,
    perm: std::fs::Permissions,
) -> io::Result<()> {
    tokio::fs::set_permissions(path, perm).await
}

#[inline]
pub async fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    tokio::fs::rename(from, to).await
}

#[inline]
pub async fn try_exists(path: impl AsRef<Path>) -> io::Result<bool> {
    tokio::fs::try_exists(path).await
}

#[inline]
pub async fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    tokio::fs::read(path).await
}

#[inline]
pub async fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    tokio::fs::remove_file(path).await
}

#[inline]
pub async fn remove_dir(path: impl AsRef<Path>) -> io::Result<()> {
    tokio::fs::remove_dir(path).await
}

#[inline]
pub async fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    tokio::fs::remove_dir_all(path).await
}

#[inline]
pub async fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    tokio::fs::canonicalize(path).await
}

#[cfg(unix)]
#[inline]
pub async fn symlink(original: impl AsRef<Path>, link: impl AsRef<Path>) -> io::Result<()> {
    tokio::fs::symlink(original, link).await
}

#[cfg(windows)]
#[inline]
pub async fn symlink_file(original: impl AsRef<Path>, link: impl AsRef<Path>) -> io::Result<()> {
    tokio::fs::symlink_file(original, link).await
}
