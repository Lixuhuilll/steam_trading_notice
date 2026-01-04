use tracing::info;

mod init;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    let _guards = init::log_init()?;
    info!("日志系统初始化完成");

    Ok(())
}
