pub mod avro;
pub mod encoding;
pub mod json;
pub mod msgpack;
pub mod plugin;
pub mod protobuf;

use crate::kafka::types::{DecodedMessage, DecodedPayload, KafkaMessage};

/// 解码器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoderType {
    Auto,
    Json,
    Text,
    Protobuf,
    Avro,
    MsgPack,
    Binary,
}

impl DecoderType {
    pub fn label(&self) -> &'static str {
        match self {
            DecoderType::Auto => "Auto",
            DecoderType::Json => "JSON",
            DecoderType::Text => "Text",
            DecoderType::Protobuf => "Protobuf",
            DecoderType::Avro => "Avro",
            DecoderType::MsgPack => "MsgPack",
            DecoderType::Binary => "Binary",
        }
    }

    pub const ALL: &'static [DecoderType] = &[
        DecoderType::Auto,
        DecoderType::Json,
        DecoderType::Text,
        DecoderType::Protobuf,
        DecoderType::Avro,
        DecoderType::MsgPack,
        DecoderType::Binary,
    ];
}

/// 解码管线：按优先级依次尝试各解码器
#[derive(Debug, Clone)]
pub struct DecoderPipeline {
    pub selected: DecoderType,
}

impl Default for DecoderPipeline {
    fn default() -> Self {
        Self {
            selected: DecoderType::Auto,
        }
    }
}

impl DecoderPipeline {
    pub fn new(selected: DecoderType) -> Self {
        Self { selected }
    }

    /// 解码单条消息
    pub fn decode(&self, msg: KafkaMessage) -> DecodedMessage {
        let decoded_key = msg.key.as_ref().map(|k| encoding::detect_and_decode(k));

        let decoded_value = match &msg.payload {
            None => DecodedPayload::Text("<null>".to_string()),
            Some(data) if data.is_empty() => DecodedPayload::Text("<empty>".to_string()),
            Some(data) => self.decode_payload(data),
        };

        DecodedMessage {
            raw: msg,
            decoded_key,
            decoded_value,
        }
    }

    /// 批量解码
    pub fn decode_batch(&self, messages: Vec<KafkaMessage>) -> Vec<DecodedMessage> {
        messages.into_iter().map(|msg| self.decode(msg)).collect()
    }

    fn decode_payload(&self, data: &[u8]) -> DecodedPayload {
        match self.selected {
            DecoderType::Auto => self.auto_detect(data),
            DecoderType::Json => json::decode_json(data),
            DecoderType::Text => DecodedPayload::Text(encoding::detect_and_decode(data)),
            DecoderType::Binary => DecodedPayload::Binary(data.to_vec()),
            DecoderType::Protobuf => protobuf::decode_protobuf(data),
            DecoderType::Avro => avro::decode_avro(data),
            DecoderType::MsgPack => msgpack::decode_msgpack(data),
        }
    }

    fn auto_detect(&self, data: &[u8]) -> DecodedPayload {
        // 1. 尝试 JSON
        if let DecodedPayload::Json(v) = json::decode_json(data) {
            return DecodedPayload::Json(v);
        }

        // 2. 尝试 MsgPack（以特定字节开头）
        if let Some(first) = data.first() {
            if matches!(first, 0x80..=0x9f | 0xdc..=0xdf) {
                if let DecodedPayload::MsgPack(v) = msgpack::decode_msgpack(data) {
                    return DecodedPayload::MsgPack(v);
                }
            }
        }

        // 3. Avro 容器检测
        if data.len() >= 4 && &data[0..4] == b"Obj\x01" {
            if let DecodedPayload::Avro(s) = avro::decode_avro(data) {
                return DecodedPayload::Avro(s);
            }
        }

        // 4. 尝试文本（编码检测）
        let text = encoding::detect_and_decode(data);
        if text
            .chars()
            .all(|c| !c.is_control() || c == '\n' || c == '\r' || c == '\t')
        {
            return DecodedPayload::Text(text);
        }

        // 5. 尝试 Protobuf 启发式解码
        if let DecodedPayload::Protobuf(s) = protobuf::decode_protobuf(data) {
            return DecodedPayload::Protobuf(s);
        }

        // 6. 回退到二进制
        DecodedPayload::Binary(data.to_vec())
    }
}
