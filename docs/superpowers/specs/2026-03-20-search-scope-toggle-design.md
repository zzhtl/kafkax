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
/// 切换"全分区搜索"开关
ToggleSearchAllPartitions,
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
        Some(p) => vec![p],   // 选中了具体分区，只搜该分区
        None    => all_partitions,  // 只选了 topic，搜全部
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

使用 iced 的 `checkbox` widget：
```rust
checkbox("全分区搜索", state.search_all_partitions)
    .on_toggle(|_| Message::ToggleSearchAllPartitions)
    .size(14)
```

### 5.2 搜索框 Placeholder 更新

根据当前状态动态显示占位符文本：

```rust
let search_placeholder = if table.has_search_results() || table.search_in_progress {
    "正在显示搜索结果"
} else if !table.search_all_partitions {
    if let Some(p) = sidebar.selected_partition {
        // 动态格式化：需在 view 函数中用 format! 生成，传入 toolbar::view
        &format!("输入关键词后回车搜索 Partition {p}")
    } else {
        "输入关键词后回车搜索全部分区"
    }
} else {
    "输入关键词后回车搜索全部分区"
};
```

> 注：由于 iced text_input 的 placeholder 需要 `&str`，format! 字符串须在 view 函数中提前生成并传入。

---

## 六、涉及文件

| 文件 | 改动内容 |
|------|----------|
| `src/message.rs` | 新增 `ToggleSearchAllPartitions` 变体 |
| `src/state/table_state.rs` | 新增 `search_all_partitions: bool` 字段，Default 为 `false` |
| `src/ui/toolbar.rs` | 加 `checkbox` 控件；更新 placeholder 逻辑；toolbar::view 接收 `sidebar.selected_partition` |
| `src/app.rs` | 处理 `ToggleSearchAllPartitions` 消息；更新 `begin_topic_search` 分区选择逻辑 |

---

## 七、边界情况

- **切换 checkbox 时若搜索词非空**：不自动重新搜索，仅更新状态。下次按 Enter 时生效。
- **切换分区时**：`begin_topic_search` 会被自动调用（现有逻辑），新的分区选择逻辑自然生效。
- **断开连接后重连**：`table` 被重置为 `Default`，`search_all_partitions` 回到 `false`，符合预期。
