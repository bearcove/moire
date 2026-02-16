use std::io;
use std::path::Path;

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
