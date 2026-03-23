# Topic 管理功能设计文档

**日期**：2026-03-23
**状态**：已确认

---

## 1. 功能概述

新增三项 Topic/Partition 管理功能，统一通过右键浮层菜单触发：

1. **发送消息**（Partition 级别）：向指定 partition 发送单条或批量 JSON 消息
2. **Topic 配置**（Topic 级别）：调整保留时间（秒）和最大磁盘占用（GB），预填充当前值
3. **清空数据**（Topic 级别）：通过 `delete_records` 清空所有 partition 堆积数据

---

## 2. 架构方案

采用**全局 overlay 状态机**方案：

- `AppState` 新增 `overlay: OverlayState` 字段和 `window_size: (f32, f32)` 字段
- 主 `view()` 使用 `iced::widget::stack!` 在最顶层渲染浮层，避免被 scrollable 裁剪
- 点击遮罩层或取消按钮触发 `Message::CloseOverlay`
- 窗口尺寸通过 `Subscription` 监听 `window::Event::Resized` 同步到 `AppState::window_size`

---

## 3. 状态模型

### 3.1 OverlayState（`src/state/overlay_state.rs`）

```rust
pub enum OverlayState {
    None,

    ContextMenu {
        x: f32,
        y: f32,
        target: ContextMenuTarget,
    },

    SendMessage {
        topic: String,
        partition: i32,
        input: String,
        sending: bool,
        error: Option<String>,
    },

    TopicConfig {
        topic: String,
        partitions: Vec<i32>,
        retention_secs: String,           // 秒，只允许正整数或 -1
        retention_gb: String,             // GB，只允许正整数或 -1
        retention_gb_note: Option<String>, // 非整数 GB 时的说明，如"原值 2000000000 bytes，已取整"
        loading: bool,
        saving: bool,
        error: Option<String>,
        purge_confirm_pending: bool,
        purging: bool,
    },
}

/// TopicConfig 状态互斥约束：
/// - loading=true 时：saving/purge_confirm_pending/purging 均为 false，禁用所有按钮
/// - saving=true 时：purge_confirm_pending 重置为 false，禁用所有按钮
/// - purging=true 时：purge_confirm_pending=false，禁用所有按钮
/// - saving 与 purging 不能同时为 true

/// ContextMenuTarget::Topic 携带 partitions，使 context_menu view() 在构造
/// OpenTopicConfig 消息时无需反查 SidebarState
pub enum ContextMenuTarget {
    Topic {
        name: String,
        partitions: Vec<i32>,  // 从 sidebar 渲染时的 TopicMeta 取出，随右键事件传入
    },
    Partition {
        topic: String,
        partition: i32,
    },
}
```

### 3.2 新增 Message 变体（`src/message.rs`）

```rust
// 右键菜单
ShowContextMenu { x: f32, y: f32, target: ContextMenuTarget },
CloseOverlay,

// 窗口尺寸同步
WindowResized(f32, f32),

// 发送消息
OpenSendMessage { topic: String, partition: i32 },
SendMessageInputChanged(String),
SendMessages,
MessagesSent(Result<usize, String>),

// Topic 配置
OpenTopicConfig { topic: String, partitions: Vec<i32> },
TopicConfigLoaded(Result<(String, String, Option<String>), String>),
// (retention_secs, retention_gb, retention_gb_note)
TopicConfigRetentionSecsChanged(String),
TopicConfigRetentionGbChanged(String),
SaveTopicConfig,
TopicConfigSaved(Result<(), String>),

// 清空数据
RequestPurgeTopicData,
ConfirmPurgeTopicData,
TopicDataPurged(Result<(), String>),
```

---

## 4. Kafka 后端模块

### 4.1 生产者（`src/kafka/producer.rs`）

使用 `rdkafka::producer::BaseProducer`。`send_messages` 是同步阻塞函数，通过 `tokio::task::spawn_blocking` 调度到阻塞线程池：

```rust
// app.rs 中调用示例
Task::perform(
    async move {
        tokio::task::spawn_blocking(move || {
            producer::send_messages(config, topic, partition, payloads)
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!("task panic: {e}")))
        .map_err(|e| e.to_string())   // 转为 Result<usize, String> 对应 MessagesSent
    },
    Message::MessagesSent,
)
```

```rust
/// 向指定 topic/partition 批量发送消息，返回成功发送条数
/// 阻塞函数，必须通过 tokio::task::spawn_blocking 调用
pub fn send_messages(
    config: ConnectionConfig,
    topic: String,
    partition: i32,
    payloads: Vec<Vec<u8>>,
) -> Result<usize>
```

实现要点：
- 输入 JSON 解析：根节点为 `Array` 则逐元素发送，根节点为 `Object` 或原始类型则发单条
- 每条消息序列化为 UTF-8 字节，key 为空
- `BaseProducer::send` 遇到 `QueueFull` 时，调用 `producer.poll(Duration::from_millis(100))` 后重试，最多重试 3 次，超出则返回错误；计数器仅在 `send` 最终成功后自增，不在重试过程中计数
- 全部 `send` 成功后调用 `producer.flush(Duration::from_secs(5))` 确保投递

### 4.2 管理客户端（`src/kafka/admin.rs`）

使用 `rdkafka::admin::AdminClient`（原生 async，无需 spawn_blocking）。通过 `Task::perform(async move { ... })` 调用，所有参数按值传入满足 `'static` 约束。

```rust
/// 拉取 topic 的 retention.ms 和 retention.bytes 配置
/// 返回 (retention_secs, retention_gb, retention_gb_note)
/// - retention.ms / 1000 取整 → retention_secs 字符串
/// - retention.bytes / 1,073,741,824 四舍五入 → retention_gb 字符串
/// - 若 retention.bytes 不是 GB 整数倍，retention_gb_note = Some("原值 X bytes，已取整")
/// - 原值为 -1 则返回 "-1"，note 为 None
pub async fn describe_topic_config(
    config: ConnectionConfig,
    topic: String,
) -> Result<(String, String, Option<String>)>

/// 修改 topic 的 retention.ms 和 retention.bytes
/// retention_ms: 毫秒（-1 无限制），retention_bytes: bytes（-1 无限制）
pub async fn alter_topic_config(
    config: ConnectionConfig,
    topic: String,
    retention_ms: i64,
    retention_bytes: i64,
) -> Result<()>

/// 清空 topic 所有分区的数据（delete_records 到 high watermark）
pub async fn purge_topic(
    config: ConnectionConfig,
    topic: String,
    partitions: Vec<i32>,
) -> Result<()>
```

---

## 5. UI 交互

### 5.1 右键菜单捕获与 partitions 数据来源

使用 `iced::widget::mouse_area` 包裹 topic/partition 按钮，通过 `on_right_press_with` 捕获坐标。`partitions` 在 sidebar `view()` 中从 `TopicMeta` 直接取出，随右键消息一同传入，使 context_menu `view()` 构造 `OpenTopicConfig` 时无需反查状态：

```rust
let partition_ids: Vec<i32> = topic.partitions.iter().map(|p| p.id).collect();

mouse_area(topic_button)
    .on_right_press_with(move |point| Message::ShowContextMenu {
        x: point.x,
        y: point.y,
        target: ContextMenuTarget::Topic {
            name: name.clone(),
            partitions: partition_ids.clone(),
        },
    })
```

### 5.2 右键菜单内容

**右键 Topic（3 项）：**
```
⚙  Topic 配置
   清空数据
✕  关闭
```

**右键 Partition（2 项）：**
```
✉  发送消息
✕  关闭
```

**浮层位置（`view()` 中用 `AppState::window_size` 计算）：**
- 水平：`x + 菜单宽度 > window_width` 时，`x = x - 菜单宽度`
- 垂直：`y + 菜单高度 > window_height` 时，`y = y - 菜单高度`

点击菜单外遮罩层 → `CloseOverlay`。

### 5.3 发送消息弹窗

```
┌─────────────────────────────────────┐
│  发送消息到 topic-name / P-0          │
├─────────────────────────────────────┤
│  ┌─────────────────────────────┐    │
│  │ (多行文本输入框)              │    │
│  │ 支持单条 {} 或数组 [{},{}]   │    │
│  └─────────────────────────────┘    │
│  [错误提示（红色，JSON 解析失败或发送失败时）] │
│                  [取消]  [发送 (N条)] │
└─────────────────────────────────────┘
```

- 输入框实时解析 JSON，底部按钮实时显示条数；JSON 无效时禁用发送按钮并显示解析错误
- 发送中禁用按钮（`sending: true`）
- 发送成功：关闭弹窗，状态栏显示"成功发送 N 条消息到 topic/P-x"
- 发送失败：`sending=false`，`error=Some(msg)`，弹窗内显示红色错误，不关闭

### 5.4 Topic 配置弹窗

```
┌─────────────────────────────────────┐
│  Topic 配置 - topic-name             │
├─────────────────────────────────────┤
│  保留时间（秒）    [___604800___]     │
│  最大磁盘占用（GB）[_____1_____]     │
│  ℹ 原值 2000000000 bytes，已取整     │  ← retention_gb_note 不为 None 时显示
│                        (-1 表示无限制)│
│  [输入验证错误提示（红色）]            │
├─────────────────────────────────────┤
│               [取消]  [保存配置]      │
│  ─────────────────────────────────  │
│  危险操作                            │
│  [清空当前堆积数据]                   │
│  [确认清空？点击确认执行]             │  ← purge_confirm_pending=true 时
│  [错误提示（红色，保存/清空失败时）]   │
└─────────────────────────────────────┘
```

**输入验证**：`retention_secs` 和 `retention_gb` 只允许正整数或 `-1`，其他输入禁用保存并显示红色提示。`retention_gb_note` 为只读展示字段，不参与验证。

**状态流转**：
- 打开时：`loading=true`，`Task::perform` 异步拉取配置；所有按钮禁用
- 拉取完成：`loading=false`，填充 `retention_secs`、`retention_gb`、`retention_gb_note`
- 点击保存：`saving=true`，`purge_confirm_pending=false`，禁用所有按钮；成功关闭弹窗；失败 `saving=false`，`error=Some(msg)`
- 点击"清空当前堆积数据"：`purge_confirm_pending=true`
- 点击确认：`purge_confirm_pending=false`，`purging=true`，执行清空
- 清空成功：关闭弹窗，状态栏显示"已清空 topic-name 的全部数据"
- 清空失败：`purging=false`，`error=Some(msg)`，不关闭，可重试

---

## 6. 主视图渲染

```rust
// src/app.rs view()
let base = row![sidebar, main_content];

match &self.state.overlay {
    OverlayState::None => base.into(),
    _ => stack![
        base,
        overlay_backdrop(),
        render_overlay(&self.state.overlay, self.state.window_size),
    ].into(),
}
```

---

## 7. 新增/修改文件清单

| 文件 | 变更 |
|------|------|
| `src/kafka/producer.rs` | 新增 |
| `src/kafka/admin.rs` | 新增 |
| `src/kafka/mod.rs` | 修改（导出新模块） |
| `src/ui/context_menu.rs` | 新增 |
| `src/ui/send_message_dialog.rs` | 新增 |
| `src/ui/topic_config_dialog.rs` | 新增 |
| `src/ui/mod.rs` | 修改（导出新模块） |
| `src/state/overlay_state.rs` | 新增 |
| `src/state/mod.rs` | 修改（导出 OverlayState） |
| `src/state/app_state.rs` | 修改（添加 overlay、window_size 字段） |
| `src/message.rs` | 修改（新增 Message 变体） |
| `src/app.rs` | 修改（处理新消息 + Stack 渲染 + WindowResized 订阅） |
| `src/ui/sidebar.rs` | 修改（mouse_area + on_right_press_with 捕获右键坐标） |

---

## 8. 性能与正确性要点

- `send_messages` 通过 `tokio::task::spawn_blocking` 调度到阻塞线程池；async block 末尾 `.map_err(|e| e.to_string())` 确保类型匹配 `Message::MessagesSent(Result<usize, String>)`
- AdminClient 原生 async，直接 `Task::perform(async move { ... })` 无需 spawn_blocking
- `ConnectionConfig` 在发起 `Task::perform` 前 clone，满足 `'static` 约束
- 发送计数仅在 `send` 成功后自增（不在 QueueFull 重试中计数）
- GB→bytes 换算：`gb_val_i64.checked_mul(1_073_741_824_i64)`，结果为 `None` 时显示溢出错误并禁止保存；`-1` 跳过换算直接传 `-1`
- `describe_topic_config` 返回 GB 时四舍五入取整，非整数情况通过独立 `retention_gb_note` 字段显示说明，不污染可编辑的 `retention_gb` 字段
- 窗口尺寸通过 Subscription 维护，菜单边界计算在 `view()` 中零额外开销完成
