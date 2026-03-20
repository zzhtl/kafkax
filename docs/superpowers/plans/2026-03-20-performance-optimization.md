# KafkaX 极致性能优化实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 支持扫描 100 万条消息（单条最大 10MB+）时 UI 全程不卡顿，搜索结果实时增量显示。

**Architecture:** 三层优化——数据结构层引入轻量 `MessageSummary` 替换大对象传递；搜索层用 rayon 并行多分区 + `Task::batch` 流式增量返回；UI 层预计算行文本并用 `scrollable(text)` 替换 `text_editor` 消除 rope 卡死。

**Tech Stack:** Rust / iced 0.14 / rdkafka 0.39 / rayon 1 / tokio

**Spec:** `docs/superpowers/specs/2026-03-20-performance-design.md`

---

## 文件改动总览

| 文件 | 操作 | 核心职责变化 |
|------|------|-------------|
| `Cargo.toml` | 修改 | 添加 rayon |
| `src/kafka/types.rs` | 修改 | 新增 `MessageSummary`，更新 `PageData`，添加 `ScanLimits` |
| `src/kafka/consumer.rs` | 修改 | `search_partition_summarized`、`fetch_single_message`、并行搜索 |
| `src/codec/mod.rs` | 修改 | 新增 `decode_to_summary()` 方法 |
| `src/message.rs` | 修改 | 新增 `SearchProgress`，移除 `DetailEditorAction`，移除 `SearchLoaded` |
| `src/state/table_state.rs` | 修改 | `Arc<String>` 替换 rope，`Arc<Vec<MessageSummary>>` 替换完整消息 Vec |
| `src/app.rs` | 修改 | 处理 `SearchProgress` 累积、并行搜索任务、按需拉取详情 |
| `src/ui/message_table.rs` | 修改 | 直接使用预计算字段，移除 `value_summary` 重算 |
| `src/ui/message_detail.rs` | 修改 | `text_editor` → `scrollable(text(...))`，读 `Arc<String>` |
| `src/ui/syntax.rs` | 修改 | `find_match_ranges` 改用 `str::find` 替代 `char_boundaries` Vec |

---

## Task 1: 添加 rayon 依赖

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: 在 [dependencies] 中添加 rayon**

在 `rdkafka` 行后面加入：

```toml
rayon = "1"
```

- [ ] **Step 2: 验证编译通过**

```bash
cd /home/qingteng/rust-workspace/kafkax && cargo check 2>&1 | tail -5
```

Expected: `Finished` 无 error

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat: 添加 rayon 并行计算依赖"
```

---

## Task 2: 新增 MessageSummary 数据结构

**Files:**
- Modify: `src/kafka/types.rs`

目标：新增轻量摘要结构体，添加 `DecodedMessage::into_summary()` 转换方法，更新 `PageData.messages` 类型，更新排序函数。

- [ ] **Step 1: 在 `src/kafka/types.rs` 顶部引入缺失 import**

在现有 `use` 块中确保有：
```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
```

- [ ] **Step 2: 在文件末尾（`#[cfg(test)]` 之前）插入 `MessageSummary` 结构体**

```rust
/// 轻量消息摘要——用于列表显示和搜索结果存储（< 1KB/条）
#[derive(Debug, Clone)]
pub struct MessageSummary {
    // 元数据（用于详情拉取和排序）
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub timestamp: Option<DateTime<Utc>>,
    pub payload_size: usize,

    // 解码摘要（最多 500 字节）
    pub key_preview: Option<String>,
    pub value_format: &'static str,

    // 预计算显示标签（view() 直接使用，零分配）
    pub partition_label: String,  // "P-0"
    pub offset_label: String,     // "12345"
    pub ts_label: String,         // "03-20 14:32:01" 或 "-"
    pub key_label: String,        // key 截断到 120 字符
    pub value_label: String,      // 含关键词上下文，截断到 200 字符
}

/// 双重扫描限制计数器（跨线程共享）
#[derive(Debug, Clone)]
pub struct ScanLimits {
    pub total_scanned: Arc<AtomicUsize>,
    pub total_bytes: Arc<AtomicUsize>,
    pub max_messages: usize,
    pub max_bytes: usize,
}

impl ScanLimits {
    pub fn new(max_messages: usize, max_bytes: usize) -> Self {
        Self {
            total_scanned: Arc::new(AtomicUsize::new(0)),
            total_bytes: Arc::new(AtomicUsize::new(0)),
            max_messages,
            max_bytes,
        }
    }

    /// 尝试记录一条消息，返回 false 表示已超限，应停止扫描
    pub fn record(&self, payload_size: usize) -> bool {
        let prev_count = self.total_scanned.fetch_add(1, Ordering::Relaxed);
        let prev_bytes = self.total_bytes.fetch_add(payload_size, Ordering::Relaxed);
        prev_count < self.max_messages && prev_bytes < self.max_bytes
    }

    pub fn scanned(&self) -> usize {
        self.total_scanned.load(Ordering::Relaxed)
    }

    pub fn bytes(&self) -> usize {
        self.total_bytes.load(Ordering::Relaxed)
    }
}
```

- [ ] **Step 3: 为 `DecodedMessage` 添加 `into_summary()` 方法**

在 `impl DecodedMessage` 块中追加：

```rust
/// 转换为轻量摘要，query 用于生成含关键词上下文的 value_label
pub fn into_summary(self, query: &str) -> MessageSummary {
    let payload_size = self.raw.payload.as_ref().map_or(0, |p| p.len());

    let partition_label = format!("P-{}", self.raw.partition);
    let offset_label = self.raw.offset.to_string();
    let ts_label = self
        .raw
        .timestamp
        .map(|ts| ts.format("%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "-".to_string());

    let key_preview = self.decoded_key.as_ref().map(|k| {
        let s = k.trim_end_matches('\0');
        truncate_text(s, 200)
    });
    let key_label = key_preview.clone().unwrap_or_else(|| "-".to_string());

    // value_label：有 query 时取命中上下文，无 query 时取前缀
    let value_format = self.decoded_value.format_name();
    let raw_preview = self.decoded_value.preview(800);
    let value_label = if query.trim().is_empty() {
        truncate_text(&raw_preview, 200)
    } else {
        // 找到命中位置，截取上下文
        excerpt_around_query(&raw_preview, query, 200)
    };

    MessageSummary {
        topic: self.raw.topic,
        partition: self.raw.partition,
        offset: self.raw.offset,
        timestamp: self.raw.timestamp,
        payload_size,
        key_preview,
        value_format,
        partition_label,
        offset_label,
        ts_label,
        key_label,
        value_label,
    }
}
```

- [ ] **Step 4: 在文件中添加 `excerpt_around_query` 私有函数**

在 `truncate_text` 函数附近添加（`#[cfg(test)]` 之前）：

```rust
/// 从文本中截取包含 query 的上下文片段（最多 max_chars 字符）
fn excerpt_around_query(text: &str, query: &str, max_chars: usize) -> String {
    let query_lower = query.trim().to_lowercase();
    let text_lower = text.to_lowercase();

    let Some(byte_pos) = text_lower.find(&query_lower) else {
        return truncate_text(text, max_chars);
    };

    let match_char = text[..byte_pos].chars().count();
    let query_char_len = query.chars().count();
    let context = max_chars.saturating_sub(query_char_len) / 2;
    let start_char = match_char.saturating_sub(context);
    let end_char = (match_char + query_char_len + context).min(text.chars().count());

    let start_byte = byte_index_at_char(text, start_char);
    let end_byte = byte_index_at_char(text, end_char);
    let snippet = &text[start_byte..end_byte];

    let mut result = String::new();
    if start_char > 0 {
        result.push_str("...");
    }
    result.push_str(snippet);
    if end_char < text.chars().count() {
        result.push_str("...");
    }
    result
}
```

- [ ] **Step 5: 更新 `PageData` 的 messages 字段类型**

找到：
```rust
pub struct PageData {
    pub messages: Vec<DecodedMessage>,
```

改为：
```rust
pub struct PageData {
    pub messages: Vec<MessageSummary>,
```

- [ ] **Step 6: 新增针对摘要的排序函数**

在 `sort_search_matches` 函数附近添加：

```rust
/// 对搜索摘要结果排序，与 sort_search_matches 逻辑一致
pub fn sort_summaries(summaries: &mut [MessageSummary], order: SortOrder) {
    summaries.sort_by(|a, b| {
        let by_partition = a.partition.cmp(&b.partition);
        let by_offset = a.offset.cmp(&b.offset);
        let by_timestamp = a.timestamp.cmp(&b.timestamp);
        match order {
            SortOrder::Asc => by_timestamp.then(by_partition).then(by_offset),
            SortOrder::Desc => by_timestamp
                .reverse()
                .then(by_partition)
                .then(by_offset.reverse()),
        }
    });
}
```

- [ ] **Step 7: 编写 MessageSummary 单元测试（在 `#[cfg(test)]` 块中追加）**

```rust
#[test]
fn into_summary_computes_labels_correctly() {
    let msg = DecodedMessage {
        raw: KafkaMessage {
            topic: "orders".to_string(),
            partition: 2,
            offset: 9999,
            timestamp: Some(chrono::Utc.with_ymd_and_hms(2026, 3, 20, 10, 30, 0).unwrap()),
            key: Some(b"order-key".to_vec()),
            payload: Some(b"{\"id\":1}".to_vec()),
        },
        decoded_key: Some("order-key".to_string()),
        decoded_value: DecodedPayload::Json(serde_json::json!({"id": 1})),
    };

    let summary = msg.into_summary("");

    assert_eq!(summary.partition_label, "P-2");
    assert_eq!(summary.offset_label, "9999");
    assert_eq!(summary.key_label, "order-key");
    assert_eq!(summary.ts_label, "03-20 10:30:00");
    assert_eq!(summary.payload_size, 8);
    assert_eq!(summary.value_format, "JSON");
}

#[test]
fn scan_limits_stops_at_count() {
    let limits = ScanLimits::new(3, usize::MAX);
    assert!(limits.record(10));
    assert!(limits.record(10));
    assert!(limits.record(10));
    assert!(!limits.record(10)); // 第 4 条：前一个 fetch_add 返回 3 >= max(3)
}

#[test]
fn scan_limits_stops_at_bytes() {
    let limits = ScanLimits::new(usize::MAX, 100);
    assert!(limits.record(50));
    assert!(limits.record(50));
    assert!(!limits.record(1)); // 前一个 bytes fetch_add 返回 100 >= max(100)
}
```

- [ ] **Step 8: 运行测试确认通过**

```bash
cargo test --lib -- kafka::types 2>&1 | tail -20
```

Expected: 所有 `kafka::types` 测试 PASS

- [ ] **Step 9: Commit**

```bash
git add src/kafka/types.rs
git commit -m "feat: 新增 MessageSummary、ScanLimits，更新 PageData 类型"
```

---

## Task 3: 更新 DecoderPipeline 添加 decode_to_summary

**Files:**
- Modify: `src/codec/mod.rs`

- [ ] **Step 1: 在 `codec/mod.rs` 顶部添加 import**

在现有 `use` 之后添加：
```rust
use crate::kafka::types::MessageSummary;
```

- [ ] **Step 2: 在 `impl DecoderPipeline` 块中追加 decode_to_summary**

```rust
/// 解码并直接生成摘要，不保留原始字节和完整 JSON Value
/// 适用于搜索场景，避免在内存中堆积大对象
pub fn decode_to_summary(&self, msg: KafkaMessage, query: &str) -> MessageSummary {
    self.decode(msg).into_summary(query)
}
```

- [ ] **Step 3: 编译检查**

```bash
cargo check --lib 2>&1 | grep "^error" | head -20
```

Expected: 错误仅来自尚未更新的 consumer.rs / app.rs，codec 本身无错误

- [ ] **Step 4: Commit**

```bash
git add src/codec/mod.rs
git commit -m "feat: DecoderPipeline 添加 decode_to_summary 方法"
```

---

## Task 4: 更新 consumer.rs —— 并行搜索 + 按需拉取

**Files:**
- Modify: `src/kafka/consumer.rs`

- [ ] **Step 1: 添加必要 import**

在文件顶部现有 `use` 块中添加：
```rust
use std::sync::atomic::Ordering;
use rayon::prelude::*;
use crate::kafka::types::{MessageSummary, ScanLimits};
```

- [ ] **Step 2: 更新 fetch_page 返回 Vec<MessageSummary>**

找到 `fetch_page` 函数中的批量解码部分：
```rust
// 批量解码
let mut decoded = decoder.decode_batch(raw_messages);
sort_search_matches(&mut decoded, sort_order);

let total = high - low;

Ok(PageData {
    messages: decoded,
```

改为：
```rust
// 批量解码为摘要（不保留原始字节）
let mut summaries: Vec<MessageSummary> = raw_messages
    .into_iter()
    .map(|msg| decoder.decode_to_summary(msg, ""))
    .collect();
crate::kafka::types::sort_summaries(&mut summaries, sort_order);

let total = high - low;

Ok(PageData {
    messages: summaries,
```

- [ ] **Step 3: 新增 search_partition_summarized 函数**

在 `search_partition` 函数之后插入：

```rust
/// 搜索单个分区，返回轻量摘要，支持双重扫描限制
pub fn search_partition_summarized(
    connection_config: &ConnectionConfig,
    topic: &str,
    partition: i32,
    query: &str,
    decoder: &DecoderPipeline,
    limits: &ScanLimits,
) -> Result<(Vec<MessageSummary>, usize, usize, bool)> {
    // 返回：(命中摘要列表, 本分区扫描数, 本分区字节数, 是否因超限停止)
    let consumer = connection::create_consumer(connection_config)?;
    let (low, high) = consumer.fetch_watermarks(topic, partition, Duration::from_secs(5))?;

    if high <= low {
        return Ok((Vec::new(), 0, 0, false));
    }

    let mut simple_consumer = SimplePartitionConsumer::start(&consumer, topic, partition, low)?;

    let mut local_scanned = 0usize;
    let mut local_bytes = 0usize;
    let mut matches = Vec::new();
    let mut timeout_streak = 0usize;
    let final_offset = high.saturating_sub(1);
    let mut last_offset = low.saturating_sub(1);
    let mut stopped_early = false;

    while last_offset < final_offset {
        match simple_consumer.poll(Duration::from_secs(2))? {
            SimplePoll::Message(kafka_message) => {
                timeout_streak = 0;
                last_offset = kafka_message.offset;
                let payload_size = kafka_message.payload.as_ref().map_or(0, |p| p.len());

                // 检查全局限制
                if !limits.record(payload_size) {
                    stopped_early = true;
                    break;
                }

                local_scanned += 1;
                local_bytes += payload_size;

                let summary = decoder.decode_to_summary(kafka_message, query);
                if summary_matches_query(&summary, query) {
                    matches.push(summary);
                }
            }
            SimplePoll::PartitionEof => break,
            SimplePoll::Timeout => {
                timeout_streak += 1;
                if timeout_streak >= 5 {
                    anyhow::bail!(
                        "扫描分区超时，已读到 offset {}，目标结束 offset {}",
                        last_offset,
                        final_offset
                    );
                }
            }
        }
    }

    Ok((matches, local_scanned, local_bytes, stopped_early))
}

/// 判断摘要是否匹配搜索词（基于预计算的预览文本）
fn summary_matches_query(summary: &MessageSummary, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    let query_lower = query.to_lowercase();
    summary.key_label.to_lowercase().contains(&query_lower)
        || summary.value_label.to_lowercase().contains(&query_lower)
        || summary.offset_label.contains(query)
}
```

注意：这里用 `value_label` 做匹配有一个缺陷——`value_label` 是截断后的预览，对于超出截断长度的命中会漏报。解决：在 `decode_to_summary` 之前，先用更长的预览（800 字符）做匹配判断，命中再存入摘要。

- [ ] **Step 4: 修正匹配逻辑，先判断再生成摘要**

将 `search_partition_summarized` 中的 `SimplePoll::Message` 分支改为：

```rust
SimplePoll::Message(kafka_message) => {
    timeout_streak = 0;
    last_offset = kafka_message.offset;
    let payload_size = kafka_message.payload.as_ref().map_or(0, |p| p.len());

    // 先用原始 DecodedMessage 判断是否命中（保证不漏报）
    let decoded = decoder.decode(kafka_message);
    let is_match = decoded.matches_query(query);

    // 检查全局限制（无论是否命中都计入扫描量）
    if !limits.record(payload_size) {
        if is_match {
            matches.push(decoded.into_summary(query));
        }
        stopped_early = true;
        break;
    }

    local_scanned += 1;
    local_bytes += payload_size;

    if is_match {
        matches.push(decoded.into_summary(query));
    }
}
```

- [ ] **Step 5: 新增并行多分区搜索函数**

在 `search_topic` 函数之后添加：

```rust
/// 并行搜索 topic 所有分区，返回每个分区的独立结果
/// 调用方（app.rs）通过 Task::batch 并发调用每个分区的任务
pub fn search_topic_parallel(
    connection_config: &ConnectionConfig,
    topic: &str,
    partitions: &[i32],
    query: &str,
    decoder: &DecoderPipeline,
    limits: &ScanLimits,
) -> Result<Vec<(Vec<MessageSummary>, usize, usize, bool)>> {
    // 使用 rayon 并行处理各分区
    let results: Vec<Result<(Vec<MessageSummary>, usize, usize, bool)>> = partitions
        .par_iter()
        .map(|&partition| {
            search_partition_summarized(connection_config, topic, partition, query, decoder, limits)
        })
        .collect();

    // 收集结果，遇到错误则失败
    results.into_iter().collect()
}
```

- [ ] **Step 6: 新增 fetch_single_message 函数**

在文件末尾（`fn page_start_offset` 之前）添加：

```rust
/// 按 partition + offset 精确拉取单条消息的完整内容
/// 用于详情面板按需加载
pub fn fetch_single_message(
    connection_config: &ConnectionConfig,
    topic: &str,
    partition: i32,
    offset: i64,
    decoder: &DecoderPipeline,
) -> Result<String> {
    let consumer = connection::create_consumer(connection_config)?;

    // 验证 offset 在有效范围内
    let (low, high) = consumer.fetch_watermarks(topic, partition, Duration::from_secs(5))?;
    if offset < low || offset >= high {
        anyhow::bail!(
            "offset {} 超出分区范围 [{}, {})",
            offset, low, high
        );
    }

    let mut simple_consumer = SimplePartitionConsumer::start(&consumer, topic, partition, offset)?;

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            anyhow::bail!("拉取消息超时: topic={}, partition={}, offset={}", topic, partition, offset);
        }

        match simple_consumer.poll(remaining.min(Duration::from_millis(500)))? {
            SimplePoll::Message(kafka_message) => {
                if kafka_message.offset == offset {
                    let decoded = decoder.decode(kafka_message);
                    return Ok(decoded.copyable_json());
                }
                // offset 不匹配则继续（理论上不应发生）
            }
            SimplePoll::PartitionEof => {
                anyhow::bail!("到达分区末尾，未找到 offset {}", offset);
            }
            SimplePoll::Timeout => continue,
        }
    }
}
```

- [ ] **Step 7: 编译检查（允许 PageData 类型相关错误，后续 Task 修复）**

```bash
cargo check --lib 2>&1 | grep "^error" | head -30
```

- [ ] **Step 8: Commit**

```bash
git add src/kafka/consumer.rs
git commit -m "feat: 新增并行分区搜索、fetch_single_message、扫描限制支持"
```

---

## Task 5: 更新 message.rs —— 新增 SearchProgress，移除旧变体

**Files:**
- Modify: `src/message.rs`

- [ ] **Step 1: 更新 import**

将文件顶部改为：
```rust
use crate::codec::DecoderType;
use crate::config::{ConnectionConfig, SaslMechanism, SecurityProtocol};
use crate::kafka::types::{MessageSummary, PageData, SortOrder, TopicMeta};
// 移除：use iced::widget::text_editor;
```

- [ ] **Step 2: 替换搜索相关消息变体**

找到并移除：
```rust
/// 全分区搜索完成
SearchLoaded(u64, Result<(SearchResult, u128), String>),
```

替换为：
```rust
/// 单个分区搜索完成（并行搜索中每个分区独立返回）
SearchProgress {
    request_id: u64,
    partition: i32,
    hits: Vec<MessageSummary>,
    local_scanned: usize,
    local_bytes: usize,
    stopped_early: bool,
},
```

- [ ] **Step 3: 移除 DetailEditorAction 变体**

找到并删除：
```rust
/// 详情区编辑器动作
DetailEditorAction(text_editor::Action),
```

- [ ] **Step 4: 编译检查**

```bash
cargo check --lib 2>&1 | grep "^error" | head -30
```

Expected: 错误来自 app.rs（还未更新对这些变体的处理），types.rs 本身应无误

- [ ] **Step 5: Commit**

```bash
git add src/message.rs
git commit -m "feat: 新增 SearchProgress 消息，移除 SearchLoaded 和 DetailEditorAction"
```

---

## Task 6: 更新 table_state.rs —— Arc<String> + Arc<Vec<MessageSummary>>

**Files:**
- Modify: `src/state/table_state.rs`

- [ ] **Step 1: 更新 import**

将文件顶部替换为：
```rust
use std::sync::Arc;
use crate::kafka::types::{MessageSummary, PageData, SortOrder, sort_summaries};
```

移除：
```rust
use iced::widget::text_editor;
use crate::kafka::types::{DecodedMessage, PageData, SortOrder, sort_search_matches};
```

- [ ] **Step 2: 更新 TableState 结构体字段**

将以下字段：
```rust
pub messages: Vec<DecodedMessage>,
pub search_results: Option<Vec<DecodedMessage>>,
// ...
pub detail_content: text_editor::Content,
pub detail_loading: bool,
```

改为：
```rust
pub messages: Vec<MessageSummary>,
pub search_results: Option<Arc<Vec<MessageSummary>>>,
// ...
pub detail_text: Arc<String>,
pub detail_loading: bool,
```

- [ ] **Step 3: 更新 Default impl**

```rust
impl Default for TableState {
    fn default() -> Self {
        Self {
            messages: vec![],
            search_results: None,
            search_scanned_messages: None,
            browse_page_before_search: None,
            current_page: 0,
            page_size: 100,
            sort_order: SortOrder::Desc,
            total_messages: 0,
            low_watermark: 0,
            high_watermark: 0,
            selected_index: None,
            detail_text: Arc::new(String::new()),
            detail_loading: false,
            search_query: String::new(),
            loading: false,
            search_in_progress: false,
            load_time_ms: None,
            error_message: None,
        }
    }
}
```

- [ ] **Step 4: 更新详情相关方法**

找到并替换 `set_detail_text`、`begin_detail_loading`、`clear_detail`：

```rust
/// 更新详情区文本（Arc 赋值，几乎零开销）
pub fn set_detail_text(&mut self, text: impl Into<String>) {
    self.detail_text = Arc::new(text.into());
    self.detail_loading = false;
}

/// 详情区进入后台加载态
pub fn begin_detail_loading(&mut self, placeholder: impl Into<String>) {
    self.detail_text = Arc::new(placeholder.into());
    self.detail_loading = true;
}

/// 清理详情区
pub fn clear_detail(&mut self) {
    self.detail_text = Arc::new(String::new());
    self.detail_loading = false;
}
```

- [ ] **Step 5: 更新 apply_page_data**

```rust
pub fn apply_page_data(&mut self, data: PageData, load_time_ms: u128) {
    self.search_results = None;
    self.search_scanned_messages = None;
    self.browse_page_before_search = None;
    self.messages = data.messages;  // 已经是 Vec<MessageSummary>
    self.current_page = data.page;
    self.total_messages = data.total_messages;
    self.low_watermark = data.low_watermark;
    self.high_watermark = data.high_watermark;
    self.loading = false;
    self.search_in_progress = false;
    self.load_time_ms = Some(load_time_ms);
    self.selected_index = None;
    self.clear_detail();
    self.error_message = None;
}
```

- [ ] **Step 6: 更新 apply_search_results**

```rust
pub fn apply_search_results(
    &mut self,
    mut results: Vec<MessageSummary>,
    scanned_messages: usize,
    low_watermark: i64,
    high_watermark: i64,
    load_time_ms: u128,
) {
    sort_summaries(&mut results, self.sort_order);
    let total = results.len() as i64;
    self.search_results = Some(Arc::new(results));
    self.search_scanned_messages = Some(scanned_messages);
    self.current_page = 0;
    self.total_messages = total;
    self.low_watermark = low_watermark;
    self.high_watermark = high_watermark;
    self.loading = false;
    self.search_in_progress = false;
    self.load_time_ms = Some(load_time_ms);
    self.selected_index = None;
    self.clear_detail();
    self.error_message = None;
    self.apply_search_page();
}
```

- [ ] **Step 7: 更新 apply_search_page**

```rust
pub fn apply_search_page(&mut self) {
    let Some(results) = &self.search_results else {
        return;
    };

    let start = self.current_page.saturating_mul(self.page_size);
    let end = (start + self.page_size).min(results.len());

    self.messages = if start < end {
        results[start..end].to_vec()  // 只 clone 当前页（最多 page_size 条摘要，< 100KB）
    } else {
        Vec::new()
    };
    self.total_messages = results.len() as i64;
    self.selected_index = None;
    self.clear_detail();
    self.loading = false;
    self.search_in_progress = false;
}
```

- [ ] **Step 8: 更新 set_sort_order**

```rust
pub fn set_sort_order(&mut self, order: SortOrder) -> bool {
    if self.sort_order == order {
        return false;
    }
    self.sort_order = order;
    self.current_page = 0;
    self.selected_index = None;
    self.clear_detail();

    if let Some(arc_results) = self.search_results.take() {
        // 从 Arc 中取出（如果只有一个引用则零拷贝，否则 clone）
        let mut results = Arc::try_unwrap(arc_results)
            .unwrap_or_else(|arc| (*arc).clone());
        sort_summaries(&mut results, order);
        self.search_results = Some(Arc::new(results));
        self.apply_search_page();
    }

    true
}
```

- [ ] **Step 9: 更新 selected_message 返回类型**

```rust
pub fn selected_message(&self) -> Option<&MessageSummary> {
    self.selected_index.and_then(|idx| self.messages.get(idx))
}
```

- [ ] **Step 10: 更新测试模块**

测试用 `DecodedMessage` 的地方改为 `MessageSummary`，在 `#[cfg(test)]` 中的 `sample_message` 改为：

```rust
fn sample_summary(offset: i64) -> MessageSummary {
    MessageSummary {
        topic: "orders".to_string(),
        partition: 0,
        offset,
        timestamp: Some(chrono::Utc.with_ymd_and_hms(2026, 3, 19, 10, 0, 0).unwrap()),
        payload_size: 10,
        key_preview: Some(format!("order-{offset}")),
        value_format: "Text",
        partition_label: "P-0".to_string(),
        offset_label: offset.to_string(),
        ts_label: "03-19 10:00:00".to_string(),
        key_label: format!("order-{offset}"),
        value_label: format!("value-{offset}"),
    }
}
```

将所有测试中调用 `sample_message` 改为 `sample_summary`，并更新 `apply_search_results` 的调用签名（移除 `scanned_messages` 参数变化等）。

- [ ] **Step 11: 运行测试**

```bash
cargo test --lib -- state:: 2>&1 | tail -20
```

Expected: `state::table_state` 模块所有测试 PASS

- [ ] **Step 12: Commit**

```bash
git add src/state/table_state.rs
git commit -m "feat: TableState 改用 Arc<String> 和 Arc<Vec<MessageSummary>>，消除昂贵 clone"
```

---

## Task 7: 更新 app.rs —— 并行搜索任务 + SearchProgress 处理 + 详情按需拉取

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: 更新 import**

移除 `kafkax::kafka::types::SearchResult` 相关引用，添加：
```rust
use kafkax::kafka::types::{MessageSummary, ScanLimits, SortOrder};
```

- [ ] **Step 2: 在 App 结构体中添加搜索累积字段**

```rust
pub struct App {
    pub state: AppState,
    pub consumer: Option<Arc<BaseConsumer>>,
    main_window_id: Option<window::Id>,
    page_request_id: u64,
    search_request_id: u64,
    detail_request_id: u64,
    // 新增：搜索累积状态
    search_total_partitions: usize,
    search_received_partitions: usize,
    search_accumulator: Vec<MessageSummary>,
    search_low_watermark: i64,
    search_high_watermark: i64,
    search_start_time: Option<std::time::Instant>,
    search_scan_limits: Option<ScanLimits>,
}
```

在 `App::new()` 中添加对应默认值：
```rust
search_total_partitions: 0,
search_received_partitions: 0,
search_accumulator: Vec::new(),
search_low_watermark: 0,
search_high_watermark: 0,
search_start_time: None,
search_scan_limits: None,
```

- [ ] **Step 3: 在 update() 中移除 SearchLoaded 分支，添加 SearchProgress 处理**

移除整个 `Message::SearchLoaded(...)` 分支。

在 `Message::DetailLoaded` 之前插入：

```rust
Message::SearchProgress {
    request_id,
    partition,
    hits,
    local_scanned,
    local_bytes: _,
    stopped_early,
} => {
    if request_id != self.search_request_id {
        return Task::none();
    }

    // 累积命中结果
    self.search_accumulator.extend(hits);
    self.search_received_partitions += 1;

    // 更新进度提示
    let scanned = self.search_scan_limits
        .as_ref()
        .map_or(0, |l| l.scanned());
    self.state.notice = Some(AppNotice::info(format!(
        "正在搜索 partition {}... 已扫描 {} 条，命中 {} 条",
        partition,
        scanned,
        self.search_accumulator.len(),
    )));

    if stopped_early {
        // 某分区超限，可能其他分区还在运行，继续等待
        tracing::info!("分区 {} 因超限停止", partition);
    }

    // 所有分区都返回了结果
    if self.search_received_partitions >= self.search_total_partitions {
        self.finalize_search()
    } else {
        Task::none()
    }
}
```

- [ ] **Step 4: 添加 finalize_search 方法**

在 `impl App` 中添加：

```rust
fn finalize_search(&mut self) -> Task<Message> {
    let results = std::mem::take(&mut self.search_accumulator);
    let scanned = self.search_scan_limits.as_ref().map_or(0, |l| l.scanned());
    let scanned_bytes = self.search_scan_limits.as_ref().map_or(0, |l| l.bytes());
    let low = self.search_low_watermark;
    let high = self.search_high_watermark;
    let match_count = results.len();
    let elapsed = self.search_start_time
        .take()
        .map_or(0, |t| t.elapsed().as_millis());

    self.search_scan_limits = None;
    self.search_total_partitions = 0;
    self.search_received_partitions = 0;

    let stopped = scanned_bytes >= 2 * 1024 * 1024 * 1024
        || scanned >= 1_000_000;

    self.state.table.apply_search_results(results, scanned, low, high, elapsed);

    let msg = if stopped {
        format!(
            "搜索完成（已达扫描上限）：扫描 {} 条，命中 {} 条，耗时 {}ms",
            scanned, match_count, elapsed
        )
    } else {
        format!(
            "搜索完成：扫描 {} 条，命中 {} 条，耗时 {}ms",
            scanned, match_count, elapsed
        )
    };
    self.state.notice = Some(AppNotice::success(msg));

    Task::none()
}
```

- [ ] **Step 5: 重写 begin_topic_search**

找到并替换整个 `fn begin_topic_search` 方法：

```rust
fn begin_topic_search(&mut self) -> Task<Message> {
    let query = self.state.table.search_query.trim().to_string();
    if query.is_empty() {
        self.invalidate_search_requests();
        if self.state.table.has_search_results() || self.state.table.search_in_progress {
            self.state.table.clear_search_results();
            return self.reload_current_page();
        }
        self.clear_detail_content();
        return Task::none();
    }

    if self.consumer.is_none() {
        self.state.notice = Some(AppNotice::info("请先连接 Kafka 再执行搜索"));
        return Task::none();
    }

    let Some((topic, partitions)) = self.selected_topic_partitions() else {
        self.state.notice = Some(AppNotice::info(
            "请先从左侧选择一个 Topic 对应的 Partition，再执行搜索",
        ));
        return Task::none();
    };

    if partitions.is_empty() {
        self.state.notice = Some(AppNotice::error(format!(
            "Topic {topic} 没有可搜索的 Partition"
        )));
        return Task::none();
    }

    self.invalidate_page_requests();
    self.state.table.begin_partition_search();

    // 重置累积状态
    self.search_accumulator.clear();
    self.search_total_partitions = partitions.len();
    self.search_received_partitions = 0;
    self.search_start_time = Some(Instant::now());
    self.search_low_watermark = i64::MAX;
    self.search_high_watermark = i64::MIN;

    // 创建共享扫描限制
    let limits = ScanLimits::new(1_000_000, 2 * 1024 * 1024 * 1024);
    self.search_scan_limits = Some(limits.clone());

    let request_id = self.next_search_request_id();
    let connection_config = self.state.connection_config.clone();
    let decoder = self.state.decoder.clone();

    self.state.notice = Some(AppNotice::info(format!(
        "正在并行搜索 Topic {topic} 的 {} 个 Partition...",
        partitions.len()
    )));

    // 每个分区独立 Task，Task::batch 并发执行
    let tasks: Vec<Task<Message>> = partitions
        .into_iter()
        .map(|partition| {
            let config = connection_config.clone();
            let topic = topic.clone();
            let query = query.clone();
            let dec = decoder.clone();
            let lim = limits.clone();

            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        kafka::consumer::search_partition_summarized(
                            &config, &topic, partition, &query, &dec, &lim,
                        )
                    })
                    .await
                    .map_err(|e| format!("分区 {partition} 搜索任务异常: {e}"))?
                    .map_err(|e| e.to_string())
                },
                move |result| match result {
                    Ok((hits, scanned, bytes, stopped)) => Message::SearchProgress {
                        request_id,
                        partition,
                        hits,
                        local_scanned: scanned,
                        local_bytes: bytes,
                        stopped_early: stopped,
                    },
                    Err(err) => {
                        tracing::error!("分区 {} 搜索失败: {}", partition, err);
                        Message::SearchProgress {
                            request_id,
                            partition,
                            hits: vec![],
                            local_scanned: 0,
                            local_bytes: 0,
                            stopped_early: false,
                        }
                    }
                },
            )
        })
        .collect();

    Task::batch(tasks)
}
```

注意：错误分支也发送 SearchProgress（hits 为空），确保 `search_received_partitions` 计数正确推进，最终一定能触发 `finalize_search`。如需记录错误可添加 `tracing::error!`。

- [ ] **Step 6: 更新 load_selected_detail**

找到并替换 `fn load_selected_detail`：

```rust
fn load_selected_detail(&mut self) -> Task<Message> {
    let Some(summary) = self.state.table.selected_message().cloned() else {
        self.clear_detail_content();
        return Task::none();
    };

    let request_id = self.next_detail_request_id();
    self.state.table.begin_detail_loading("正在拉取完整消息详情...");

    let connection_config = self.state.connection_config.clone();
    let decoder = self.state.decoder.clone();
    let topic = summary.topic.clone();
    let partition = summary.partition;
    let offset = summary.offset;

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                kafka::consumer::fetch_single_message(
                    &connection_config,
                    &topic,
                    partition,
                    offset,
                    &decoder,
                )
            })
            .await
            .map_err(|e| format!("后台详情任务异常终止: {e}"))?
            .map_err(|e| e.to_string())
        },
        move |result| Message::DetailLoaded(request_id, result),
    )
}
```

- [ ] **Step 7: 更新 copy_selected_json**

找到并替换 `fn copy_selected_json`：

```rust
fn copy_selected_json(&mut self) -> Task<Message> {
    let detail = self.state.table.detail_text.as_ref().clone();
    if detail.is_empty() {
        self.state.notice = Some(AppNotice::info("详情尚未加载完成，请稍候再复制"));
        return Task::none();
    }
    self.state.notice = Some(AppNotice::success("已复制当前消息 JSON"));
    clipboard::write::<Message>(detail)
}
```

- [ ] **Step 8: 移除 DetailEditorAction 处理分支**

在 `update()` 中找到并删除：
```rust
Message::DetailEditorAction(action) => {
    if !action.is_edit() {
        self.state.table.detail_content.perform(action);
    }
    Task::none()
}
```

- [ ] **Step 9: 更新 DetailLoaded 处理**

找到 `Message::DetailLoaded` 分支，确保它使用 `set_detail_text`（不变，只是底层实现变了）。

- [ ] **Step 10: 编译检查**

```bash
cargo check 2>&1 | grep "^error" | head -30
```

逐一解决编译错误，主要会是类型不匹配和 import 问题。

- [ ] **Step 11: Commit**

```bash
git add src/app.rs
git commit -m "feat: 并行搜索任务、SearchProgress 累积、详情按需拉取"
```

---

## Task 8: 更新 message_table.rs —— 使用预计算字段

**Files:**
- Modify: `src/ui/message_table.rs`

- [ ] **Step 1: 更新 import**

```rust
use crate::kafka::types::MessageSummary;
// 移除：use crate::kafka::types::DecodedMessage;
```

- [ ] **Step 2: 替换行渲染中的字段计算**

在 `view` 函数的 `state.messages.iter().enumerate().map(...)` 内，将：

```rust
let partition_str = format!("P-{}", msg.raw.partition);
let offset_str = msg.raw.offset.to_string();
let ts_str = msg.raw.timestamp
    .map(|ts| ts.format("%m-%d %H:%M:%S").to_string())
    .unwrap_or_else(|| "-".to_string());
let key_str = msg.decoded_key.as_deref().unwrap_or("-").to_string();
let value_str = value_summary(msg, search_query, 200);
```

改为：

```rust
let partition_str = &msg.partition_label;
let offset_str = &msg.offset_label;
let ts_str = &msg.ts_label;
let key_str = &msg.key_label;
let value_str = if search_query.is_empty() {
    msg.value_label.clone()
} else {
    // value_label 已是 500 字符以内的预览，直接使用，highlight_search 会在其上高亮
    msg.value_label.clone()
};
```

- [ ] **Step 3: 删除 value_summary 函数**

找到并删除整个 `fn value_summary` 函数（不再需要）。

- [ ] **Step 4: 更新 cell_rich_text 调用（类型兼容）**

确保 `highlight_search` 的参数从 `&String` 正确传入。

- [ ] **Step 5: 编译检查**

```bash
cargo check 2>&1 | grep "^error" | head -20
```

- [ ] **Step 6: Commit**

```bash
git add src/ui/message_table.rs
git commit -m "perf: 消息表格行直接使用预计算标签，消除每帧字符串重算"
```

---

## Task 9: 更新 message_detail.rs —— text_editor → scrollable(text)

**Files:**
- Modify: `src/ui/message_detail.rs`

- [ ] **Step 1: 更新 import**

将文件顶部替换为：
```rust
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};
// 移除：use iced::widget::{..., text_editor};
// 移除：use crate::theme::{self, AppColors};
use crate::message::Message;
use crate::state::TableState;
use crate::theme::AppColors;
```

- [ ] **Step 2: 替换详情显示区域**

在 `view` 函数的 `Some(msg)` 分支中，找到 `let editor = text_editor(...)` 整段，替换为：

```rust
直接内联，把原来的 `scrollable(container(editor))` 部分改为：

```rust
let detail_scroll = scrollable(
    container(
        text(state.detail_text.as_ref())
            .font(crate::theme::detail_font())
            .size(13)
            .color(AppColors::TEXT_PRIMARY),
    )
    .padding(12),
)
.height(Length::Fill);
```

- [ ] **Step 3: 更新 detail 列构建**

找到：
```rust
let detail = column![
    header,
    text(if state.detail_loading { ... }).size(11)...,
    scrollable(container(editor).height(Length::Fill)),
]
```

改为：
```rust
let detail = column![
    header,
    text(if state.detail_loading {
        "正在后台拉取完整消息，完成后自动刷新..."
    } else {
        "点击右上角按钮复制完整 JSON。"
    })
    .size(11)
    .color(AppColors::TEXT_MUTED),
    detail_scroll,
]
.spacing(10)
.padding(12);
```

- [ ] **Step 4: 更新 selected_message 使用**

`Some(msg)` 现在接收 `&MessageSummary`，元数据字段改为：
- `msg.raw.topic` → `msg.topic`
- `msg.raw.partition` → `msg.partition`
- `msg.raw.offset` → `msg.offset`
- `msg.raw.timestamp` → `msg.timestamp`

- [ ] **Step 5: 编译检查**

```bash
cargo check 2>&1 | grep "^error" | head -20
```

- [ ] **Step 6: Commit**

```bash
git add src/ui/message_detail.rs
git commit -m "perf: 详情面板改用 scrollable(text)，彻底消除 rope 构建卡死"
```

---

## Task 10: 优化 syntax.rs —— 快速字节扫描替换 char_boundaries

**Files:**
- Modify: `src/ui/syntax.rs`

- [ ] **Step 1: 替换 find_match_ranges 实现**

找到并替换整个 `fn find_match_ranges` 函数：

```rust
fn find_match_ranges(text_str: &str, query: &str) -> Vec<(usize, usize)> {
    let query = query.trim();
    if query.is_empty() || text_str.is_empty() {
        return Vec::new();
    }

    // 一次性 lowercase（避免逐字符比较）
    let text_lower = text_str.to_lowercase();
    let query_lower = query.to_lowercase();
    let query_bytes = query_lower.len();

    if query_bytes == 0 || query_bytes > text_lower.len() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut start = 0;

    while start + query_bytes <= text_lower.len() {
        // 用标准库的 Boyer-Moore-Horspool 实现，O(n) 平均
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
```

- [ ] **Step 2: 删除 char_boundaries 函数**

找到并删除：
```rust
fn char_boundaries(text_str: &str) -> Vec<usize> {
    text_str
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text_str.len()))
        .collect()
}
```

- [ ] **Step 3: 运行 syntax 模块测试**

```bash
cargo test --lib -- ui::syntax 2>&1 | tail -15
```

Expected: 所有测试 PASS

- [ ] **Step 4: Commit**

```bash
git add src/ui/syntax.rs
git commit -m "perf: find_match_ranges 改用 str::find 快速路径，消除 char_boundaries Vec 分配"
```

---

## Task 11: 全量编译修复 & 最终验证

**Files:**
- All modified files as needed

- [ ] **Step 1: 全量编译**

```bash
cargo build 2>&1 | grep "^error" | head -40
```

逐一修复所有剩余编译错误。常见错误类型：
- `DecodedMessage` 在某处仍被引用 → 改为 `MessageSummary`
- `detail_content` 字段名 → `detail_text`
- `SearchResult` 类型导入 → 不再需要，移除 import
- `SearchLoaded` 变体 → 不存在，改为 `SearchProgress`
- `DetailEditorAction` → 已删除

- [ ] **Step 2: 运行所有测试**

```bash
cargo test 2>&1 | tail -30
```

Expected: 所有测试 PASS，重点关注：
- `kafka::types` 测试（MessageSummary、ScanLimits）
- `state::table_state` 测试（分页、排序、搜索结果）
- `ui::syntax` 测试（高亮、摘要提取）
- `tests::codec_tests` 测试（不受影响）

- [ ] **Step 3: Release 编译确认**

```bash
cargo build --release 2>&1 | tail -5
```

Expected: `Finished release` 无 warning（或 warning 数量未增加）

- [ ] **Step 4: 最终 Commit**

```bash
git add -u
git commit -m "feat: 极致性能优化完成——并行搜索+流式结果+Arc状态+预计算渲染"
```

---

## 快速验证清单（手动测试）

实现完成后，按以下步骤手动验证：

1. **启动应用**：`cargo run --release`
2. **连接 Kafka**，选择一个 Topic 和 Partition
3. **翻页**：切换分区后翻页，UI 应无卡顿
4. **搜索小 Topic**（< 1 万条）：输入关键词回车，结果应快速出现
5. **搜索大 Topic**（100 万条）：应看到进度更新，每个分区完成后立即追加结果
6. **点击消息**：详情面板异步加载，UI 不阻塞
7. **点击大消息**（10MB+）：详情面板显示完整内容，不卡死
8. **复制 JSON**：点击复制按钮，粘贴验证内容完整
9. **搜索超限**：状态栏显示"已达扫描上限"提示

---

## 注意事项

- `search_partition_summarized` 的 `summary_matches_query` 使用了预计算的 `value_label`（500字符以内）做二次判断。为保证不漏报，在 `Step 4` 中改为先用完整 `DecodedMessage.matches_query()` 判断，命中后再调 `into_summary()`。这会稍微增加内存占用（短暂持有 DecodedMessage），但保证正确性。
- `Task::batch` 并发执行所有分区任务，每个任务在 `tokio::task::spawn_blocking` 中运行，会占用 `tokio` 的 blocking thread pool（默认 512 个线程），分区数通常远小于此上限，无需担心。
- `ScanLimits::record` 使用 `Relaxed` 内存序，有极小概率在超限后仍处理少量额外消息（ABA 问题），属于可接受的近似限制，不影响正确性。
