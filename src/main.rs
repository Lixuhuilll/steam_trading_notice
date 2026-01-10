use crate::mail::smtp_send_test;
use lettre::AsyncTransport;
use tracing::error;

mod config;
mod crawler;
mod err_type;
mod log;
mod mail;
mod scheduler;

async fn run() -> err_type::Result<()> {
    let mailer = mail::smtp_init().await?;
    smtp_send_test(&mailer).await;

    // 优雅退出
    tokio::signal::ctrl_c().await?;
    mailer.shutdown().await;

    Ok(())
}

#[tokio::main]
pub async fn main() -> err_type::Result<()> {
    // 开发环境下引入 dotenv 读取 .env 文件，方便修改环境变量
    #[cfg(debug_assertions)]
    {
        println!("当前程序编译级别为 debug，尝试读取 .env 文件");
        dotenvy::dotenv().ok();
    }

    // 初始化日志
    let _guards = log::log_init().expect("日志系统初始化失败");

    // 全局错误日志记录
    if let Err(err) = run().await {
        // 全局错误日志记录
        error!("应用程序发生致命错误: {}", err);
        // 抛出错误
        return Err(err);
    }

    Ok(())
}
