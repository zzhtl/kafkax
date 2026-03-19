use anyhow::Context as _;
use serde::{Deserialize, Serialize};

/// SASL 认证机制
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SaslMechanism {
    #[default]
    Plain,
    ScramSha256,
    ScramSha512,
}

impl SaslMechanism {
    pub const ALL: [Self; 3] = [Self::Plain, Self::ScramSha256, Self::ScramSha512];

    pub fn as_str(&self) -> &'static str {
        match self {
            SaslMechanism::Plain => "PLAIN",
            SaslMechanism::ScramSha256 => "SCRAM-SHA-256",
            SaslMechanism::ScramSha512 => "SCRAM-SHA-512",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SaslMechanism::Plain => "PLAIN",
            SaslMechanism::ScramSha256 => "SCRAM-SHA-256",
            SaslMechanism::ScramSha512 => "SCRAM-SHA-512",
        }
    }
}

/// 安全协议
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SecurityProtocol {
    #[default]
    Plaintext,
    Ssl,
    SaslPlaintext,
    SaslSsl,
}

impl SecurityProtocol {
    pub const ALL: [Self; 4] = [
        Self::Plaintext,
        Self::Ssl,
        Self::SaslPlaintext,
        Self::SaslSsl,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            SecurityProtocol::Plaintext => "plaintext",
            SecurityProtocol::Ssl => "ssl",
            SecurityProtocol::SaslPlaintext => "sasl_plaintext",
            SecurityProtocol::SaslSsl => "sasl_ssl",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SecurityProtocol::Plaintext => "PLAINTEXT",
            SecurityProtocol::Ssl => "SSL",
            SecurityProtocol::SaslPlaintext => "SASL_PLAINTEXT",
            SecurityProtocol::SaslSsl => "SASL_SSL",
        }
    }

    pub fn uses_sasl(&self) -> bool {
        matches!(self, Self::SaslPlaintext | Self::SaslSsl)
    }

    pub fn uses_ssl(&self) -> bool {
        matches!(self, Self::Ssl | Self::SaslSsl)
    }
}

/// SASL 配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SaslConfig {
    pub mechanism: SaslMechanism,
    pub username: String,
    pub password: String,
}

/// SSL 配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SslConfig {
    pub ca_location: Option<String>,
    pub cert_location: Option<String>,
    pub key_location: Option<String>,
    pub key_password: Option<String>,
}

/// 连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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

    /// 返回安全配置摘要，便于在界面上快速识别连接类型
    pub fn security_summary(&self) -> String {
        let mut parts = vec![self.security_protocol.label().to_string()];

        if let Some(sasl) = &self.sasl {
            parts.push(sasl.mechanism.label().to_string());

            if !sasl.username.trim().is_empty() {
                parts.push(sasl.username.trim().to_string());
            }
        }

        if self
            .ssl
            .as_ref()
            .and_then(|ssl| ssl.cert_location.as_deref())
            .is_some()
        {
            parts.push("Mutual TLS".to_string());
        }

        parts.join(" · ")
    }
}

/// 应用配置（包含所有连接）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub connections: Vec<ConnectionConfig>,
    pub last_connection: Option<String>,
}

impl AppConfig {
    /// 从配置文件加载
    pub fn load() -> anyhow::Result<Self> {
        let config_path = Self::config_path();
        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("读取配置文件失败: {}", config_path.display()))?;

        toml::from_str(&content)
            .with_context(|| format!("解析配置文件失败: {}", config_path.display()))
    }

    /// 保存到配置文件
    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建配置目录失败: {}", parent.display()))?;
        }
        let content = toml::to_string_pretty(self).context("序列化应用配置失败")?;
        std::fs::write(&config_path, content)
            .with_context(|| format!("写入配置文件失败: {}", config_path.display()))?;
        Ok(())
    }

    /// 新增或更新连接，并将其置顶为最近使用
    pub fn upsert_connection(&mut self, connection: ConnectionConfig) {
        self.connections.retain(|item| item.name != connection.name);
        self.last_connection = Some(connection.name.clone());
        self.connections.insert(0, connection);
    }

    /// 删除指定连接
    pub fn remove_connection(&mut self, name: &str) -> bool {
        let before = self.connections.len();
        self.connections
            .retain(|connection| connection.name != name);

        if self.last_connection.as_deref() == Some(name) {
            self.last_connection = self
                .connections
                .first()
                .map(|connection| connection.name.clone());
        }

        before != self.connections.len()
    }

    /// 根据名称查找连接
    pub fn connection_by_name(&self, name: &str) -> Option<&ConnectionConfig> {
        self.connections
            .iter()
            .find(|connection| connection.name == name)
    }

    /// 返回最近一次使用的连接
    pub fn last_connection_config(&self) -> Option<&ConnectionConfig> {
        self.last_connection
            .as_deref()
            .and_then(|name| self.connection_by_name(name))
            .or_else(|| self.connections.first())
    }

    fn config_path() -> std::path::PathBuf {
        directories::ProjectDirs::from("com", "kafkax", "KafkaX")
            .map(|dirs| dirs.config_dir().join("config.toml"))
            .unwrap_or_else(|| std::path::PathBuf::from("kafkax.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, ConnectionConfig, SecurityProtocol};

    fn sample_connection(name: &str, brokers: &str) -> ConnectionConfig {
        ConnectionConfig {
            name: name.to_string(),
            brokers: brokers.to_string(),
            security_protocol: SecurityProtocol::Plaintext,
            sasl: None,
            ssl: None,
            group_id: Some(format!("group-{name}")),
        }
    }

    #[test]
    fn upsert_connection_moves_latest_to_front() {
        let mut config = AppConfig::default();

        config.upsert_connection(sample_connection("prod", "10.0.0.1:9092"));
        config.upsert_connection(sample_connection("test", "10.0.0.2:9092"));
        config.upsert_connection(sample_connection("prod", "10.0.0.3:9092"));

        assert_eq!(config.connections.len(), 2);
        assert_eq!(config.connections[0].name, "prod");
        assert_eq!(config.connections[0].brokers, "10.0.0.3:9092");
        assert_eq!(config.last_connection.as_deref(), Some("prod"));
    }

    #[test]
    fn remove_connection_updates_last_connection() {
        let mut config = AppConfig::default();

        config.upsert_connection(sample_connection("prod", "10.0.0.1:9092"));
        config.upsert_connection(sample_connection("test", "10.0.0.2:9092"));

        assert!(config.remove_connection("test"));
        assert_eq!(config.last_connection.as_deref(), Some("prod"));
        assert_eq!(config.connections.len(), 1);

        assert!(config.remove_connection("prod"));
        assert!(config.last_connection.is_none());
        assert!(config.connections.is_empty());
    }
}
