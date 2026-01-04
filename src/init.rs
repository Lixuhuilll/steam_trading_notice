use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;

pub fn log_init() -> Result<Vec<WorkerGuard>, Box<dyn std::error::Error>> {
    // 控制台日志和文件日志的公共配置
    let formatter = tracing_subscriber::fmt::format().with_thread_names(true);

    // 初始化控制台日志非阻塞写入
    let (console_non_blocking, console_guard) =
        tracing_appender::non_blocking::NonBlockingBuilder::default()
            .thread_name("tracing-appender-console")
            .finish(std::io::stdout());

    // 控制台日志格式
    let console_layer = tracing_subscriber::fmt::layer()
        .event_format(formatter.clone())
        .with_writer(console_non_blocking)
        .with_ansi(true);

    // 初始化文件日志非阻塞写入
    let file_appender = tracing_appender::rolling::hourly("logs", "steam_trading_notice.log");
    let (file_non_blocking, file_guard) =
        tracing_appender::non_blocking::NonBlockingBuilder::default()
            .thread_name("tracing-appender-file")
            .finish(file_appender);

    // 文件日志格式
    let file_layer = tracing_subscriber::fmt::layer()
        .event_format(formatter)
        .with_writer(file_non_blocking)
        .with_ansi(false);

    let subscriber = tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer);

    tracing::subscriber::set_global_default(subscriber)?;

    // 所有的 WorkerGuard 都必须返回到 main 函数，妥善保管直至程序结束
    Ok(vec![console_guard, file_guard])
}