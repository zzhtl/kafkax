use std::cmp::Ordering;
use std::fmt::Write as _;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// 原始 Kafka 消息
#[derive(Debug, Clone)]
pub struct KafkaMessage {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub timestamp: Option<DateTime<Utc>>,
    pub key: Option<Vec<u8>>,
    pub payload: Option<Vec<u8>>,
}

/// 消息排序方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    Asc,
    #[default]
    Desc,
}

impl SortOrder {
    pub const ALL: [Self; 2] = [Self::Desc, Self::Asc];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Asc => "正序",
            Self::Desc => "倒序",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }

    pub fn indicator(self) -> &'static str {
        match self {
            Self::Asc => "↑",
            Self::Desc => "↓",
        }
    }
}

/// 解码后的负载
#[derive(Debug, Clone)]
pub enum DecodedPayload {
    Json(serde_json::Value),
    Text(String),
    Protobuf(String),
    Avro(String),
    MsgPack(serde_json::Value),
    Binary(Vec<u8>),
    Error(String),
}

impl DecodedPayload {
    /// 获取预览文本（截断到指定长度）
    pub fn preview(&self, max_len: usize) -> String {
        match self {
            DecodedPayload::Json(v) | DecodedPayload::MsgPack(v) => json_preview(v, max_len),
            DecodedPayload::Text(s) | DecodedPayload::Protobuf(s) | DecodedPayload::Avro(s) => {
                truncate_text(s, max_len)
            }
            DecodedPayload::Binary(b) => hex_preview(b, max_len),
            DecodedPayload::Error(e) => truncate_text(&format!("[Error] {e}"), max_len),
        }
    }

    /// 获取用于表格显示的摘要（截断到指定长度）
    pub fn summary(&self, max_len: usize) -> String {
        self.preview(max_len)
    }

    /// 获取完整的格式化字符串（用于详情面板）
    pub fn full_display(&self) -> String {
        match self {
            DecodedPayload::Json(v) => serde_json::to_string_pretty(v).unwrap_or_default(),
            DecodedPayload::Text(s) => s.clone(),
            DecodedPayload::Protobuf(s) => s.clone(),
            DecodedPayload::Avro(s) => s.clone(),
            DecodedPayload::MsgPack(v) => serde_json::to_string_pretty(v).unwrap_or_default(),
            DecodedPayload::Binary(b) => hex_dump(b),
            DecodedPayload::Error(e) => format!("[Error] {}", e),
        }
    }

    /// 获取格式名称
    pub fn format_name(&self) -> &'static str {
        match self {
            DecodedPayload::Json(_) => "JSON",
            DecodedPayload::Text(_) => "Text",
            DecodedPayload::Protobuf(_) => "Protobuf",
            DecodedPayload::Avro(_) => "Avro",
            DecodedPayload::MsgPack(_) => "MsgPack",
            DecodedPayload::Binary(_) => "Binary",
            DecodedPayload::Error(_) => "Error",
        }
    }

    /// 返回适合放入 JSON 的值
    pub fn json_value(&self) -> serde_json::Value {
        match self {
            DecodedPayload::Json(value) | DecodedPayload::MsgPack(value) => value.clone(),
            DecodedPayload::Text(value)
            | DecodedPayload::Protobuf(value)
            | DecodedPayload::Avro(value)
            | DecodedPayload::Error(value) => serde_json::Value::String(value.clone()),
            DecodedPayload::Binary(_) => serde_json::Value::String(self.full_display()),
        }
    }

    /// 判断当前负载是否命中搜索词，尽量避免为大消息构造整段显示文本
    pub fn contains_query(&self, query: &str) -> bool {
        let query = query.trim();
        if query.is_empty() {
            return true;
        }

        match self {
            DecodedPayload::Json(value) | DecodedPayload::MsgPack(value) => {
                json_contains_query(value, query)
            }
            DecodedPayload::Text(value)
            | DecodedPayload::Protobuf(value)
            | DecodedPayload::Avro(value)
            | DecodedPayload::Error(value) => text_contains_query(value, query),
            DecodedPayload::Binary(bytes) => binary_contains_query(bytes, query),
        }
    }
}

/// 解码后的消息
#[derive(Debug, Clone)]
pub struct DecodedMessage {
    pub raw: KafkaMessage,
    pub decoded_key: Option<String>,
    pub decoded_value: DecodedPayload,
}

impl DecodedMessage {
    /// 返回可直接复制的 Key 文本
    pub fn copyable_key(&self) -> Option<String> {
        self.decoded_key
            .as_ref()
            .map(|key| key.trim_end_matches('\0').to_string())
    }

    /// 返回可直接复制的 Value 文本
    pub fn copyable_value(&self) -> String {
        self.decoded_value.full_display()
    }

    /// 返回包含元数据的完整消息文本
    pub fn copyable_message(&self) -> String {
        let timestamp = self
            .raw
            .timestamp
            .map(|timestamp| timestamp.to_rfc3339())
            .unwrap_or_else(|| "-".to_string());
        let key = self
            .copyable_key()
            .filter(|key| !key.is_empty())
            .unwrap_or_else(|| "<empty>".to_string());

        format!(
            "Topic: {}\nPartition: {}\nOffset: {}\nTimestamp: {}\nKey:\n{}\n\nValue [{}]:\n{}",
            self.raw.topic,
            self.raw.partition,
            self.raw.offset,
            timestamp,
            key,
            self.decoded_value.format_name(),
            self.copyable_value(),
        )
    }

    /// 返回完整消息的 JSON 文本
    pub fn copyable_json(&self) -> String {
        let json = json!({
            "topic": self.raw.topic,
            "partition": self.raw.partition,
            "offset": self.raw.offset,
            "timestamp": self.raw.timestamp.map(|timestamp| timestamp.to_rfc3339()),
            "key": self.copyable_key(),
            "value_format": self.decoded_value.format_name(),
            "value": self.decoded_value.json_value(),
        });

        serde_json::to_string_pretty(&json).unwrap_or_default()
    }

    /// 判断消息是否匹配搜索词
    pub fn matches_query(&self, query: &str) -> bool {
        let query = query.trim();
        if query.is_empty() {
            return true;
        }

        let key_matches = text_contains_query(self.decoded_key.as_deref().unwrap_or(""), query);
        let value_matches = self.decoded_value.contains_query(query);
        let offset_matches = self.raw.offset.to_string().contains(query);

        key_matches || value_matches || offset_matches
    }

    /// 转换为轻量摘要，query 用于生成含关键词上下文的 value_label
    pub fn into_summary(self, query: &str) -> MessageSummary {
        let payload_size = self.raw.payload.as_ref().map_or(0, |p| p.len());

        let partition_label = format!("P-{}", self.raw.partition);
        let offset_label = self.raw.offset.to_string();
        let ts_label = self
            .raw
            .timestamp
            .map(|ts| ts.format("%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "-".to_string());

        let key_preview = self.decoded_key.as_ref().map(|k| {
            let s = k.trim_end_matches('\0');
            truncate_text(s, 200)
        });
        let key_label = key_preview.clone().unwrap_or_else(|| "-".to_string());

        // value_label：有 query 时取命中上下文，无 query 时取前缀
        let value_format = self.decoded_value.format_name();
        let raw_preview = self.decoded_value.preview(800);
        let value_label = if query.trim().is_empty() {
            truncate_text(&raw_preview, 200)
        } else {
            excerpt_around_query(&raw_preview, query, 200)
        };

        MessageSummary {
            topic: self.raw.topic,
            partition: self.raw.partition,
            offset: self.raw.offset,
            timestamp: self.raw.timestamp,
            payload_size,
            key_preview,
            value_format,
            partition_label,
            offset_label,
            ts_label,
            key_label,
            value_label,
        }
    }
}

/// 统一排序 Topic 搜索结果，优先按时间，再按分区和 offset 保持稳定可读
pub fn sort_search_matches(messages: &mut [DecodedMessage], order: SortOrder) {
    messages.sort_by(|left, right| search_ordering(left, right, order));
}

/// 对搜索摘要结果排序，与 sort_search_matches 逻辑一致
pub fn sort_summaries(summaries: &mut [MessageSummary], order: SortOrder) {
    summaries.sort_by(|a, b| {
        let by_partition = a.partition.cmp(&b.partition);
        let by_offset = a.offset.cmp(&b.offset);
        let by_timestamp = a.timestamp.cmp(&b.timestamp);
        match order {
            SortOrder::Asc => by_timestamp.then(by_partition).then(by_offset),
            SortOrder::Desc => by_timestamp
                .reverse()
                .then(by_partition)
                .then(by_offset.reverse()),
        }
    });
}

/// 全分区搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub matches: Vec<DecodedMessage>,
    pub scanned_messages: usize,
    pub low_watermark: i64,
    pub high_watermark: i64,
}

/// Topic 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMeta {
    pub name: String,
    pub partitions: Vec<PartitionMeta>,
}

/// Partition 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionMeta {
    pub id: i32,
    pub leader: i32,
    pub replicas: Vec<i32>,
    pub isr: Vec<i32>,
}

/// 分页数据
#[derive(Debug, Clone)]
pub struct PageData {
    pub messages: Vec<MessageSummary>,
    pub page: usize,
    pub total_messages: i64,
    pub low_watermark: i64,
    pub high_watermark: i64,
}

/// Offset 范围信息
#[derive(Debug, Clone)]
pub struct OffsetRange {
    pub low: i64,
    pub high: i64,
}

/// 生成 Hex dump 显示
fn hex_dump(data: &[u8]) -> String {
    let mut result = String::new();
    for (i, chunk) in data.chunks(16).enumerate() {
        // 地址
        result.push_str(&format!("{:08X}  ", i * 16));
        // Hex 值
        for (j, byte) in chunk.iter().enumerate() {
            result.push_str(&format!("{:02X} ", byte));
            if j == 7 {
                result.push(' ');
            }
        }
        // 填充不足 16 字节的行
        for j in chunk.len()..16 {
            result.push_str("   ");
            if j == 7 {
                result.push(' ');
            }
        }
        result.push_str(" |");
        // ASCII 表示
        for byte in chunk {
            if byte.is_ascii_graphic() || *byte == b' ' {
                result.push(*byte as char);
            } else {
                result.push('.');
            }
        }
        result.push_str("|\n");
    }
    result
}

fn hex_preview(data: &[u8], max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let byte_budget = max_chars.saturating_div(3).max(1);
    let mut preview = String::new();

    for (index, byte) in data.iter().take(byte_budget).enumerate() {
        if index > 0 {
            preview.push(' ');
        }
        let _ = write!(preview, "{byte:02X}");
    }

    if data.len() > byte_budget {
        preview.push_str("...");
    }

    preview
}

fn json_preview(value: &serde_json::Value, max_chars: usize) -> String {
    let mut writer = TruncatingWriter::new(max_chars);
    if serde_json::to_writer(&mut writer, value).is_err() {
        return String::new();
    }
    writer.finish()
}

fn json_contains_query(value: &serde_json::Value, query: &str) -> bool {
    let mut matcher = CaseInsensitiveMatcher::new(query);
    if serde_json::to_writer(&mut matcher, value).is_err() {
        return false;
    }
    matcher.found()
}

fn text_contains_query(text: &str, query: &str) -> bool {
    let mut matcher = CaseInsensitiveMatcher::new(query);
    matcher.push_text(text);
    matcher.found()
}

fn binary_contains_query(bytes: &[u8], query: &str) -> bool {
    let mut matcher = CaseInsensitiveMatcher::new(query);
    for chunk in bytes.chunks(16) {
        let mut line = String::new();
        for (index, byte) in chunk.iter().enumerate() {
            if index > 0 {
                line.push(' ');
            }
            let _ = write!(line, "{byte:02X}");
        }
        matcher.push_text(&line);
        if matcher.found() {
            return true;
        }
    }
    matcher.found()
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }

    let end = byte_index_at_char(text, max_chars);
    format!("{}...", &text[..end])
}

fn byte_index_at_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(text.len())
}

#[derive(Debug)]
struct TruncatingWriter {
    output: String,
    max_chars: usize,
    written_chars: usize,
    truncated: bool,
}

impl TruncatingWriter {
    fn new(max_chars: usize) -> Self {
        Self {
            output: String::new(),
            max_chars,
            written_chars: 0,
            truncated: false,
        }
    }

    fn finish(mut self) -> String {
        if self.truncated {
            self.output.push_str("...");
        }
        self.output
    }
}

impl io::Write for TruncatingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.max_chars == 0 || self.truncated {
            return Ok(buf.len());
        }

        let text = std::str::from_utf8(buf)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        for ch in text.chars() {
            if self.written_chars >= self.max_chars {
                self.truncated = true;
                return Ok(buf.len());
            }
            self.output.push(ch);
            self.written_chars += 1;
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
struct CaseInsensitiveMatcher {
    needle: String,
    overlap: String,
    found: bool,
}

impl CaseInsensitiveMatcher {
    fn new(query: &str) -> Self {
        Self {
            needle: query.trim().to_lowercase(),
            overlap: String::new(),
            found: false,
        }
    }

    fn found(&self) -> bool {
        self.needle.is_empty() || self.found
    }

    fn push_text(&mut self, text: &str) {
        if self.found() || text.is_empty() {
            return;
        }

        const CHUNK_BYTES: usize = 4096;

        let mut start = 0usize;
        while start < text.len() {
            let end = next_char_boundary(text, start, CHUNK_BYTES);
            self.push_chunk(&text[start..end]);
            if self.found() {
                return;
            }
            start = end;
        }
    }

    fn push_chunk(&mut self, chunk: &str) {
        if self.found() || chunk.is_empty() {
            return;
        }

        self.overlap.push_str(&chunk.to_lowercase());
        if self.overlap.contains(&self.needle) {
            self.found = true;
            return;
        }

        let keep = self.needle.chars().count().saturating_sub(1);
        if keep == 0 {
            self.overlap.clear();
        } else {
            self.overlap = tail_chars(&self.overlap, keep);
        }
    }
}

impl io::Write for CaseInsensitiveMatcher {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = std::str::from_utf8(buf)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        self.push_chunk(text);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn next_char_boundary(text: &str, start: usize, max_bytes: usize) -> usize {
    let tentative_end = (start + max_bytes).min(text.len());
    if tentative_end == text.len() || text.is_char_boundary(tentative_end) {
        return tentative_end;
    }

    let mut end = tentative_end;
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }

    if end == start {
        text[start..]
            .char_indices()
            .nth(1)
            .map(|(index, _)| start + index)
            .unwrap_or(text.len())
    } else {
        end
    }
}

fn tail_chars(text: &str, keep: usize) -> String {
    if keep == 0 {
        return String::new();
    }

    let total = text.chars().count();
    if total <= keep {
        return text.to_string();
    }

    let start = byte_index_at_char(text, total - keep);
    text[start..].to_string()
}

/// 轻量消息摘要——用于列表显示和搜索结果存储（< 1KB/条）
#[derive(Debug, Clone)]
pub struct MessageSummary {
    // 元数据（用于详情拉取和排序）
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub timestamp: Option<DateTime<Utc>>,
    pub payload_size: usize,

    // 解码摘要（最多 500 字节）
    pub key_preview: Option<String>,
    pub value_format: &'static str,

    // 预计算显示标签（view() 直接使用，零分配）
    pub partition_label: String,  // "P-0"
    pub offset_label: String,     // "12345"
    pub ts_label: String,         // "03-20 14:32:01" 或 "-"
    pub key_label: String,        // key 截断到 120 字符
    pub value_label: String,      // 含关键词上下文，截断到 200 字符
}

/// 双重扫描限制计数器（跨线程共享）
#[derive(Debug, Clone)]
pub struct ScanLimits {
    pub total_scanned: Arc<AtomicUsize>,
    pub total_bytes: Arc<AtomicUsize>,
    pub max_messages: usize,
    pub max_bytes: usize,
}

impl ScanLimits {
    pub fn new(max_messages: usize, max_bytes: usize) -> Self {
        Self {
            total_scanned: Arc::new(AtomicUsize::new(0)),
            total_bytes: Arc::new(AtomicUsize::new(0)),
            max_messages,
            max_bytes,
        }
    }

    /// 尝试记录一条消息，返回 false 表示已超限，应停止扫描
    pub fn record(&self, payload_size: usize) -> bool {
        let prev_count = self.total_scanned.fetch_add(1, AtomicOrdering::Relaxed);
        let prev_bytes = self.total_bytes.fetch_add(payload_size, AtomicOrdering::Relaxed);
        prev_count < self.max_messages && prev_bytes < self.max_bytes
    }

    pub fn scanned(&self) -> usize {
        self.total_scanned.load(AtomicOrdering::Relaxed)
    }

    pub fn bytes(&self) -> usize {
        self.total_bytes.load(AtomicOrdering::Relaxed)
    }
}

/// 从文本中截取包含 query 的上下文片段（最多 max_chars 字符）
fn excerpt_around_query(text: &str, query: &str, max_chars: usize) -> String {
    let query_lower = query.trim().to_lowercase();
    let text_lower = text.to_lowercase();

    let Some(byte_pos) = text_lower.find(&query_lower) else {
        return truncate_text(text, max_chars);
    };

    let match_char = text[..byte_pos].chars().count();
    let query_char_len = query.chars().count();
    let context = max_chars.saturating_sub(query_char_len) / 2;
    let start_char = match_char.saturating_sub(context);
    let end_char = (match_char + query_char_len + context).min(text.chars().count());

    let start_byte = byte_index_at_char(text, start_char);
    let end_byte = byte_index_at_char(text, end_char);
    let snippet = &text[start_byte..end_byte];

    let mut result = String::new();
    if start_char > 0 {
        result.push_str("...");
    }
    result.push_str(snippet);
    if end_char < text.chars().count() {
        result.push_str("...");
    }
    result
}

fn search_ordering(left: &DecodedMessage, right: &DecodedMessage, order: SortOrder) -> Ordering {
    let by_partition = left.raw.partition.cmp(&right.raw.partition);
    let by_offset = left.raw.offset.cmp(&right.raw.offset);
    let by_timestamp = left.raw.timestamp.cmp(&right.raw.timestamp);

    match order {
        SortOrder::Asc => by_timestamp
            .then_with(|| by_partition)
            .then_with(|| by_offset),
        SortOrder::Desc => by_timestamp
            .reverse()
            .then_with(|| by_partition)
            .then_with(|| by_offset.reverse()),
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use serde_json::json;

    use super::{
        DecodedMessage, DecodedPayload, KafkaMessage, ScanLimits, SortOrder, sort_search_matches,
    };

    #[test]
    fn copyable_message_contains_metadata_and_payload() {
        let message = DecodedMessage {
            raw: KafkaMessage {
                topic: "orders".to_string(),
                partition: 3,
                offset: 42,
                timestamp: Some(chrono::Utc.with_ymd_and_hms(2026, 3, 19, 10, 0, 0).unwrap()),
                key: None,
                payload: None,
            },
            decoded_key: Some("order-1".to_string()),
            decoded_value: DecodedPayload::Text("{\"id\":1}".to_string()),
        };

        let copied = message.copyable_message();

        assert!(copied.contains("Topic: orders"));
        assert!(copied.contains("Partition: 3"));
        assert!(copied.contains("Offset: 42"));
        assert!(copied.contains("Key:\norder-1"));
        assert!(copied.contains("Value [Text]:\n{\"id\":1}"));
    }

    #[test]
    fn copyable_json_is_valid_json_document() {
        let message = DecodedMessage {
            raw: KafkaMessage {
                topic: "orders".to_string(),
                partition: 1,
                offset: 8,
                timestamp: None,
                key: None,
                payload: None,
            },
            decoded_key: Some("order-8".to_string()),
            decoded_value: DecodedPayload::Json(json!({"id": 8, "status": "ok"})),
        };

        let copied = message.copyable_json();
        let parsed: serde_json::Value = serde_json::from_str(&copied).unwrap();

        assert_eq!(parsed["topic"], "orders");
        assert_eq!(parsed["partition"], 1);
        assert_eq!(parsed["key"], "order-8");
        assert_eq!(parsed["value"]["id"], 8);
    }

    #[test]
    fn matches_query_checks_key_value_and_offset_case_insensitively() {
        let message = DecodedMessage {
            raw: KafkaMessage {
                topic: "orders".to_string(),
                partition: 1,
                offset: 128,
                timestamp: None,
                key: None,
                payload: None,
            },
            decoded_key: Some("Order-Created".to_string()),
            decoded_value: DecodedPayload::Text("{\"status\":\"PAID\"}".to_string()),
        };

        assert!(message.matches_query("created"));
        assert!(message.matches_query("paid"));
        assert!(message.matches_query("128"));
        assert!(!message.matches_query("missing"));
    }

    #[test]
    fn summary_truncates_unicode_on_char_boundary() {
        let payload = DecodedPayload::Text("中文连接名称测试".to_string());

        let summary = payload.summary(4);

        assert_eq!(summary, "中文连接...");
    }

    #[test]
    fn search_matches_can_sort_descending_or_ascending() {
        let mut messages = vec![
            DecodedMessage {
                raw: KafkaMessage {
                    topic: "orders".to_string(),
                    partition: 1,
                    offset: 3,
                    timestamp: Some(chrono::Utc.with_ymd_and_hms(2026, 3, 19, 10, 0, 3).unwrap()),
                    key: None,
                    payload: None,
                },
                decoded_key: None,
                decoded_value: DecodedPayload::Text("third".to_string()),
            },
            DecodedMessage {
                raw: KafkaMessage {
                    topic: "orders".to_string(),
                    partition: 0,
                    offset: 1,
                    timestamp: Some(chrono::Utc.with_ymd_and_hms(2026, 3, 19, 10, 0, 1).unwrap()),
                    key: None,
                    payload: None,
                },
                decoded_key: None,
                decoded_value: DecodedPayload::Text("first".to_string()),
            },
        ];

        sort_search_matches(&mut messages, SortOrder::Desc);
        assert_eq!(messages[0].raw.offset, 3);

        sort_search_matches(&mut messages, SortOrder::Asc);
        assert_eq!(messages[0].raw.offset, 1);
    }

    #[test]
    fn into_summary_computes_labels_correctly() {
        let msg = DecodedMessage {
            raw: KafkaMessage {
                topic: "orders".to_string(),
                partition: 2,
                offset: 9999,
                timestamp: Some(chrono::Utc.with_ymd_and_hms(2026, 3, 20, 10, 30, 0).unwrap()),
                key: Some(b"order-key".to_vec()),
                payload: Some(b"{\"id\":1}".to_vec()),
            },
            decoded_key: Some("order-key".to_string()),
            decoded_value: DecodedPayload::Json(serde_json::json!({"id": 1})),
        };

        let summary = msg.into_summary("");

        assert_eq!(summary.partition_label, "P-2");
        assert_eq!(summary.offset_label, "9999");
        assert_eq!(summary.key_label, "order-key");
        assert_eq!(summary.ts_label, "03-20 10:30:00");
        assert_eq!(summary.payload_size, 8);
        assert_eq!(summary.value_format, "JSON");
    }

    #[test]
    fn scan_limits_stops_at_count() {
        let limits = ScanLimits::new(3, usize::MAX);
        assert!(limits.record(10));
        assert!(limits.record(10));
        assert!(limits.record(10));
        assert!(!limits.record(10));
    }

    #[test]
    fn scan_limits_stops_at_bytes() {
        let limits = ScanLimits::new(usize::MAX, 100);
        assert!(limits.record(50));
        assert!(limits.record(50));
        assert!(!limits.record(1));
    }
}
