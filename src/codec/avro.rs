use crate::kafka::types::DecodedPayload;

/// 尝试将 bytes 解码为 Avro
pub fn decode_avro(data: &[u8]) -> DecodedPayload {
    // Avro 文件以 magic bytes "Obj\x01" 开头
    if data.len() >= 4 && &data[0..4] == b"Obj\x01" {
        match decode_avro_container(data) {
            Ok(text) => DecodedPayload::Avro(text),
            Err(e) => DecodedPayload::Error(format!("Avro 解码失败: {}", e)),
        }
    } else {
        // 可能是单条 Avro 记录（没有容器头），需要 schema
        DecodedPayload::Error("Avro 解码需要 Schema 或容器文件格式".to_string())
    }
}

fn decode_avro_container(data: &[u8]) -> Result<String, String> {
    use apache_avro::Reader;
    use std::io::Cursor;

    let cursor = Cursor::new(data);
    let reader = Reader::new(cursor).map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for value in reader {
        match value {
            Ok(v) => {
                results.push(format!("{:?}", v));
            }
            Err(e) => {
                results.push(format!("[Error: {}]", e));
            }
        }
    }

    if results.is_empty() {
        Ok("(空 Avro 容器)".to_string())
    } else {
        Ok(results.join("\n---\n"))
    }
}
