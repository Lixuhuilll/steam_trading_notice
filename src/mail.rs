use crate::config::{CONFIG, MailConfig};
use crate::err_type;
use crate::mail::TlsMode::{STARTTLS, TLS};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::{SUBMISSION_PORT, SUBMISSIONS_PORT};
use lettre::{AsyncSmtpTransport, Tokio1Executor};
use std::time::Duration;
use tracing::{info, warn};

#[derive(Debug)]
pub enum TlsMode {
    TLS,
    STARTTLS,
}

pub async fn smtp_init() -> err_type::Result<AsyncSmtpTransport<Tokio1Executor>> {
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
