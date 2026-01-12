use crate::config::CONFIG;
use crate::err_type;
use reqwest;
use serde::Deserialize;
use serde_json::json;
use std::sync::OnceLock;
use tokio_stream::StreamExt;
use tracing::{debug, info, trace};

const MAX_RESPONSE_SIZE: usize = 1 * 1024 * 1024; // 1 MB
static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

#[derive(Debug, Default, Deserialize)]
pub struct DataDumpList {
    pub files: Vec<String>,
    pub success: bool,
}

// http client 初始化函数：只能调用一次，后续调用会被忽略
pub fn client_init() -> err_type::Result<()> {
    let client = reqwest::Client::builder()
        // 在这里可以自定义配置，例如超时、User-Agent 等
        .timeout(std::time::Duration::from_secs(30))
        .gzip(true)
        .build()?;

    CLIENT.set(client).map_err(|_| "HTTP 客户端已初始化")?;

    Ok(())
}

// 获取 client 的引用
pub fn get_client() -> &'static reqwest::Client {
    CLIENT.get().expect("HTTP 客户端还未初始化")
}

pub async fn get_website_jpeg() -> err_type::Result<Vec<u8>> {
    let url = format!(
        "https://production-sfo.browserless.io/chrome/screenshot?token={}",
        CONFIG.browserless.token
    );

    let json = json!({
        "waitForSelector": {
            "hidden": true,
            "selector": ".ant-spin"
        },
        "url": "https://www.iflow.work/?page_num=1&platforms=uuyp-buff-igxe-eco-c5&games=csgo-dota2&sort_by=safe_buy&min_price=1&max_price=5000&min_volume=10000&max_latency=600&price_mode=buy",
        "options": {
            "type": "jpeg",
            "fullPage": true,
            "encoding": "binary"
        }
    });

    // 发起请求
    let resp = get_client().post(url).json(&json).send().await?;

    // 检查请求是否成功
    let resp = resp.error_for_status()?;

    // 获取原始字节数据
    let bytes = limited_bytes(resp, MAX_RESPONSE_SIZE).await?;

    Ok(bytes)
}

async fn limited_bytes(resp: reqwest::Response, limit: usize) -> err_type::Result<Vec<u8>> {
    let content_length = resp.content_length().unwrap_or(0);

    debug!("声明的响应体体积为：{} Bytes", content_length);

    if content_length > limit as u64 {
        return Err(format!(
            "声明的响应体体积为：{} Bytes，声明的响应体过大",
            content_length
        )
        .into());
    }

    let mut bytes = if content_length > 0 {
        Vec::with_capacity(content_length as usize)
    } else {
        Vec::new()
    };

    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.try_next().await? {
        let chunk_length = chunk.len();
        let total_length = bytes.len() + chunk_length;

        trace!("分块的大小为：{} Bytes", chunk_length);

        if total_length > limit {
            return Err(format!(
                "实际的响应体体积达到了：{} Bytes，实际的响应体过大，提前终止传输",
                total_length
            )
            .into());
        }

        bytes.extend_from_slice(&chunk);
    }

    debug!("实际的响应体体积为：{} Bytes", bytes.len());

    Ok(bytes)
}
