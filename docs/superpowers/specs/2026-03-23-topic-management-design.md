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

- `AppState` 新增 `overlay: OverlayState` 字段，统一管理所有弹窗状态
- 主 `view()` 使用 `iced::widget::stack!` 在最顶层渲染浮层，避免被 scrollable 裁剪
- 点击遮罩层或取消按钮触发 `Message::CloseOverlay`

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
    },

    TopicConfig {
        topic: String,
        retention_secs: String,   // 秒，-1 无限制
        retention_gb: String,     // GB，-1 无限制
        loading: bool,            // 正在拉取当前配置
        saving: bool,
    },
}

pub enum ContextMenuTarget {
    Topic(String),
    Partition { topic: String, partition: i32 },
}
```

### 3.2 新增 Message 变体（`src/message.rs`）

```rust
// 右键菜单
ShowContextMenu { x: f32, y: f32, target: ContextMenuTarget },
CloseOverlay,

// 发送消息
OpenSendMessage { topic: String, partition: i32 },
SendMessageInputChanged(String),
SendMessages,
MessagesSent(Result<usize, String>),   // 成功发送条数

// Topic 配置
OpenTopicConfig(String),
TopicConfigLoaded(Result<(String, String), String>),  // (retention_secs, retention_gb)
TopicConfigRetentionSecsChanged(String),
TopicConfigRetentionGbChanged(String),
SaveTopicConfig,
TopicConfigSaved(Result<(), String>),

// 清空数据
PurgeTopicData(String),
TopicDataPurged(Result<usize, String>),  // 清空的消息条数
```

---

## 4. Kafka 后端模块

### 4.1 生产者（`src/kafka/producer.rs`）

使用 `rdkafka::producer::BaseProducer`（同步，零额外线程开销）。

```rust
/// 向指定 topic/partition 批量发送消息，返回成功发送条数
pub fn send_messages(
    config: &ConnectionConfig,
    topic: &str,
    partition: i32,
    payloads: &[Vec<u8>],
) -> Result<usize>
```

- 输入 JSON 解析：根节点为 `Array` 则逐元素发送，根节点为 `Object` 或原始类型则发单条
- 每条消息序列化为 UTF-8 字节，key 为空
- 发送完成后调用 `producer.flush(Duration::from_secs(5))`

### 4.2 管理客户端（`src/kafka/admin.rs`）

使用 `rdkafka::admin::AdminClient`。

```rust
/// 拉取 topic 的 retention.ms 和 retention.bytes 配置
/// 返回 (retention_secs: String, retention_gb: String)，-1 表示无限制
pub fn describe_topic_config(
    config: &ConnectionConfig,
    topic: &str,
) -> Result<(String, String)>

/// 修改 topic 的 retention.ms 和 retention.bytes
/// retention_secs: 秒（-1 无限制），retention_gb: GB（-1 无限制）
pub fn alter_topic_config(
    config: &ConnectionConfig,
    topic: &str,
    retention_secs: i64,
    retention_gb: f64,
) -> Result<()>

/// 清空 topic 所有分区的数据（delete_records 到 high watermark）
/// 返回清空的消息总条数
pub fn purge_topic(
    config: &ConnectionConfig,
    topic: &str,
    partitions: &[i32],
) -> Result<usize>
```

单位转换：
- 展示秒 → 提交 ms：`retention_secs * 1000`
- 展示 GB → 提交 bytes：`(retention_gb * 1024^3) as i64`，`-1` 保持 `-1`

---

## 5. UI 交互

### 5.1 右键菜单捕获

在 `src/ui/sidebar.rs` 的 topic 和 partition 按钮上，通过 `iced` 的鼠标事件监听右键：

- 右键 Topic → `ShowContextMenu { target: ContextMenuTarget::Topic(name) }`
- 右键 Partition → `ShowContextMenu { target: ContextMenuTarget::Partition { topic, partition } }`

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

浮层位置：以右键坐标为基点，自动避免超出窗口边界。
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
│  [错误提示（红色，JSON 解析失败时）]   │
│                  [取消]  [发送 (N条)] │
└─────────────────────────────────────┘
```

- 输入框实时解析 JSON，底部按钮实时显示条数
- 发送中禁用按钮，显示"发送中..."
- 发送成功：关闭弹窗，状态栏显示"成功发送 N 条消息到 topic/P-x"
- 发送失败：弹窗内显示错误，不关闭

### 5.4 Topic 配置弹窗

```
┌─────────────────────────────────────┐
│  Topic 配置 - topic-name             │
├─────────────────────────────────────┤
│  保留时间（秒）    [___604800___]     │
│  最大磁盘占用（GB）[_____-1_____]    │
│                        (-1 表示无限制)│
├─────────────────────────────────────┤
│               [取消]  [保存配置]      │
│  ─────────────────────────────────  │
│  危险操作                            │
│  [清空当前堆积数据]                   │
└─────────────────────────────────────┘
```

- 打开时立即异步拉取当前配置预填充
- 拉取中显示加载状态（输入框 placeholder 显示"加载中..."）
- 保存中禁用按钮
- 清空数据按钮点击后状态栏显示确认提示，二次点击确认执行

---

## 6. 主视图渲染

```rust
// src/app.rs view()
let base = row![sidebar, main_content];

match &self.state.overlay {
    OverlayState::None => base.into(),
    _ => stack![
        base,
        overlay_backdrop(),                    // 全屏半透明遮罩
        render_overlay(&self.state.overlay),   // 居中或鼠标位置弹窗
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
| `src/state/app_state.rs` | 修改（添加 overlay 字段） |
| `src/message.rs` | 修改（新增 Message 变体） |
| `src/app.rs` | 修改（处理新消息 + Stack 渲染） |
| `src/ui/sidebar.rs` | 修改（捕获右键事件） |

---

## 8. 性能要点

- `BaseProducer` 同步发送，无额外线程，批量时逐条 `send` + 最终 `flush`，高吞吐
- `AdminClient` 操作通过 `Task::perform` 异步执行，不阻塞 UI 线程
- `describe_topic_config` 仅在打开配置弹窗时调用一次，结果缓存在 `OverlayState` 中
- `purge_topic` 通过 `delete_records` 直接设置 offset，无需修改 retention，操作原子高效
