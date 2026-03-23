// src/kafka/producer.rs
use anyhow::{Context, Result};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{BaseProducer, BaseRecord, Producer};
use serde_json::Value;
use std::time::Duration;

use crate::config::ConnectionConfig;

/// 将 JSON 字符串解析为待发送的消息列表
/// Array → 多条，Object/其他 → 单条
pub fn parse_json_to_payloads(input: &str) -> Result<Vec<Vec<u8>>> {
    let value: Value = serde_json::from_str(input.trim())
        .context("JSON 解析失败")?;
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| {
                serde_json::to_vec(item).context("序列化 JSON 元素失败")
            })
            .collect(),
        other => Ok(vec![serde_json::to_vec(&other).context("序列化 JSON 失败")?]),
    }
}

/// 向指定 topic/partition 批量发送消息，返回成功发送条数
/// 阻塞函数，必须通过 tokio::task::spawn_blocking 调用
pub fn send_messages(
    config: ConnectionConfig,
    topic: String,
    partition: i32,
    payloads: Vec<Vec<u8>>,
) -> Result<usize> {
    let producer: BaseProducer = build_producer_config(&config)?.create()?;
    let mut sent = 0usize;

    for payload in &payloads {
        let mut retries = 0usize;
        loop {
            let record = BaseRecord::<(), [u8]>::to(&topic)
                .partition(partition)
                .payload(payload.as_slice());

            match producer.send(record) {
                Ok(()) => {
                    sent += 1;
                    break;
                }
                Err((rdkafka::error::KafkaError::MessageProduction(
                    rdkafka::types::RDKafkaErrorCode::QueueFull,
                ), _)) => {
                    if retries >= 3 {
                        anyhow::bail!("发送队列持续满，已重试 3 次，中止发送");
                    }
                    producer.poll(Duration::from_millis(100));
                    retries += 1;
                }
                Err((e, _)) => {
                    return Err(anyhow::anyhow!("发送失败: {e}"));
                }
            }
        }
    }

    producer.flush(Duration::from_secs(5)).context("flush 超时")?;
    Ok(sent)
}

fn build_producer_config(config: &ConnectionConfig) -> Result<ClientConfig> {
    use crate::config::SecurityProtocol;
    let mut client_config = ClientConfig::new();
    client_config
        .set("bootstrap.servers", &config.brokers)
        .set("security.protocol", config.security_protocol.as_str());

    if let Some(sasl) = &config.sasl {
        client_config
            .set("sasl.mechanism", sasl.mechanism.as_str())
            .set("sasl.username", &sasl.username)
            .set("sasl.password", &sasl.password);
    }

    #[allow(clippy::collapsible_if)]
    if matches!(
        config.security_protocol,
        SecurityProtocol::Ssl | SecurityProtocol::SaslSsl
    ) {
        if let Some(ssl) = &config.ssl {
            if let Some(ca) = &ssl.ca_location {
                client_config.set("ssl.ca.location", ca);
            }
            if let Some(cert) = &ssl.cert_location {
                client_config.set("ssl.certificate.location", cert);
            }
            if let Some(key) = &ssl.key_location {
                client_config.set("ssl.key.location", key);
            }
            if let Some(pwd) = &ssl.key_password {
                client_config.set("ssl.key.password", pwd);
            }
        }
    }
    Ok(client_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_object_returns_single_payload() {
        let payloads = parse_json_to_payloads(r#"{"key":"value"}"#).unwrap();
        assert_eq!(payloads.len(), 1);
        let v: Value = serde_json::from_slice(&payloads[0]).unwrap();
        assert_eq!(v["key"], "value");
    }

    #[test]
    fn parse_array_returns_multiple_payloads() {
        let payloads = parse_json_to_payloads(r#"[{"a":1},{"b":2}]"#).unwrap();
        assert_eq!(payloads.len(), 2);
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        assert!(parse_json_to_payloads("not json").is_err());
    }

    #[test]
    fn parse_empty_array_returns_zero_payloads() {
        let payloads = parse_json_to_payloads("[]").unwrap();
        assert_eq!(payloads.len(), 0);
    }

    #[test]
    fn build_config_with_plain_text() {
        use crate::config::{ConnectionConfig, SecurityProtocol};
        let config = ConnectionConfig {
            brokers: "localhost:9092".to_string(),
            security_protocol: SecurityProtocol::Plaintext,
            ..ConnectionConfig::default()
        };
        // 只验证不 panic，能成功创建 ClientConfig
        assert!(build_producer_config(&config).is_ok());
    }
}
