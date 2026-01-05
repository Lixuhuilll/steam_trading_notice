use config::{Config, Environment, File};
use serde::{Deserialize, Deserializer};
use std::sync::LazyLock;
use tracing_subscriber::filter::LevelFilter;

const CONFIG_FILE_NAME: &str = "stn_config.toml";
const CONFIG_ENV_PREFIX: &str = "STN_CONFIG";

pub static CONFIG: LazyLock<AppConfig> = LazyLock::new(|| {
    Config::builder()
        .add_source(File::with_name(CONFIG_FILE_NAME).required(false))
        .add_source(Environment::with_prefix(CONFIG_ENV_PREFIX).separator("__"))
        .build()
        .expect("构建配置错误")
        .try_deserialize()
        .expect("反序列化配置文件错误")
});

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub mail: MailConfig,
    pub log: LogConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct MailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_timeout: u64,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct LogConfig {
    #[serde(
        default = "default_max_level",
        deserialize_with = "deserialize_level_filter"
    )]
    pub max_level: LevelFilter,
}

fn default_max_level() -> LevelFilter {
    LevelFilter::INFO
}

fn deserialize_level_filter<'de, D>(deserializer: D) -> Result<LevelFilter, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            max_level: default_max_level(),
        }
    }
}
