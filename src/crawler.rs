use crate::err_type;
use crate::file::create_temp_file;
use reqwest;
use reqwest::Response;
use serde::Deserialize;
use std::sync::OnceLock;
use tokio::fs::File;
use tokio_stream::StreamExt;
use tokio_util::bytes::Bytes;
use tokio_util::io::StreamReader;
use tracing::info;

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

pub async fn get_latest_file_zip() -> err_type::Result<File> {
    let url = format!(
        "https://api.iflow.work/export/download?dir_name=priority_archive&file_name={}",
        get_latest_file_name().await?
    );
    info!("GET {}", url);

    let resp = get_client().get(url).send().await?;
    // 检查请求是否成功
    let resp = resp.error_for_status()?;

    // 写入临时文件
    let mut temp_file = create_temp_file().await?;
    resp_save_to(resp, &mut temp_file).await?;

    Ok(temp_file)
}

async fn resp_save_to(resp: Response, file: &mut File) -> err_type::Result<()> {
    // 获取响应流
    let stream = resp.bytes_stream();
    // 使用 StreamExt 将 error 映射到 std::io::error
    let stream = stream.map(|result: reqwest::Result<Bytes>| {
        result.map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
    });
    let mut stream_reader = StreamReader::new(stream);

    // 写入文件
    tokio::io::copy(&mut stream_reader, file).await?;

    Ok(())
}
