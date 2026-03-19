use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
            DecodedPayload::Binary(b) => {
                b.iter().take(max_len / 3).map(|byte| format!("{:02X}", byte)).collect::<Vec<_>>().join(" ")
            }
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
}

/// 解码后的消息
#[derive(Debug, Clone)]
pub struct DecodedMessage {
    pub raw: KafkaMessage,
    pub decoded_key: Option<String>,
    pub decoded_value: DecodedPayload,
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
