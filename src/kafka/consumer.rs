use std::ffi::{CStr, CString};
use std::ptr;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rdkafka::bindings as rdsys;
use rdkafka::consumer::{BaseConsumer, Consumer};

use crate::codec::DecoderPipeline;
use crate::config::ConnectionConfig;
use crate::kafka::connection;
use crate::kafka::types::{
    KafkaMessage, MessageSummary, OffsetRange, PageData, SearchResult, SortOrder,
    sort_search_matches,
};

/// 从指定 partition 的 offset 开始消费一页数据
pub fn fetch_page(
    connection_config: &ConnectionConfig,
    topic: &str,
    partition: i32,
    page: usize,
    page_size: usize,
    sort_order: SortOrder,
    decoder: &DecoderPipeline,
) -> Result<PageData> {
    let consumer = connection::create_consumer(connection_config)?;

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

    let actual_offset = page_start_offset(low, high, page, page_size, sort_order);

    let mut simple_consumer =
        SimplePartitionConsumer::start(&consumer, topic, partition, actual_offset)?;

    // 消费指定数量的消息
    let mut raw_messages = Vec::with_capacity(page_size);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);

    while raw_messages.len() < page_size {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        match simple_consumer.poll(remaining.min(Duration::from_millis(500)))? {
            SimplePoll::Message(message) => raw_messages.push(message),
            SimplePoll::PartitionEof => break,
            SimplePoll::Timeout => {
                // 超时，可能已经没有更多消息了
                if raw_messages.is_empty() {
                    continue;
                }
                break;
            }
        }
    }

    // 批量解码
    let mut decoded = decoder.decode_batch(raw_messages);
    sort_search_matches(&mut decoded, sort_order);

    let total = high - low;

    let summaries: Vec<MessageSummary> = decoded
        .into_iter()
        .map(|msg| msg.into_summary(""))
        .collect();

    Ok(PageData {
        messages: summaries,
        page,
        total_messages: total,
        low_watermark: low,
        high_watermark: high,
    })
}

/// 获取 offset 范围
pub fn get_offset_range(
    consumer: &BaseConsumer,
    topic: &str,
    partition: i32,
) -> Result<OffsetRange> {
    let (low, high) = consumer.fetch_watermarks(topic, partition, Duration::from_secs(5))?;
    Ok(OffsetRange { low, high })
}

/// 搜索指定 partition 中所有仍保留的消息
pub fn search_partition(
    connection_config: &ConnectionConfig,
    topic: &str,
    partition: i32,
    query: &str,
    decoder: &DecoderPipeline,
) -> Result<SearchResult> {
    let consumer = connection::create_consumer(connection_config)?;
    let (low, high) = consumer.fetch_watermarks(topic, partition, Duration::from_secs(5))?;

    if high <= low {
        return Ok(SearchResult {
            matches: Vec::new(),
            scanned_messages: 0,
            low_watermark: low,
            high_watermark: high,
        });
    }

    let mut simple_consumer = SimplePartitionConsumer::start(&consumer, topic, partition, low)?;

    let mut scanned_messages = 0usize;
    let mut matches = Vec::new();
    let mut timeout_streak = 0usize;
    let final_offset = high.saturating_sub(1);
    let mut last_offset = low.saturating_sub(1);

    while last_offset < final_offset {
        match simple_consumer.poll(Duration::from_secs(2))? {
            SimplePoll::Message(kafka_message) => {
                timeout_streak = 0;
                last_offset = kafka_message.offset;
                scanned_messages += 1;

                let decoded = decoder.decode(kafka_message);
                if decoded.matches_query(query) {
                    matches.push(decoded);
                }
            }
            SimplePoll::PartitionEof => break,
            SimplePoll::Timeout => {
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

enum SimplePoll {
    Message(KafkaMessage),
    Timeout,
    PartitionEof,
}

struct SimplePartitionConsumer<'a> {
    consumer: &'a BaseConsumer,
    topic_handle: *mut rdsys::rd_kafka_topic_t,
    topic: String,
    partition: i32,
    started: bool,
}

impl<'a> SimplePartitionConsumer<'a> {
    fn start(consumer: &'a BaseConsumer, topic: &str, partition: i32, offset: i64) -> Result<Self> {
        let topic_name =
            CString::new(topic).with_context(|| format!("Topic 名称包含非法字符: {topic}"))?;
        let topic_handle = unsafe {
            rdsys::rd_kafka_topic_new(
                consumer.client().native_ptr(),
                topic_name.as_ptr(),
                ptr::null_mut(),
            )
        };

        if topic_handle.is_null() {
            anyhow::bail!("创建 Topic 句柄失败: {topic}");
        }

        let mut this = Self {
            consumer,
            topic_handle,
            topic: topic.to_string(),
            partition,
            started: false,
        };

        let start_result =
            unsafe { rdsys::rd_kafka_consume_start(this.topic_handle, partition, offset) };
        if start_result == -1 {
            let os_error = std::io::Error::last_os_error();
            anyhow::bail!(
                "启动分区读取失败: topic={}, partition={}, offset={}, errno={}",
                topic,
                partition,
                offset,
                os_error
            );
        }

        this.started = true;
        Ok(this)
    }

    fn poll(&mut self, timeout: Duration) -> Result<SimplePoll> {
        let timeout_ms = timeout
            .as_millis()
            .min(i32::MAX as u128)
            .try_into()
            .unwrap_or(i32::MAX);
        let message_ptr =
            unsafe { rdsys::rd_kafka_consume(self.topic_handle, self.partition, timeout_ms) };

        if message_ptr.is_null() {
            let os_error = std::io::Error::last_os_error();
            if os_error.kind() == std::io::ErrorKind::TimedOut {
                return Ok(SimplePoll::Timeout);
            }

            anyhow::bail!(
                "读取分区消息失败: topic={}, partition={}, errno={}",
                self.topic,
                self.partition,
                os_error
            );
        }

        let message = SimpleMessage::new(message_ptr);
        let err = unsafe { (*message.ptr).err };
        if err != rdsys::rd_kafka_resp_err_t::RD_KAFKA_RESP_ERR_NO_ERROR {
            if err == rdsys::rd_kafka_resp_err_t::RD_KAFKA_RESP_ERR__PARTITION_EOF {
                return Ok(SimplePoll::PartitionEof);
            }

            let err_str = unsafe {
                CStr::from_ptr(rdsys::rd_kafka_message_errstr(message.ptr))
                    .to_string_lossy()
                    .into_owned()
            };
            anyhow::bail!(
                "读取分区消息失败: topic={}, partition={}, err={}",
                self.topic,
                self.partition,
                err_str
            );
        }

        Ok(SimplePoll::Message(message.to_owned_message(&self.topic)))
    }
}

impl Drop for SimplePartitionConsumer<'_> {
    fn drop(&mut self) {
        if self.started {
            let stop_result =
                unsafe { rdsys::rd_kafka_consume_stop(self.topic_handle, self.partition) };
            if stop_result == -1 {
                let os_error = std::io::Error::last_os_error();
                tracing::warn!(
                    "停止分区读取失败: topic={}, partition={}, errno={}",
                    self.topic,
                    self.partition,
                    os_error
                );
            }
        }

        unsafe {
            rdsys::rd_kafka_topic_destroy(self.topic_handle);
            rdsys::rd_kafka_poll(self.consumer.client().native_ptr(), 0);
        }
    }
}

struct SimpleMessage {
    ptr: *mut rdsys::rd_kafka_message_t,
}

impl SimpleMessage {
    fn new(ptr: *mut rdsys::rd_kafka_message_t) -> Self {
        Self { ptr }
    }

    fn to_owned_message(&self, topic: &str) -> KafkaMessage {
        let message = unsafe { &*self.ptr };
        let mut timestamp_type = rdsys::rd_kafka_timestamp_type_t::RD_KAFKA_TIMESTAMP_NOT_AVAILABLE;
        let timestamp_ms =
            unsafe { rdsys::rd_kafka_message_timestamp(self.ptr, &mut timestamp_type) };

        KafkaMessage {
            topic: topic.to_string(),
            partition: message.partition,
            offset: message.offset,
            timestamp: if timestamp_ms >= 0 {
                DateTime::from_timestamp_millis(timestamp_ms).map(|dt| dt.with_timezone(&Utc))
            } else {
                None
            },
            key: bytes_from_ptr(message.key, message.key_len),
            payload: bytes_from_ptr(message.payload, message.len),
        }
    }
}

impl Drop for SimpleMessage {
    fn drop(&mut self) {
        unsafe {
            rdsys::rd_kafka_message_destroy(self.ptr);
        }
    }
}

fn bytes_from_ptr(ptr: *mut std::ffi::c_void, len: usize) -> Option<Vec<u8>> {
    if ptr.is_null() {
        return None;
    }

    let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) };
    Some(bytes.to_vec())
}

/// 搜索指定 topic 的所有 partition 中仍保留的消息
pub fn search_topic(
    connection_config: &ConnectionConfig,
    topic: &str,
    partitions: &[i32],
    query: &str,
    sort_order: SortOrder,
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
            .with_context(|| format!("搜索 Topic {topic} 的 Partition {partition} 失败"))?;

        scanned_messages += result.scanned_messages;
        matches.extend(result.matches);
        low_watermark = low_watermark.min(result.low_watermark);
        high_watermark = high_watermark.max(result.high_watermark);
    }

    sort_search_matches(&mut matches, sort_order);

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

fn page_start_offset(
    low_watermark: i64,
    high_watermark: i64,
    page: usize,
    page_size: usize,
    sort_order: SortOrder,
) -> i64 {
    if high_watermark <= low_watermark {
        return low_watermark;
    }

    let page_size = page_size.max(1) as i64;
    let highest_offset = high_watermark.saturating_sub(1);

    match sort_order {
        SortOrder::Asc => (low_watermark + page as i64 * page_size).min(highest_offset),
        SortOrder::Desc => {
            let page_span = (page as i64 + 1) * page_size;
            (high_watermark - page_span)
                .max(low_watermark)
                .min(highest_offset)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::page_start_offset;
    use crate::kafka::types::SortOrder;

    #[test]
    fn descending_page_starts_from_latest_offsets() {
        assert_eq!(page_start_offset(0, 250, 0, 100, SortOrder::Desc), 150);
        assert_eq!(page_start_offset(0, 250, 1, 100, SortOrder::Desc), 50);
        assert_eq!(page_start_offset(0, 250, 2, 100, SortOrder::Desc), 0);
    }

    #[test]
    fn ascending_page_starts_from_low_watermark() {
        assert_eq!(page_start_offset(20, 250, 0, 100, SortOrder::Asc), 20);
        assert_eq!(page_start_offset(20, 250, 1, 100, SortOrder::Asc), 120);
        assert_eq!(page_start_offset(20, 250, 2, 100, SortOrder::Asc), 220);
    }
}
