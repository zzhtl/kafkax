use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rdkafka::TopicPartitionList;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;

use crate::codec::DecoderPipeline;
use crate::config::ConnectionConfig;
use crate::kafka::connection;
use crate::kafka::types::{KafkaMessage, OffsetRange, PageData, SearchResult};

/// 从指定 partition 的 offset 开始消费一页数据
pub async fn fetch_page(
    consumer: &StreamConsumer,
    topic: &str,
    partition: i32,
    start_offset: i64,
    page_size: usize,
    decoder: &DecoderPipeline,
) -> Result<PageData> {
    // 获取 watermark
    let (low, high) = consumer.fetch_watermarks(topic, partition, Duration::from_secs(5))?;

    // 如果无消息
    if high <= low {
        return Ok(PageData {
            messages: vec![],
            page: 0,
            total_messages: 0,
            low_watermark: low,
            high_watermark: high,
        });
    }

    // 修正 offset 范围
    let actual_offset = start_offset.max(low).min(high);

    // 使用 assign 精确定位
    let mut tpl = TopicPartitionList::new();
    tpl.add_partition_offset(topic, partition, rdkafka::Offset::Offset(actual_offset))?;
    consumer.assign(&tpl)?;

    // 消费指定数量的消息
    let mut raw_messages = Vec::with_capacity(page_size);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);

    while raw_messages.len() < page_size {
        let remaining = deadline - tokio::time::Instant::now();
        if remaining.is_zero() {
            break;
        }

        match tokio::time::timeout(remaining.min(Duration::from_millis(500)), consumer.recv()).await
        {
            Ok(Ok(msg)) => {
                let kafka_msg = KafkaMessage {
                    topic: msg.topic().to_string(),
                    partition: msg.partition(),
                    offset: msg.offset(),
                    timestamp: msg.timestamp().to_millis().and_then(|ms| {
                        DateTime::from_timestamp_millis(ms).map(|dt| dt.with_timezone(&Utc))
                    }),
                    key: msg.key().map(|k| k.to_vec()),
                    payload: msg.payload().map(|p| p.to_vec()),
                };
                raw_messages.push(kafka_msg);
            }
            Ok(Err(e)) => {
                tracing::warn!("消费消息失败: {}", e);
                break;
            }
            Err(_) => {
                // 超时，可能已经没有更多消息了
                if raw_messages.is_empty() {
                    continue;
                }
                break;
            }
        }
    }

    // 批量解码
    let decoded = decoder.decode_batch(raw_messages);

    // 计算页码
    let total = high - low;
    let page = if total > 0 {
        ((actual_offset - low) / page_size as i64) as usize
    } else {
        0
    };

    Ok(PageData {
        messages: decoded,
        page,
        total_messages: total,
        low_watermark: low,
        high_watermark: high,
    })
}

/// 获取 offset 范围
pub fn get_offset_range(
    consumer: &StreamConsumer,
    topic: &str,
    partition: i32,
) -> Result<OffsetRange> {
    let (low, high) = consumer.fetch_watermarks(topic, partition, Duration::from_secs(5))?;
    Ok(OffsetRange { low, high })
}

/// 搜索指定 partition 中所有仍保留的消息
pub async fn search_partition(
    connection_config: &ConnectionConfig,
    topic: &str,
    partition: i32,
    query: &str,
    decoder: &DecoderPipeline,
) -> Result<SearchResult> {
    let mut search_config = connection_config.clone();
    let group_id = connection_config.consumer_group_id();
    search_config.group_id = Some(format!("{}-search-{}", group_id, uuid::Uuid::new_v4()));

    let consumer = connection::create_consumer(&search_config)?;
    let (low, high) = consumer.fetch_watermarks(topic, partition, Duration::from_secs(5))?;

    if high <= low {
        return Ok(SearchResult {
            matches: Vec::new(),
            scanned_messages: 0,
            low_watermark: low,
            high_watermark: high,
        });
    }

    let mut tpl = TopicPartitionList::new();
    tpl.add_partition_offset(topic, partition, rdkafka::Offset::Offset(low))?;
    consumer.assign(&tpl)?;

    let mut scanned_messages = 0usize;
    let mut matches = Vec::new();
    let mut timeout_streak = 0usize;
    let final_offset = high.saturating_sub(1);
    let mut last_offset = low.saturating_sub(1);

    while last_offset < final_offset {
        match tokio::time::timeout(Duration::from_secs(2), consumer.recv()).await {
            Ok(Ok(msg)) => {
                timeout_streak = 0;
                last_offset = msg.offset();
                scanned_messages += 1;

                let kafka_message = KafkaMessage {
                    topic: msg.topic().to_string(),
                    partition: msg.partition(),
                    offset: msg.offset(),
                    timestamp: msg.timestamp().to_millis().and_then(|ms| {
                        DateTime::from_timestamp_millis(ms).map(|dt| dt.with_timezone(&Utc))
                    }),
                    key: msg.key().map(|k| k.to_vec()),
                    payload: msg.payload().map(|p| p.to_vec()),
                };

                let decoded = decoder.decode(kafka_message);
                if decoded.matches_query(query) {
                    matches.push(decoded);
                }
            }
            Ok(Err(error)) => {
                return Err(error.into());
            }
            Err(_) => {
                timeout_streak += 1;
                if timeout_streak >= 5 {
                    anyhow::bail!(
                        "扫描分区超时，已读到 offset {}，目标结束 offset {}",
                        last_offset,
                        final_offset
                    );
                }
            }
        }
    }

    Ok(SearchResult {
        matches,
        scanned_messages,
        low_watermark: low,
        high_watermark: high,
    })
}

/// 搜索指定 topic 的所有 partition 中仍保留的消息
pub async fn search_topic(
    connection_config: &ConnectionConfig,
    topic: &str,
    partitions: &[i32],
    query: &str,
    decoder: &DecoderPipeline,
) -> Result<SearchResult> {
    if partitions.is_empty() {
        return Ok(SearchResult {
            matches: Vec::new(),
            scanned_messages: 0,
            low_watermark: 0,
            high_watermark: 0,
        });
    }

    let mut scanned_messages = 0usize;
    let mut matches = Vec::new();
    let mut low_watermark = i64::MAX;
    let mut high_watermark = i64::MIN;

    for &partition in partitions {
        let result = search_partition(connection_config, topic, partition, query, decoder)
            .await
            .with_context(|| format!("搜索 Topic {topic} 的 Partition {partition} 失败"))?;

        scanned_messages += result.scanned_messages;
        matches.extend(result.matches);
        low_watermark = low_watermark.min(result.low_watermark);
        high_watermark = high_watermark.max(result.high_watermark);
    }

    matches.sort_by(|left, right| {
        right
            .raw
            .timestamp
            .cmp(&left.raw.timestamp)
            .then_with(|| left.raw.partition.cmp(&right.raw.partition))
            .then_with(|| right.raw.offset.cmp(&left.raw.offset))
    });

    Ok(SearchResult {
        matches,
        scanned_messages,
        low_watermark: if low_watermark == i64::MAX {
            0
        } else {
            low_watermark
        },
        high_watermark: if high_watermark == i64::MIN {
            0
        } else {
            high_watermark
        },
    })
}
