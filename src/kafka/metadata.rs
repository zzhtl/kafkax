use anyhow::Result;
use rdkafka::consumer::Consumer;
use rdkafka::consumer::StreamConsumer;

use crate::kafka::types::{PartitionMeta, TopicMeta};

/// 从 Kafka 集群拉取 topic/partition 元数据
pub fn fetch_metadata(consumer: &StreamConsumer) -> Result<Vec<TopicMeta>> {
    let metadata = consumer.fetch_metadata(None, std::time::Duration::from_secs(10))?;

    let topics: Vec<TopicMeta> = metadata
        .topics()
        .iter()
        .filter(|t| !t.name().starts_with("__")) // 过滤内部 topic
        .map(|topic| TopicMeta {
            name: topic.name().to_string(),
            partitions: topic
                .partitions()
                .iter()
                .map(|p| PartitionMeta {
                    id: p.id(),
                    leader: p.leader(),
                    replicas: p.replicas().to_vec(),
                    isr: p.isr().to_vec(),
                })
                .collect(),
        })
        .collect();

    Ok(topics)
}

/// 获取指定 partition 的 watermark（low, high offset）
pub fn fetch_watermarks(
    consumer: &StreamConsumer,
    topic: &str,
    partition: i32,
) -> Result<(i64, i64)> {
    let (low, high) =
        consumer.fetch_watermarks(topic, partition, std::time::Duration::from_secs(5))?;
    Ok((low, high))
}
