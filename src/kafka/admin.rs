// src/kafka/admin.rs
use anyhow::{Context, Result};
use rdkafka::admin::{AdminClient, AdminOptions, AlterConfig, ResourceSpecifier};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::TopicPartitionList;
use rdkafka::topic_partition_list::Offset;
use std::time::Duration;

use crate::config::ConnectionConfig;

/// 拉取 topic 的 retention.ms 和 retention.bytes 配置
/// 返回 (retention_secs, retention_gb, retention_gb_note)
/// retention_gb_note：若 retention.bytes 不是 GB 整数倍，返回原始 bytes 说明
pub async fn describe_topic_config(
    config: ConnectionConfig,
    topic: String,
) -> Result<(String, String, Option<String>)> {
    let admin = build_admin(&config)?;
    let resource = ResourceSpecifier::Topic(topic.as_str());
    let results = admin
        .describe_configs(
            &[resource],
            &AdminOptions::new().request_timeout(Some(Duration::from_secs(10))),
        )
        .await
        .context("describe_configs 失败")?;

    // ConfigResourceResult = Result<ConfigResource, RDKafkaErrorCode>
    let topic_result = results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("未收到配置结果"))?
        .map_err(|e| anyhow::anyhow!("获取配置失败: {:?}", e))?;

    let get = |key: &str| -> Option<String> {
        // ConfigResource::get 返回 Option<&ConfigEntry>，ConfigEntry.value 是 Option<String>
        topic_result.get(key).and_then(|e| e.value.clone())
    };

    // retention.ms → 秒
    let retention_secs = match get("retention.ms").as_deref() {
        Some("-1") | None => "-1".to_string(),
        Some(ms_str) => ms_str
            .parse::<i64>()
            .map(|ms| (ms / 1000).to_string())
            .unwrap_or_else(|_| "-1".to_string()),
    };

    // retention.bytes → GB（四舍五入）
    let (retention_gb, retention_gb_note) = match get("retention.bytes").as_deref() {
        Some("-1") | None => ("-1".to_string(), None),
        Some(bytes_str) => match bytes_str.parse::<i64>() {
            Ok(bytes) => {
                let gb = (bytes as f64 / 1_073_741_824.0).round() as i64;
                let note = if bytes % 1_073_741_824 != 0 {
                    Some(format!("原值 {} bytes，已取整", bytes))
                } else {
                    None
                };
                (gb.to_string(), note)
            }
            Err(_) => ("-1".to_string(), None),
        },
    };

    Ok((retention_secs, retention_gb, retention_gb_note))
}

/// 修改 topic 的 retention.ms 和 retention.bytes
/// retention_ms: 毫秒（-1 无限制），retention_bytes: bytes（-1 无限制）
pub async fn alter_topic_config(
    config: ConnectionConfig,
    topic: String,
    retention_ms: i64,
    retention_bytes: i64,
) -> Result<()> {
    let admin = build_admin(&config)?;
    let ms_str = retention_ms.to_string();
    let bytes_str = retention_bytes.to_string();
    let alter = AlterConfig::new(ResourceSpecifier::Topic(&topic))
        .set("retention.ms", &ms_str)
        .set("retention.bytes", &bytes_str);

    // AlterConfigsResult = Result<OwnedResourceSpecifier, (OwnedResourceSpecifier, RDKafkaErrorCode)>
    let results = admin
        .alter_configs(
            &[alter],
            &AdminOptions::new().request_timeout(Some(Duration::from_secs(15))),
        )
        .await
        .context("alter_configs 失败")?;

    for result in results {
        result.map_err(|(specifier, code)| {
            anyhow::anyhow!("修改配置失败: {:?}, 错误码: {:?}", specifier, code)
        })?;
    }
    Ok(())
}

/// 清空 topic 所有分区数据（delete_records 到 high watermark）
pub async fn purge_topic(
    config: ConnectionConfig,
    topic: String,
    partitions: Vec<i32>,
) -> Result<()> {
    // 先用 BaseConsumer 拉取各分区的 high watermark
    let consumer: BaseConsumer = build_consumer_config(&config)?.create()?;
    let mut tpl = TopicPartitionList::new();

    for partition in &partitions {
        let (_, high) = consumer
            .fetch_watermarks(&topic, *partition, Duration::from_secs(5))
            .with_context(|| format!("获取 partition {} watermark 失败", partition))?;
        tpl.add_partition_offset(&topic, *partition, Offset::Offset(high))
            .with_context(|| format!("设置 partition {} offset 失败", partition))?;
    }

    let admin = build_admin(&config)?;
    // delete_records 返回 KafkaResult<TopicPartitionList>，通过 elements() 遍历各分区结果
    let result_tpl = admin
        .delete_records(&tpl, &AdminOptions::new().request_timeout(Some(Duration::from_secs(30))))
        .await
        .context("delete_records 失败")?;

    for elem in result_tpl.elements() {
        elem.error().with_context(|| {
            format!("清空 partition {} 失败", elem.partition())
        })?;
    }
    Ok(())
}

fn build_admin(config: &ConnectionConfig) -> Result<AdminClient<DefaultClientContext>> {
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

    if matches!(
        config.security_protocol,
        SecurityProtocol::Ssl | SecurityProtocol::SaslSsl
    ) {
        if let Some(ssl) = &config.ssl {
            if let Some(ca) = &ssl.ca_location {
                client_config.set("ssl.ca.location", ca);
            }
        }
    }
    Ok(client_config.create()?)
}

fn build_consumer_config(config: &ConnectionConfig) -> Result<ClientConfig> {
    let mut client_config = ClientConfig::new();
    client_config
        .set("bootstrap.servers", &config.brokers)
        .set("security.protocol", config.security_protocol.as_str())
        .set("enable.auto.commit", "false")
        .set("enable.auto.offset.store", "false");

    if let Some(sasl) = &config.sasl {
        client_config
            .set("sasl.mechanism", sasl.mechanism.as_str())
            .set("sasl.username", &sasl.username)
            .set("sasl.password", &sasl.password);
    }
    Ok(client_config)
}

#[cfg(test)]
mod tests {
    #[test]
    fn bytes_to_gb_round_trip() {
        // 精确整数 GB
        let bytes: i64 = 5 * 1_073_741_824;
        let gb = (bytes as f64 / 1_073_741_824.0).round() as i64;
        assert_eq!(gb, 5);
    }

    #[test]
    fn bytes_non_integer_gb_rounds_up() {
        // 2,000,000,000 bytes ≈ 1.86 GB → rounds to 2
        let bytes: i64 = 2_000_000_000;
        let gb = (bytes as f64 / 1_073_741_824.0).round() as i64;
        assert_eq!(gb, 2);
        assert_ne!(bytes % 1_073_741_824, 0);
    }
}
