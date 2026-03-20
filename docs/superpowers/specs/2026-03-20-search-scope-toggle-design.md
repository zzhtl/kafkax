# KafkaX 搜索范围切换设计文档

**日期**: 2026-03-20
**目标**: 在搜索框旁加 checkbox，支持"只搜当前分区"与"搜索全部分区"两种模式

---

## 一、背景与问题

当前全分区搜索在数据量过大时会触发扫描上限（100 万条 / 2GB），可能导致部分消息未被扫描。用户希望在确定目标分区时可以缩小搜索范围，提高精准度和速度。

---

## 二、行为逻辑

| 场景 | checkbox 未勾选（默认） | checkbox 勾选 |
|------|------------------------|--------------|
| 左侧选中了 Topic（未选具体分区） | 搜索该 Topic 全部分区 | 搜索该 Topic 全部分区 |
| 左侧选中了某个具体 Partition | **只搜该 Partition** | 搜索该 Topic 全部分区 |

checkbox 仅在选中具体分区时才影响搜索行为；选中 Topic 时两种状态等价，均搜全部分区。

---

## 三、状态变化

### 3.1 TableState 新增字段

```rust
/// 是否强制搜索全部分区（false = 智能判断：选 topic 搜全部，选 partition 搜单个）
pub search_all_partitions: bool,  // Default: false
```

`Default` impl 中初始化为 `false`。

`clear_search_results` 和 `apply_page_data` 等方法**不重置**该字段——它是用户偏好，应在多次搜索间保持。

### 3.2 新增 Message 变体

```rust
/// 设置"全分区搜索"开关状态（直接传入目标值，避免多次 toggle 竞态）
SetSearchAllPartitions(bool),
```

---

## 四、搜索分区选择逻辑

`app.rs` 中 `begin_topic_search` 替换当前固定取全部分区的逻辑：

```rust
// 旧：始终取 topic 下全部分区
let Some((topic, partitions)) = self.selected_topic_partitions() else { ... };

// 新：根据开关和侧边栏选中状态决定分区列表
let Some((topic, all_partitions)) = self.selected_topic_partitions() else { ... };

let partitions = if self.state.table.search_all_partitions {
    // 强制全部分区
    all_partitions
} else {
    match self.state.sidebar.selected_partition {
        Some(p) => vec![p],       // 选中了具体分区，只搜该分区
        None    => all_partitions, // 只选了 topic，搜全部
    }
};
```

---

## 五、UI 变化

### 5.1 Toolbar 布局

在搜索框右侧、排序按钮左侧加入 checkbox：

```
[搜索框________________] [☐ 全分区搜索]  [排序 ▾]  [每页 ▾]
```

使用 iced 的 `checkbox` widget，直接传递目标 bool 值：

```rust
checkbox("全分区搜索", state.search_all_partitions)
    .on_toggle(Message::SetSearchAllPartitions)
    .size(14)
```

### 5.2 搜索框 Placeholder 更新

`toolbar::view` 新增参数 `selected_partition: Option<i32>`，在函数内部提前生成占位符字符串以避免悬垂引用：

```rust
pub fn view(
    table: &TableState,
    decoder_type: DecoderType,
    selected_partition: Option<i32>,
) -> Element<'_, Message> {
    let placeholder = if table.has_search_results() || table.search_in_progress {
        "正在显示搜索结果".to_string()
    } else if !table.search_all_partitions {
        match selected_partition {
            Some(p) => format!("输入关键词后回车搜索 Partition {p}"),
            None    => "输入关键词后回车搜索全部分区".to_string(),
        }
    } else {
        "输入关键词后回车搜索全部分区".to_string()
    };

    let search_input = text_input(&placeholder, &table.search_query)
        .on_input(Message::SearchInputChanged)
        .on_submit(Message::Search)
        // ...
```

`app.rs` 的 `view()` 方法中更新调用签名：

```rust
// 旧
let toolbar = ui::toolbar::view(&self.state.table, self.state.decoder.selected);

// 新
let toolbar = ui::toolbar::view(
    &self.state.table,
    self.state.decoder.selected,
    self.state.sidebar.selected_partition,
);
```

---

## 六、涉及文件

| 文件 | 改动内容 |
|------|----------|
| `src/message.rs` | 新增 `SetSearchAllPartitions(bool)` 变体 |
| `src/state/table_state.rs` | 新增 `search_all_partitions: bool` 字段，Default 为 `false` |
| `src/ui/toolbar.rs` | 新增 `selected_partition: Option<i32>` 参数；加 `checkbox` 控件；更新 placeholder 逻辑 |
| `src/app.rs` | 处理 `SetSearchAllPartitions(bool)` 消息；更新 `begin_topic_search` 分区选择逻辑；更新 `view()` 中 `toolbar::view` 调用签名，传入 `self.state.sidebar.selected_partition` |

---

## 七、边界情况

- **切换 checkbox 时若搜索词非空**：不自动重新搜索，仅更新状态。下次按 Enter 时生效。
- **切换分区时**：`begin_topic_search` 会被自动调用（现有逻辑），新的分区选择逻辑自然生效。
- **断开连接后重连**：`table` 被重置为 `Default`，`search_all_partitions` 回到 `false`，符合预期。
- **搜索进行中切换 checkbox**：`search_in_progress` 为 `true` 时切换开关不影响正在执行的搜索（分区列表已在启动时固定），仅改变下次搜索行为。实现上无需特殊处理。
- **勾选状态下从 partition 切换到 topic**：`selected_partition` 变为 `None`，checkbox 仍显示为勾选，但此时勾选与不勾选效果相同（均搜全部分区），无功能性问题，不需要自动取消勾选。
