use crate::err_type;
use reqwest;
use serde::Deserialize;
use std::io::Read;
use std::sync::OnceLock;
use tokio::task;
use tokio_stream::StreamExt;
use tracing::info;
use zip::ZipArchive;
use zip::result::ZipError;

const MAX_RESPONSE_SIZE: usize = 10 * 1024 * 1024; // 10 MB
const MAX_UNCOMPRESSED_SIZE: u64 = 30 * 1024 * 1024; // 30 MB
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

pub async fn get_latest_file_name() -> err_type::Result<String> {
    let mut data_dump_list: DataDumpList = get_client()
        .get("https://api.iflow.work/export/list?dir_name=priority_archive")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if data_dump_list.success && !data_dump_list.files.is_empty() {
        Ok(data_dump_list.files.pop().unwrap())
    } else {
        Err("DataDump 文件列表获取失败".into())
    }
}

pub async fn get_latest_data() -> err_type::Result<String> {
    let url = format!(
        "https://api.iflow.work/export/download?dir_name=priority_archive&file_name={}",
        get_latest_file_name().await?
    );
    info!("GET {}", url);

    let resp = get_client().get(url).send().await?;
    // 检查请求是否成功
    let resp = resp.error_for_status()?;

    // 获取原始字节数据
    let bytes = limited_bytes(resp, MAX_RESPONSE_SIZE).await?;

    // 构建并解压 ZIP 文件（文件正常不超过 10 MB，没必要搞那么复杂）
    let data = task::spawn_blocking(move || {
        let cursor = std::io::Cursor::new(bytes);
        let mut zip = ZipArchive::new(cursor)?;

        if zip.is_empty() {
            return Err(ZipError::InvalidArchive(
                "获取数据失败，空的 ZIP 文件".into(),
            ));
        }

        let mut file = zip.by_index(0)?;
        if file.size() > MAX_UNCOMPRESSED_SIZE {
            return Err(ZipError::InvalidArchive("文件解压后过大".into()));
        }

        // 预分配容量
        let mut data = String::with_capacity(file.size() as usize);
        file.read_to_string(&mut data)?;

        Ok(data)
    })
    .await??;

    Ok(data)
}

async fn limited_bytes(resp: reqwest::Response, limit: usize) -> err_type::Result<Vec<u8>> {
    let content_length = resp.content_length().unwrap_or(0);

    if content_length > limit as u64 {
        return Err("声明的响应体过大".into());
    }

    let mut body = if content_length > 0 {
        Vec::with_capacity(content_length as usize)
    } else {
        Vec::new()
    };

    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.try_next().await? {
        if body.len() + chunk.len() > limit {
            return Err("实际响应体过大".into());
        }
        body.extend_from_slice(&chunk);
    }

    Ok(body)
}
