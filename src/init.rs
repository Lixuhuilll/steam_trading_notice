use crate::config::{CONFIG, MailConfig};
use crate::init::TlsMode::{STARTTLS, TLS};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::{SUBMISSION_PORT, SUBMISSIONS_PORT};
use lettre::{AsyncSmtpTransport, Tokio1Executor};
use std::time::Duration;
use tracing::{info, warn};
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
        .with(file_layer)
        .with(CONFIG.log.max_level);

    tracing::subscriber::set_global_default(subscriber)?;

    info!("日志系统初始化完成，max_level={}", CONFIG.log.max_level);

    // 所有的 WorkerGuard 都必须返回到 main 函数，妥善保管直至程序结束
    Ok(vec![console_guard, file_guard])
}

#[derive(Debug)]
enum TlsMode {
    TLS,
    STARTTLS,
}

pub async fn smtp_init() -> Result<AsyncSmtpTransport<Tokio1Executor>, Box<dyn std::error::Error>> {
    let MailConfig {
        smtp_host,
        smtp_port,
        smtp_username,
        smtp_password,
        smtp_timeout,
    } = &CONFIG.mail;

    if smtp_host.len() == 0 || smtp_username.len() == 0 || smtp_password.len() == 0 {
        let err = format!(
            "smtp 参数异常，主机名、用户名、密码必须全部具备，host={}，username={}，password.len={}",
            smtp_host,
            smtp_username,
            smtp_password.len()
        );
        return Err(err.into());
    }

    for mode in [TLS, STARTTLS] {
        let mut smtp_port = *smtp_port;

        let mut transport_build = match mode {
            TLS => {
                if smtp_port == 0 {
                    smtp_port = SUBMISSIONS_PORT;
                }
                AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_host)?.port(smtp_port)
            }
            STARTTLS => {
                if smtp_port == 0 {
                    smtp_port = SUBMISSION_PORT;
                }
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_host)?.port(smtp_port)
            }
        };

        info!(
            "尝试与 SMTP 服务器（{}:{}）建立 {:?} 加密连接",
            smtp_host, smtp_port, mode
        );

        if *smtp_timeout > 0 {
            transport_build = transport_build.timeout(Some(Duration::from_secs(*smtp_timeout)));
        }

        let transport: AsyncSmtpTransport<Tokio1Executor> = transport_build
            .credentials(Credentials::new(
                smtp_username.to_owned(),
                smtp_password.to_owned(),
            ))
            .build();

        match transport.test_connection().await {
            Ok(true) => {
                info!(
                    "成功与 SMTP 服务器（{}:{}）建立 {:?} 加密连接。",
                    smtp_host, smtp_port, mode
                );

                return Ok(transport);
            }
            Ok(false) => {
                warn!(
                    "无法与 SMTP 服务器（{}:{}）建立 {:?} 加密连接。 没有更多相关信息了。",
                    smtp_host, smtp_port, mode
                );
            }
            Err(err) => {
                warn!(err = %err, "无法与 SMTP 服务器（{}:{}）建立 {:?} 加密连接。", smtp_host, smtp_port, mode);
            }
        };
    }

    Err("SMTP 服务器连接失败".into())
}
