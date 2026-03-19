use iced::widget::text::Span;
use iced::Color;

use crate::theme::AppColors;

/// JSON 语法高亮：将 JSON 字符串转换为带颜色的 Span 列表
pub fn highlight_json(json_str: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let chars: Vec<char> = json_str.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];
        match ch {
            // 字符串
            '"' => {
                let start = i;
                i += 1;
                let mut escaped = false;
                while i < len {
                    if escaped {
                        escaped = false;
                        i += 1;
                        continue;
                    }
                    if chars[i] == '\\' {
                        escaped = true;
                        i += 1;
                        continue;
                    }
                    if chars[i] == '"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();

                // 判断是 key 还是 value：如果后面紧跟 : 则是 key
                let is_key = {
                    let mut j = i;
                    while j < len && chars[j].is_whitespace() {
                        j += 1;
                    }
                    j < len && chars[j] == ':'
                };

                let color = if is_key {
                    AppColors::JSON_KEY
                } else {
                    AppColors::JSON_STRING
                };

                spans.push(Span::new(s).color(color));
            }
            // 数字
            c if c == '-' || c.is_ascii_digit() => {
                let start = i;
                if c == '-' {
                    i += 1;
                }
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == 'e' || chars[i] == 'E' || chars[i] == '+' || chars[i] == '-') {
                    // 避免 '-' 不是数字的情况
                    if i > start + 1 && (chars[i] == '-' || chars[i] == '+') && chars[i-1] != 'e' && chars[i-1] != 'E' {
                        break;
                    }
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                spans.push(Span::new(s).color(AppColors::JSON_NUMBER));
            }
            // true/false
            't' if i + 4 <= len && chars[i..i+4].iter().collect::<String>() == "true" => {
                spans.push(Span::new("true".to_string()).color(AppColors::JSON_BOOL));
                i += 4;
            }
            'f' if i + 5 <= len && chars[i..i+5].iter().collect::<String>() == "false" => {
                spans.push(Span::new("false".to_string()).color(AppColors::JSON_BOOL));
                i += 5;
            }
            // null
            'n' if i + 4 <= len && chars[i..i+4].iter().collect::<String>() == "null" => {
                spans.push(Span::new("null".to_string()).color(AppColors::JSON_NULL));
                i += 4;
            }
            // 结构字符和空白
            '{' | '}' | '[' | ']' | ':' | ',' => {
                spans.push(Span::new(ch.to_string()).color(AppColors::TEXT_SECONDARY));
                i += 1;
            }
            // 空白字符
            _ if ch.is_whitespace() => {
                let start = i;
                while i < len && chars[i].is_whitespace() {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                spans.push(Span::new(s));
            }
            // 其他字符
            _ => {
                spans.push(Span::new(ch.to_string()).color(AppColors::TEXT_PRIMARY));
                i += 1;
            }
        }
    }

    spans
}

/// 搜索高亮：在文本中高亮匹配的关键词
pub fn highlight_search(text_str: &str, query: &str) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::new(text_str.to_string()).color(AppColors::TEXT_PRIMARY)];
    }

    let query_lower = query.to_lowercase();
    let text_lower = text_str.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in text_lower.match_indices(&query_lower) {
        // 匹配前的普通文本
        if start > last_end {
            spans.push(
                Span::new(text_str[last_end..start].to_string())
                    .color(AppColors::TEXT_PRIMARY),
            );
        }
        // 高亮匹配部分
        let end = start + query.len();
        spans.push(
            Span::new(text_str[start..end].to_string())
                .color(Color::BLACK)
                .background(AppColors::WARNING),
        );
        last_end = end;
    }

    // 剩余文本
    if last_end < text_str.len() {
        spans.push(
            Span::new(text_str[last_end..].to_string())
                .color(AppColors::TEXT_PRIMARY),
        );
    }

    if spans.is_empty() {
        spans.push(Span::new(text_str.to_string()).color(AppColors::TEXT_PRIMARY));
    }

    spans
}
