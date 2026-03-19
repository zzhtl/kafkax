use crate::kafka::types::DecodedPayload;

/// 尝试将 bytes 解码为 Protobuf（无描述符时尝试启发式解码）
pub fn decode_protobuf(data: &[u8]) -> DecodedPayload {
    // 没有描述符文件时，尝试启发式解码 protobuf wire format
    match decode_wire_format(data) {
        Some(fields) => {
            let formatted = format_fields(&fields, 0);
            DecodedPayload::Protobuf(formatted)
        }
        None => DecodedPayload::Error("Protobuf 解码失败".to_string()),
    }
}

/// Protobuf wire type
#[derive(Debug)]
enum WireValue {
    Varint(u64),
    Fixed64(u64),
    LengthDelimited(Vec<u8>),
    Fixed32(u32),
}

#[derive(Debug)]
struct Field {
    field_number: u32,
    value: WireValue,
}

fn decode_wire_format(data: &[u8]) -> Option<Vec<Field>> {
    let mut fields = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let (tag, new_pos) = decode_varint(data, pos)?;
        pos = new_pos;

        let field_number = (tag >> 3) as u32;
        let wire_type = tag & 0x07;

        if field_number == 0 {
            return None;
        }

        let value = match wire_type {
            0 => {
                // Varint
                let (val, new_pos) = decode_varint(data, pos)?;
                pos = new_pos;
                WireValue::Varint(val)
            }
            1 => {
                // 64-bit
                if pos + 8 > data.len() {
                    return None;
                }
                let val = u64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
                pos += 8;
                WireValue::Fixed64(val)
            }
            2 => {
                // Length-delimited
                let (len, new_pos) = decode_varint(data, pos)?;
                pos = new_pos;
                let len = len as usize;
                if pos + len > data.len() {
                    return None;
                }
                let val = data[pos..pos + len].to_vec();
                pos += len;
                WireValue::LengthDelimited(val)
            }
            5 => {
                // 32-bit
                if pos + 4 > data.len() {
                    return None;
                }
                let val = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
                pos += 4;
                WireValue::Fixed32(val)
            }
            _ => return None,
        };

        fields.push(Field {
            field_number,
            value,
        });
    }

    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
}

fn decode_varint(data: &[u8], mut pos: usize) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;

    loop {
        if pos >= data.len() || shift >= 70 {
            return None;
        }
        let byte = data[pos];
        pos += 1;
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some((result, pos));
        }
        shift += 7;
    }
}

fn format_fields(fields: &[Field], indent: usize) -> String {
    let prefix = "  ".repeat(indent);
    let mut result = String::new();

    for field in fields {
        result.push_str(&prefix);
        result.push_str(&format!("field_{}: ", field.field_number));

        match &field.value {
            WireValue::Varint(v) => {
                result.push_str(&format!("{}", v));
            }
            WireValue::Fixed64(v) => {
                // 尝试解释为 double
                let f = f64::from_bits(*v);
                if f.is_finite() && f.abs() < 1e15 {
                    result.push_str(&format!("{} (double: {})", v, f));
                } else {
                    result.push_str(&format!("{}", v));
                }
            }
            WireValue::Fixed32(v) => {
                let f = f32::from_bits(*v);
                if f.is_finite() && f.abs() < 1e10 {
                    result.push_str(&format!("{} (float: {})", v, f));
                } else {
                    result.push_str(&format!("{}", v));
                }
            }
            WireValue::LengthDelimited(data) => {
                // 尝试解释为 UTF-8 字符串
                if let Ok(s) = std::str::from_utf8(data) {
                    if s.chars().all(|c| !c.is_control() || c == '\n' || c == '\r' || c == '\t') {
                        result.push_str(&format!("\"{}\"", s));
                    } else {
                        // 尝试解释为嵌套消息
                        if let Some(nested) = decode_wire_format(data) {
                            result.push_str("{\n");
                            result.push_str(&format_fields(&nested, indent + 1));
                            result.push_str(&prefix);
                            result.push('}');
                        } else {
                            result.push_str(&format!("<{} bytes>", data.len()));
                        }
                    }
                } else {
                    // 尝试解释为嵌套消息
                    if let Some(nested) = decode_wire_format(data) {
                        result.push_str("{\n");
                        result.push_str(&format_fields(&nested, indent + 1));
                        result.push_str(&prefix);
                        result.push('}');
                    } else {
                        result.push_str(&format!("<{} bytes>", data.len()));
                    }
                }
            }
        }
        result.push('\n');
    }

    result
}
