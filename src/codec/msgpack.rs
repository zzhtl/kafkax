use crate::kafka::types::DecodedPayload;

/// 尝试将 bytes 解码为 MsgPack
pub fn decode_msgpack(data: &[u8]) -> DecodedPayload {
    match rmp_serde::from_slice::<serde_json::Value>(data) {
        Ok(value) => DecodedPayload::MsgPack(value),
        Err(e) => DecodedPayload::Error(format!("MsgPack 解码失败: {}", e)),
    }
}
