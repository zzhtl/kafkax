use chardetng::EncodingDetector;
use encoding_rs::Encoding;

/// 检测字节数据的编码并转换为 UTF-8 字符串
pub fn detect_and_decode(data: &[u8]) -> String {
    // 快速路径：如果是合法的 UTF-8 直接返回
    if let Ok(s) = std::str::from_utf8(data) {
        return s.to_string();
    }

    // 使用 chardetng 检测编码
    let mut detector = EncodingDetector::new();
    detector.feed(data, true);
    let encoding = detector.guess(None, true);

    decode_with_encoding(data, encoding)
}

/// 使用指定编码解码
fn decode_with_encoding(data: &[u8], encoding: &'static Encoding) -> String {
    let (result, _, _) = encoding.decode(data);
    result.into_owned()
}
