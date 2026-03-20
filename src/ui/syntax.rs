use iced::widget::text::Span;
use iced::{Background, Border, Color};

use crate::theme::AppColors;

#[derive(Debug, Clone)]
struct StyledSegment {
    text: String,
    color: Color,
}

/// JSON 语法高亮，并可叠加关键词高亮
pub fn highlight_json(json_str: &str, query: &str) -> Vec<Span<'static>> {
    let mut segments = Vec::new();
    let chars: Vec<char> = json_str.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];
        match ch {
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

                let text: String = chars[start..i].iter().collect();
                let is_key = {
                    let mut j = i;
                    while j < len && chars[j].is_whitespace() {
                        j += 1;
                    }
                    j < len && chars[j] == ':'
                };

                segments.push(StyledSegment {
                    text,
                    color: if is_key {
                        AppColors::JSON_KEY
                    } else {
                        AppColors::JSON_STRING
                    },
                });
            }
            c if c == '-' || c.is_ascii_digit() => {
                let start = i;
                if c == '-' {
                    i += 1;
                }
                while i < len
                    && (chars[i].is_ascii_digit()
                        || chars[i] == '.'
                        || chars[i] == 'e'
                        || chars[i] == 'E'
                        || chars[i] == '+'
                        || chars[i] == '-')
                {
                    if i > start + 1
                        && (chars[i] == '-' || chars[i] == '+')
                        && chars[i - 1] != 'e'
                        && chars[i - 1] != 'E'
                    {
                        break;
                    }
                    i += 1;
                }

                segments.push(StyledSegment {
                    text: chars[start..i].iter().collect(),
                    color: AppColors::JSON_NUMBER,
                });
            }
            't' if i + 4 <= len && chars[i..i + 4].iter().collect::<String>() == "true" => {
                segments.push(StyledSegment {
                    text: "true".to_string(),
                    color: AppColors::JSON_BOOL,
                });
                i += 4;
            }
            'f' if i + 5 <= len && chars[i..i + 5].iter().collect::<String>() == "false" => {
                segments.push(StyledSegment {
                    text: "false".to_string(),
                    color: AppColors::JSON_BOOL,
                });
                i += 5;
            }
            'n' if i + 4 <= len && chars[i..i + 4].iter().collect::<String>() == "null" => {
                segments.push(StyledSegment {
                    text: "null".to_string(),
                    color: AppColors::JSON_NULL,
                });
                i += 4;
            }
            '{' | '}' | '[' | ']' | ':' | ',' => {
                segments.push(StyledSegment {
                    text: ch.to_string(),
                    color: AppColors::TEXT_SECONDARY,
                });
                i += 1;
            }
            _ if ch.is_whitespace() => {
                let start = i;
                while i < len && chars[i].is_whitespace() {
                    i += 1;
                }
                segments.push(StyledSegment {
                    text: chars[start..i].iter().collect(),
                    color: AppColors::TEXT_PRIMARY,
                });
            }
            _ => {
                segments.push(StyledSegment {
                    text: ch.to_string(),
                    color: AppColors::TEXT_PRIMARY,
                });
                i += 1;
            }
        }
    }

    highlight_segments(segments, query)
}

/// 普通文本关键词高亮
pub fn highlight_search(text_str: &str, query: &str, base_color: Color) -> Vec<Span<'static>> {
    highlight_segments(
        vec![StyledSegment {
            text: text_str.to_string(),
            color: base_color,
        }],
        query,
    )
}

/// 大小写不敏感包含判断
pub fn contains_query(text_str: &str, query: &str) -> bool {
    !find_match_ranges(text_str, query).is_empty()
}

/// 为搜索结果生成包含命中点的片段
pub fn excerpt_for_search(text_str: &str, query: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let query = query.trim();
    if query.is_empty() {
        return truncate_chars(text_str, max_chars);
    }

    let Some((start_byte, end_byte)) = first_match_range(text_str, query) else {
        return truncate_chars(text_str, max_chars);
    };

    let total_chars = text_str.chars().count();
    if total_chars <= max_chars {
        return text_str.to_string();
    }

    let match_start_char = text_str[..start_byte].chars().count();
    let match_end_char = text_str[..end_byte].chars().count();
    let match_len = match_end_char.saturating_sub(match_start_char);
    let context_chars = max_chars.saturating_sub(match_len).saturating_div(2);

    let mut start_char = match_start_char.saturating_sub(context_chars);
    let mut end_char = (match_end_char + context_chars).min(total_chars);

    let window_len = end_char.saturating_sub(start_char);
    if window_len < max_chars {
        let remaining = max_chars - window_len;
        start_char = start_char.saturating_sub(remaining / 2);
        end_char = (end_char + remaining).min(total_chars);
        if end_char - start_char < max_chars {
            start_char = end_char.saturating_sub(max_chars);
        }
    }

    let snippet = slice_by_char_range(text_str, start_char, end_char);
    let mut result = String::new();

    if start_char > 0 {
        result.push_str("...");
    }
    result.push_str(snippet);
    if end_char < total_chars {
        result.push_str("...");
    }

    result
}

fn highlight_segments(segments: Vec<StyledSegment>, query: &str) -> Vec<Span<'static>> {
    let query = query.trim();
    let mut spans = Vec::new();

    for segment in segments {
        if query.is_empty() {
            spans.push(Span::new(segment.text).color(segment.color));
            continue;
        }

        let ranges = find_match_ranges(&segment.text, query);
        if ranges.is_empty() {
            spans.push(Span::new(segment.text).color(segment.color));
            continue;
        }

        let mut last_end = 0;
        for (start, end) in ranges {
            if start > last_end {
                spans.push(
                    Span::new(segment.text[last_end..start].to_string()).color(segment.color),
                );
            }

            spans.push(
                Span::new(segment.text[start..end].to_string())
                    .color(AppColors::SEARCH_HIGHLIGHT_TEXT)
                    .background(Background::Color(AppColors::SEARCH_HIGHLIGHT_BG))
                    .border(Border {
                        color: AppColors::SEARCH_HIGHLIGHT,
                        width: 1.0,
                        radius: 4.0.into(),
                    })
                    .padding([0, 1]),
            );
            last_end = end;
        }

        if last_end < segment.text.len() {
            spans.push(Span::new(segment.text[last_end..].to_string()).color(segment.color));
        }
    }

    if spans.is_empty() {
        spans.push(Span::new(String::new()).color(AppColors::TEXT_PRIMARY));
    }

    spans
}

fn first_match_range(text_str: &str, query: &str) -> Option<(usize, usize)> {
    find_match_ranges(text_str, query).into_iter().next()
}

fn find_match_ranges(text_str: &str, query: &str) -> Vec<(usize, usize)> {
    let query = query.trim();
    if query.is_empty() || text_str.is_empty() {
        return Vec::new();
    }

    // 一次性 lowercase，用 str::find 进行 Boyer-Moore-Horspool 搜索（零额外分配）
    let text_lower = text_str.to_lowercase();
    let query_lower = query.to_lowercase();
    let query_bytes = query_lower.len();

    if query_bytes == 0 || query_bytes > text_lower.len() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut start = 0;

    while start + query_bytes <= text_lower.len() {
        if let Some(rel_pos) = text_lower[start..].find(&query_lower) {
            let byte_start = start + rel_pos;
            let byte_end = byte_start + query_bytes;

            // 确保字节边界是合法的 UTF-8 字符边界
            if text_str.is_char_boundary(byte_start) && text_str.is_char_boundary(byte_end) {
                ranges.push((byte_start, byte_end));
                start = byte_end;
            } else {
                // 边界不合法（多字节字符分割），前进一个字符
                start = byte_start
                    + text_str[byte_start..]
                        .chars()
                        .next()
                        .map_or(1, |c| c.len_utf8());
            }
        } else {
            break;
        }
    }

    ranges
}

fn truncate_chars(text_str: &str, max_chars: usize) -> String {
    let char_count = text_str.chars().count();
    if char_count <= max_chars {
        return text_str.to_string();
    }

    let end = byte_index_at_char(text_str, max_chars);
    format!("{}...", &text_str[..end])
}

fn slice_by_char_range(text_str: &str, start_char: usize, end_char: usize) -> &str {
    let start = byte_index_at_char(text_str, start_char);
    let end = byte_index_at_char(text_str, end_char);
    &text_str[start..end]
}

fn byte_index_at_char(text_str: &str, char_index: usize) -> usize {
    text_str
        .char_indices()
        .nth(char_index)
        .map(|(byte, _)| byte)
        .unwrap_or(text_str.len())
}


#[cfg(test)]
mod tests {
    use super::{contains_query, excerpt_for_search, highlight_search};
    use crate::theme::AppColors;

    #[test]
    fn contains_query_is_case_insensitive() {
        assert!(contains_query("OrderCreated", "created"));
        assert!(contains_query("订单创建成功", "创建"));
        assert!(!contains_query("payments", "orders"));
    }

    #[test]
    fn excerpt_for_search_keeps_match_in_view() {
        let excerpt = excerpt_for_search("abcdefghijklmnopqrstuvwxyz0123456789", "mnop", 12);

        assert!(excerpt.contains("mnop"));
        assert!(excerpt.starts_with("..."));
    }

    #[test]
    fn highlight_search_only_marks_keyword() {
        let spans = highlight_search("hello world", "world", AppColors::TEXT_PRIMARY);

        assert_eq!(spans.len(), 2);
        assert!(spans[1].highlight.is_some());
    }
}
