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
    /// 获取用于表格显示的摘要（截断到指定长度）
    pub fn summary(&self, max_len: usize) -> String {
        let full = match self {
            DecodedPayload::Json(v) => serde_json::to_string(v).unwrap_or_default(),
            DecodedPayload::Text(s) => s.clone(),
            DecodedPayload::Protobuf(s) => s.clone(),
            DecodedPayload::Avro(s) => s.clone(),
            DecodedPayload::MsgPack(v) => serde_json::to_string(v).unwrap_or_default(),
            DecodedPayload::Binary(b) => b
                .iter()
                .take(max_len / 3)
                .map(|byte| format!("{:02X}", byte))
                .collect::<Vec<_>>()
                .join(" "),
            DecodedPayload::Error(e) => format!("[Error] {}", e),
        };
        if full.len() > max_len {
            format!("{}...", &full[..max_len])
        } else {
            full
        }
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

        let query_lower = query.to_lowercase();
        let key_matches = self
            .decoded_key
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains(&query_lower);
        let value_matches = self
            .decoded_value
            .full_display()
            .to_lowercase()
            .contains(&query_lower);
        let offset_matches = self.raw.offset.to_string().contains(query);

        key_matches || value_matches || offset_matches
    }
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
    pub messages: Vec<DecodedMessage>,
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

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use serde_json::json;

    use super::{DecodedMessage, DecodedPayload, KafkaMessage};

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
}
