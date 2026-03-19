use crate::kafka::types::DecodedPayload;

/// 尝试将 bytes 解码为 JSON
pub fn decode_json(data: &[u8]) -> DecodedPayload {
    // 先尝试 UTF-8 解析
    if let Ok(text) = std::str::from_utf8(data) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text.trim()) {
            return DecodedPayload::Json(value);
        }
    }

    // 尝试其他编码转 UTF-8 后解析
    let text = super::encoding::detect_and_decode(data);
    match serde_json::from_str::<serde_json::Value>(text.trim()) {
        Ok(value) => DecodedPayload::Json(value),
        Err(_) => DecodedPayload::Error("非 JSON 格式".to_string()),
    }
}

/// 格式化 JSON 为带缩进的字符串
pub fn pretty_print(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}
