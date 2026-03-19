use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::TopicPartitionList;

use crate::kafka::types::{KafkaMessage, OffsetRange, PageData};
use crate::codec::DecoderPipeline;

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
    tpl.add_partition_offset(
        topic,
        partition,
        rdkafka::Offset::Offset(actual_offset),
    )?;
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
