use crate::config::{CONFIG, MailConfig};
use crate::crawler::get_website_jpeg;
use crate::email::TlsMode::{STARTTLS, TLS};
use crate::err_type;
use lettre::message::{Attachment, Mailbox, MessageBuilder, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::{SUBMISSION_PORT, SUBMISSIONS_PORT};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Debug)]
pub enum TlsMode {
    TLS,
    STARTTLS,
}

static MAILER: OnceLock<AsyncSmtpTransport<Tokio1Executor>> = OnceLock::new();

pub async fn smtp_init() -> err_type::Result<()> {
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
                MAILER.set(transport).map_err(|_| "SMTP 客户端已初始化")?;
                return Ok(());
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

pub fn get_mailer() -> &'static AsyncSmtpTransport<Tokio1Executor> {
    MAILER.get().expect("SMTP 客户端还未初始化")
}

pub async fn smtp_send_test() -> bool {
    let from = &CONFIG.mail.smtp_username;
    let send_to = &CONFIG.mail.smtp_send_to;

    smtp_send(get_mailer(), from, send_to, async |email_builder| {
        let html = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>STN 邮件通知功能测试</title>
</head>
<body>
    <div style="display: flex; flex-direction: column; align-items: center;">
        <h1 style="font-family: Arial, Helvetica, sans-serif;">本邮件用于测试您是否能收到 STN 的邮件通知，避免遗失消息</h1>
        <p>以下是当前的 Steam 挂刀情报：</p>
        <img src=cid:screenshot alt="Steam 挂刀情报站的截图，如未显示图像请检查邮箱的相关设置">
    </div>
</body>
</html>"#;

        let jpeg = get_website_jpeg().await?;

        let email = email_builder
            .subject("STN 邮件通知功能测试")
            .multipart(
                // 将纯文本和 HTML 消息合并成一封邮件
                MultiPart::alternative()
                    // 纯文本用于在 HTML 无法显示时展示回退信息
                    .singlepart(
                        SinglePart::plain("您的邮箱无法显示 HTML 邮件，请检查安全设置或者更换更加现代化的邮箱系统".to_owned())
                    )
                    // 主要内容由 HTML 展示
                    .multipart(
                        // 将图片和 HTML 混合起来
                        MultiPart::related()
                            .singlepart(
                                SinglePart::html(html.to_owned())
                            )
                            .singlepart(
                                Attachment::new_inline("screenshot".to_owned())
                                    .body(jpeg, "image/jpeg".parse()?)
                            )
                    ),
            )?;

        Ok(email)
    }).await
}

async fn smtp_send(
    mailer: &AsyncSmtpTransport<Tokio1Executor>,
    from: &String,
    send_to: &Vec<String>,
    build_email: impl AsyncFnOnce(MessageBuilder) -> err_type::Result<Message>,
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

    let email = match build_email(email_builder).await {
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
