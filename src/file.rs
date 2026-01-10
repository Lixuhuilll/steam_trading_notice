use std::io;
use tokio::fs::File;

pub async fn create_temp_file() -> io::Result<File> {
    let std = tokio::task::spawn_blocking(move || tempfile::tempfile()).await??;
    Ok(File::from_std(std))
}
