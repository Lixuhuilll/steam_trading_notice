use tracing::error;

mod config;
mod init;

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    init::smtp_init().await?;

    // 优雅退出
    tokio::signal::ctrl_c().await?;

    Ok(())
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 开发环境下引入 dotenv 读取 .env 文件，方便修改环境变量
    #[cfg(debug_assertions)]
    {
        println!("当前程序编译级别为 debug，尝试读取 .env 文件");
        dotenvy::dotenv().ok();
    }

    // 初始化日志
    let _guards = init::log_init().expect("日志系统初始化失败");

    // 全局错误日志记录
    if let Err(err) = run().await {
        // 全局错误日志记录
        error!("应用程序发生致命错误: {}", err);
        // 抛出错误
        return Err(err);
    }

    Ok(())
}
