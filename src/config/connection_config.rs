use serde::{Deserialize, Serialize};

/// SASL 认证机制
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SaslMechanism {
    Plain,
    ScramSha256,
    ScramSha512,
}

impl SaslMechanism {
    pub fn as_str(&self) -> &'static str {
        match self {
            SaslMechanism::Plain => "PLAIN",
            SaslMechanism::ScramSha256 => "SCRAM-SHA-256",
            SaslMechanism::ScramSha512 => "SCRAM-SHA-512",
        }
    }
}

/// 安全协议
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityProtocol {
    Plaintext,
    Ssl,
    SaslPlaintext,
    SaslSsl,
}

impl SecurityProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            SecurityProtocol::Plaintext => "plaintext",
            SecurityProtocol::Ssl => "ssl",
            SecurityProtocol::SaslPlaintext => "sasl_plaintext",
            SecurityProtocol::SaslSsl => "sasl_ssl",
        }
    }
}

/// SASL 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaslConfig {
    pub mechanism: SaslMechanism,
    pub username: String,
    pub password: String,
}

/// SSL 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslConfig {
    pub ca_location: Option<String>,
    pub cert_location: Option<String>,
    pub key_location: Option<String>,
    pub key_password: Option<String>,
}

/// 连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub name: String,
    pub brokers: String,
    pub security_protocol: SecurityProtocol,
    pub sasl: Option<SaslConfig>,
    pub ssl: Option<SslConfig>,
    pub group_id: Option<String>,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            name: "默认连接".to_string(),
            brokers: "localhost:9092".to_string(),
            security_protocol: SecurityProtocol::Plaintext,
            sasl: None,
            ssl: None,
            group_id: None,
        }
    }
}

impl ConnectionConfig {
    /// 生成消费者组 ID
    pub fn consumer_group_id(&self) -> String {
        self.group_id
            .clone()
            .unwrap_or_else(|| format!("kafkax-{}", uuid::Uuid::new_v4()))
    }
}

/// 应用配置（包含所有连接）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub connections: Vec<ConnectionConfig>,
    pub last_connection: Option<String>,
}

impl AppConfig {
    /// 从配置文件加载
    pub fn load() -> Self {
        let config_path = Self::config_path();
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path).unwrap_or_default();
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// 保存到配置文件
    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    fn config_path() -> std::path::PathBuf {
        directories::ProjectDirs::from("com", "kafkax", "KafkaX")
            .map(|dirs| dirs.config_dir().join("config.toml"))
            .unwrap_or_else(|| std::path::PathBuf::from("kafkax.toml"))
    }
}
