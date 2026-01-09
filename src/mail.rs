use crate::config::{CONFIG, MailConfig};
use crate::err_type;
use crate::mail::TlsMode::{STARTTLS, TLS};
use lettre::message::header::ContentType;
use lettre::message::{Mailbox, MessageBuilder};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::{SUBMISSION_PORT, SUBMISSIONS_PORT};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::time::Duration;
use tracing::{error, info, warn};

pub type BuildEMailFn = fn(MessageBuilder) -> err_type::Result<Message>;

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
        ..
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

pub async fn smtp_send_test(mailer: &AsyncSmtpTransport<Tokio1Executor>) {
    let from = &CONFIG.mail.smtp_username;
    let send_to = &CONFIG.mail.smtp_send_to;

    smtp_send(mailer, from, send_to, |email_builder| {
        Ok(email_builder
            .subject("STN 邮件通知功能测试")
            .header(ContentType::TEXT_PLAIN)
            .body("本邮件用于测试您是否能收到 STN 的邮件通知，避免遗失消息".to_owned())?)
    })
    .await;
}

async fn smtp_send(
    mailer: &AsyncSmtpTransport<Tokio1Executor>,
    from: &String,
    send_to: &Vec<String>,
    build_email_fn: BuildEMailFn,
) -> bool {
    let from: Mailbox = match from.parse() {
        Ok(from) => from,
        Err(err) => {
            error!(from = %from, err = %err, "无法解析发件人");
            return true;
        }
    };

    let mut email_builder = Message::builder().from(from);

    for send_to in send_to.iter() {
        let to: Mailbox = match send_to.parse() {
            Ok(send_to) => send_to,
            Err(err) => {
                error!(send_to = %send_to, err = %err, "无法解析收件人");
                continue;
            }
        };

        email_builder = email_builder.bcc(to);
    }

    let email = match build_email_fn(email_builder) {
        Ok(email) => email,
        Err(err) => {
            error!(err = %err, "无法构建邮件");
            return true;
        }
    };

    info!("本次发送的目标收件人地址为：{:#?}", email.envelope().to());

    match mailer.send(email).await {
        Ok(res) => {
            info!(
                "邮件服务器回应 code={} {}",
                res.code(),
                res.first_line().unwrap_or("None")
            );
        }
        Err(err) => {
            error!(err = %err, "邮件发送失败");
            return true;
        }
    };

    false
}
