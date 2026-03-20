# KafkaX 极致性能优化设计文档

**日期**: 2026-03-20
**目标**: 支持扫描 100 万条消息（单条最大 10MB+），UI 全程不卡顿
**方案**: 方案三——数据结构重构 + rayon 并行搜索 + 流式增量返回 + UI 渲染优化

---

## 一、背景与问题

### 已确认的性能根因

| 层级 | 问题描述 | 严重程度 |
|------|----------|----------|
| 搜索层 | `search_topic()` 串行逐分区扫描，每分区新建 consumer | 🔴 极高 |
| 内存层 | 搜索结果存完整 `DecodedMessage`（原始字节 + Value），100 万大消息 = OOM | 🔴 极高 |
| 状态层 | `TableState` Clone 时复制 `text_editor::Content`（iced rope），大 JSON 卡死 | 🔴 极高 |
| 渲染层 | 每帧每行 5 次 `highlight_search()`，每次 `char_boundaries()` 分配 Vec | 🟡 中等 |
| 渲染层 | `text_editor::Content::with_text()` 在 UI 线程构建 rope，10MB JSON 直接卡死 | 🔴 极高 |
| 数据层 | `apply_search_page()` clone 当前页消息到 `messages`，双份内存 | 🟡 中等 |

### 用户场景约束
- 单条消息最大可达 10MB+，但大部分较小
- 搜索结果最多 100 万条
- 扫描上限：消息数上限 100 万条 **且** 累计 payload 总大小 2GB，任一先到则停止
- 搜索结果展示方式：只存摘要，点击行时按需从 Kafka 重新拉取完整消息
- 详情面板：显示完整内容，不截断，但可以失去拖拽选区功能

---

## 二、数据结构重构

### 2.1 拆分 MessageSummary 与 MessageDetail

废弃原有的 `DecodedMessage`（或保留用于翻页），新增：

```rust
/// 列表用——轻量摘要，搜索结果只存这个（< 1KB/条）
pub struct MessageSummary {
    // 元数据
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub timestamp: Option<DateTime<Utc>>,
    pub payload_size: usize,          // 原始字节数，用于 2GB 计数

    // 解码摘要（最多 500 字节）
    pub key_preview: Option<String>,
    pub value_preview: String,
    pub value_format: &'static str,   // "JSON" / "Text" 等

    // 预计算显示文本（view() 直接用，零分配）
    pub partition_label: String,      // "P-0"
    pub offset_label: String,         // "12345"
    pub ts_label: String,             // "03-20 14:32:01"
    pub key_label: String,            // key 截断文本
    pub value_label: String,          // 含关键词上下文的摘要
}

/// 详情用——点击时按 partition+offset 精确拉取
pub struct MessageDetail {
    pub summary: MessageSummary,
    pub full_value: String,           // 完整格式化内容（pretty print）
}
```

### 2.2 TableState 改动

```rust
pub struct TableState {
    pub messages: Vec<MessageSummary>,              // 当前页（翻页模式）
    pub search_results: Option<Arc<Vec<MessageSummary>>>,  // 搜索结果（Arc，零 clone 开销）
    pub detail_text: Arc<String>,                   // 替换 text_editor::Content
    pub detail_loading: bool,
    // ... 其余不变 ...
}
```

关键改动：
- `search_results` 用 `Arc<Vec<>>` 包装，翻页切片时只 clone Arc（原子操作）
- `detail_text: Arc<String>` 替换 `text_editor::Content`，clone 只是原子计数 +1
- `apply_search_page()` 不再 clone 消息，视图层直接通过 Arc 切片访问

---

## 三、搜索层重构

### 3.1 并行多分区搜索（rayon）

```rust
// 新增依赖
rayon = "1"

// 所有分区并行扫描
partitions.par_iter().for_each(|&partition| {
    search_partition_streaming(config, topic, partition, query, decoder, tx.clone(), limits);
});
```

每个分区在独立的 rayon 线程中运行，N 个分区同时扫描，速度提升 N 倍。

### 3.2 流式增量返回（mpsc channel）

```
扫描线程                    UI 线程
   │                          │
   │──SearchProgress(batch)──>│  立即显示已扫描结果
   │──SearchProgress(batch)──>│  追加，更新进度
   │──SearchProgress(done)───>│  标记完成，最终排序
```

新增消息类型：
```rust
Message::SearchProgress {
    hits: Vec<MessageSummary>,
    scanned_count: usize,
    scanned_bytes: usize,
    is_done: bool,
    partition: i32,
    stopped_early: bool,   // 是否因超限而停止
}
```

iced 侧通过 `Subscription`（`channel` 或 `run_with_id`）接收流式结果。

### 3.3 双重扫描限制

每个扫描分区线程维护共享的原子计数器：

```rust
let total_scanned = Arc::new(AtomicUsize::new(0));
let total_bytes   = Arc::new(AtomicUsize::new(0));

// 每条消息处理后检查
if total_scanned.fetch_add(1, Relaxed) >= MAX_SCAN_COUNT
    || total_bytes.fetch_add(payload_size, Relaxed) >= MAX_SCAN_BYTES {
    // 停止当前分区，通知 UI
    break;
}
```

限制：MAX_SCAN_COUNT = 1_000_000，MAX_SCAN_BYTES = 2 * 1024 * 1024 * 1024（2GB）

超限时状态栏显示：`"已扫描 XX 万条 / 2GB，已达上限停止，建议使用更精确的关键词"`

---

## 四、UI 渲染优化

### 4.1 预计算行显示文本

`MessageSummary` 在数据到达时（后台线程中）一次性计算好所有显示字段：
- `partition_label`, `offset_label`, `ts_label`, `key_label`, `value_label`

`message_table::view()` 直接使用这些字段，零字符串分配。

### 4.2 highlight_search 快速字节扫描

替换 `char_boundaries()` 的 Vec 分配：

```rust
// 旧：每次分配 Vec<usize>
fn char_boundaries(text: &str) -> Vec<usize> { ... }

// 新：直接字节迭代，零分配
fn find_match_ranges_fast(text: &str, query: &str) -> Vec<(usize, usize)> {
    // 对 ASCII 纯字节操作，只在多字节字符边界做修正
}
```

### 4.3 详情面板替换 text_editor

```rust
// 旧（UI 线程构建 rope，10MB 卡死）
text_editor::Content::with_text(&large_json)

// 新（Arc<String> 零 clone，scrollable 只渲染可见区域）
scrollable(
    text(detail_text.as_ref())
        .font(detail_font())
        .size(13)
)
```

- `Arc<String>` 在后台 blocking 线程中准备，完成后发送给 UI
- `scrollable` 裁剪可见区域，只绘制屏幕内文字，10MB 内容也不卡
- 复制整条 JSON 的按钮保留，从 `Arc<String>` 直接读取

### 4.4 detail_text 不截断

完整 pretty-print 内容全部显示。格式化在 `spawn_blocking` 中完成，UI 线程只做 Arc 赋值。

---

## 五、点击按需拉取详情

```
用户点击某行
    │
    ▼
Message::SelectMessage(idx)
    │
    ▼
从 search_results/messages 取出 MessageSummary（只有摘要）
    │
    ▼
spawn_blocking {
    按 partition + offset 精确拉取单条消息
    pretty-print full_value
    Arc::new(full_value_string)
}
    │
    ▼
Message::DetailLoaded(Arc<String>)
    │
    ▼
state.detail_text = arc_string  （零 clone）
```

精确拉取：设置 consumer offset = target_offset，消费 1 条消息即停止。

---

## 六、改动文件清单

| 文件 | 改动内容 |
|------|----------|
| `Cargo.toml` | 添加 `rayon = "1"` |
| `src/kafka/types.rs` | 新增 `MessageSummary`、`MessageDetail`；`DecodedMessage` 保留用于翻页 |
| `src/kafka/consumer.rs` | `search_topic` 改为 rayon 并行 + streaming；新增 `fetch_single_message`；扫描限制逻辑 |
| `src/state/table_state.rs` | `detail_content` → `Arc<String>`；`search_results` → `Arc<Vec<MessageSummary>>`；`apply_search_page` 零 clone |
| `src/message.rs` | 新增 `SearchProgress` 消息变体 |
| `src/app.rs` | 处理 `SearchProgress`；流式搜索 Subscription；detail 按需拉取逻辑 |
| `src/ui/message_table.rs` | 直接使用预计算字段；`highlight_search` 快速路径 |
| `src/ui/message_detail.rs` | `text_editor` → `scrollable(text(...))` |
| `src/ui/syntax.rs` | `find_match_ranges` 改为快速字节扫描 |

---

## 七、性能预期

| 指标 | 优化前 | 优化后 |
|------|--------|--------|
| 搜索 100 万条（8 分区）| 串行，极慢 | 并行 8x，且流式显示 |
| 搜索结果内存（100 万条）| OOM | ~500MB（每条 ~500B） |
| 点击详情（10MB JSON）| UI 卡死 | 后台准备，UI 零阻塞 |
| 每帧渲染开销 | ~500 次 Vec 分配 | ~0 分配（预计算） |
| TableState Clone | 复制 rope + Vec | 原子计数 +1 |
