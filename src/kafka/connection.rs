use anyhow::Result;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::StreamConsumer;

use crate::config::{ConnectionConfig, SecurityProtocol};

/// 根据连接配置创建 Kafka StreamConsumer
pub fn create_consumer(config: &ConnectionConfig) -> Result<StreamConsumer> {
    let mut client_config = ClientConfig::new();

    client_config
        .set("bootstrap.servers", &config.brokers)
        .set("group.id", config.consumer_group_id())
        .set("enable.auto.commit", "false")
        .set("auto.offset.reset", "earliest")
        .set("security.protocol", config.security_protocol.as_str());

    // SASL 配置
    if let Some(sasl) = &config.sasl {
        client_config
            .set("sasl.mechanism", sasl.mechanism.as_str())
            .set("sasl.username", &sasl.username)
            .set("sasl.password", &sasl.password);
    }

    // SSL 配置
    if matches!(
        config.security_protocol,
        SecurityProtocol::Ssl | SecurityProtocol::SaslSsl
    ) && let Some(ssl) = &config.ssl
    {
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

    let consumer: StreamConsumer = client_config.create()?;
    Ok(consumer)
}
