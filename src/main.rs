use tracing::{error, info};

mod init;

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    init::smtp_init("test", 0, "test", "00", 5).await?;

    // 优雅退出
    tokio::signal::ctrl_c().await?;

    Ok(())
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    let _guards = init::log_init()?;
    info!("日志系统初始化完成");

    // 全局错误日志记录
    if let Err(err) = run().await {
        // 全局错误日志记录
        error!("应用程序发生致命错误: {}", err);
        // 抛出错误
        return Err(err);
    }

    Ok(())
}
