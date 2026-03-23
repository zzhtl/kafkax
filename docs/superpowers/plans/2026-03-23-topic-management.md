# Topic 管理功能实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 通过右键浮层菜单向指定 Partition 发送 JSON 消息、修改 Topic retention 配置、清空 Topic 堆积数据。

**Architecture:** 全局 `OverlayState` 枚举维护所有弹窗状态，`app.rs view()` 使用已有的 `stack!` 模式渲染浮层。Kafka 写操作（`BaseProducer`、`AdminClient`）均通过 `Task::perform + spawn_blocking/async` 在后台执行，不阻塞 UI。

**Tech Stack:** Rust, Iced 0.14, rdkafka 0.39（AdminClient 已包含在 crate 中，无需新增 feature）

---

## 文件职责一览

| 文件 | 新增/修改 | 职责 |
|------|---------|------|
| `src/state/overlay_state.rs` | 新增 | `OverlayState` 枚举 + `ContextMenuTarget` + 输入验证 helper |
| `src/state/mod.rs` | 修改 | pub use overlay_state::* |
| `src/state/app_state.rs` | 修改 | 添加 `overlay: OverlayState`, `window_size: (f32, f32)`, `cursor_pos: (f32, f32)` |
| `src/message.rs` | 修改 | 新增右键菜单/弹窗相关 Message 变体 |
| `src/kafka/producer.rs` | 新增 | `send_messages`（同步，spawn_blocking 调用） |
| `src/kafka/admin.rs` | 新增 | `describe_topic_config`/`alter_topic_config`/`purge_topic`（async） |
| `src/kafka/mod.rs` | 修改 | pub mod producer; pub mod admin; |
| `src/ui/context_menu.rs` | 新增 | 右键浮层菜单渲染 |
| `src/ui/send_message_dialog.rs` | 新增 | 发送消息弹窗渲染 |
| `src/ui/topic_config_dialog.rs` | 新增 | Topic 配置弹窗渲染 |
| `src/ui/mod.rs` | 修改 | pub mod context_menu; pub mod send_message_dialog; pub mod topic_config_dialog; |
| `src/ui/sidebar.rs` | 修改 | mouse_area + on_move 追踪光标位置, on_right_press 触发右键菜单 |
| `src/app.rs` | 修改 | update() 处理新消息 + view() Stack 渲染 + subscription 窗口尺寸 |
| `tests/codec_tests.rs` | 不修改 | — |

---

## Task 1: OverlayState 状态模型

**Files:**
- Create: `src/state/overlay_state.rs`
- Modify: `src/state/mod.rs`

- [ ] **Step 1: 新建 overlay_state.rs，定义枚举和 helper**

```rust
// src/state/overlay_state.rs

/// 右键菜单目标对象
#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    Topic {
        name: String,
        partitions: Vec<i32>,
    },
    Partition {
        topic: String,
        partition: i32,
    },
}

/// 全局弹窗/浮层状态机
#[derive(Debug, Clone, Default)]
pub enum OverlayState {
    #[default]
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
        retention_secs: String,
        retention_gb: String,
        retention_gb_note: Option<String>,
        loading: bool,
        saving: bool,
        error: Option<String>,
        purge_confirm_pending: bool,
        purging: bool,
    },
}

/// 验证 retention 输入是否合法（正整数或 -1）
pub fn is_valid_retention_input(s: &str) -> bool {
    let s = s.trim();
    if s == "-1" {
        return true;
    }
    s.parse::<u64>().is_ok()
}

/// 将 retention_secs 字符串换算为毫秒 i64
/// -1 → -1，其余按 secs * 1000
pub fn secs_to_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    if s == "-1" {
        return Some(-1);
    }
    s.parse::<i64>().ok().map(|v| v * 1000)
}

/// 将 retention_gb 字符串换算为 bytes i64
/// -1 → -1，其余按 checked_mul(1_073_741_824)
pub fn gb_to_bytes(s: &str) -> Option<i64> {
    let s = s.trim();
    if s == "-1" {
        return Some(-1);
    }
    s.parse::<i64>()
        .ok()
        .and_then(|v| v.checked_mul(1_073_741_824_i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_retention_inputs() {
        assert!(is_valid_retention_input("-1"));
        assert!(is_valid_retention_input("0"));
        assert!(is_valid_retention_input("604800"));
        assert!(!is_valid_retention_input("abc"));
        assert!(!is_valid_retention_input("1.5"));
        assert!(!is_valid_retention_input("-2"));
        assert!(!is_valid_retention_input(""));
    }

    #[test]
    fn secs_to_ms_converts_correctly() {
        assert_eq!(secs_to_ms("-1"), Some(-1));
        assert_eq!(secs_to_ms("604800"), Some(604_800_000));
        assert_eq!(secs_to_ms("abc"), None);
    }

    #[test]
    fn gb_to_bytes_converts_correctly() {
        assert_eq!(gb_to_bytes("-1"), Some(-1));
        assert_eq!(gb_to_bytes("1"), Some(1_073_741_824));
        assert_eq!(gb_to_bytes("10"), Some(10_737_418_240));
        // 溢出应返回 None
        assert_eq!(gb_to_bytes("9999999999999"), None);
    }
}
```

- [ ] **Step 2: 修改 src/state/mod.rs，导出新模块**

在 `src/state/mod.rs` 中添加：
```rust
pub mod overlay_state;
pub use overlay_state::*;
```

- [ ] **Step 3: 运行测试验证通过**

```bash
cd /home/qingteng/rust-workspace/kafkax && cargo test overlay_state
```
期望：3 个测试全部 PASS

- [ ] **Step 4: Commit**

```bash
git add src/state/overlay_state.rs src/state/mod.rs
git commit -m "feat: 添加 OverlayState 状态模型和 retention 换算 helper"
```

---

## Task 2: 更新 AppState 和 Message

**Files:**
- Modify: `src/state/app_state.rs`
- Modify: `src/message.rs`

- [ ] **Step 1: 修改 AppState，添加 overlay 和 window_size 字段**

在 `src/state/app_state.rs` 中：
1. 引入 `use crate::state::overlay_state::OverlayState;`
2. 在 `AppState` struct 添加三个字段：
```rust
pub overlay: OverlayState,
pub window_size: (f32, f32),
pub cursor_pos: (f32, f32),
```
3. 在 `Default::default()` 中初始化：
```rust
overlay: OverlayState::None,
window_size: (1280.0, 800.0),
cursor_pos: (0.0, 0.0),
```

- [ ] **Step 2: 修改 message.rs，新增 Message 变体**

在 `src/message.rs` 顶部导入：
```rust
use crate::state::overlay_state::ContextMenuTarget;
```

在 `Message` 枚举末尾（`Noop` 之前）添加：
```rust
// --- 右键菜单 ---
/// 光标移动（用于记录右键触发时的坐标）
CursorMoved(f32, f32),
/// 显示右键浮层菜单（坐标从 AppState::cursor_pos 读取）
ShowContextMenu { target: ContextMenuTarget },
/// 关闭任意浮层
CloseOverlay,
/// 窗口尺寸变化
WindowResized(f32, f32),

// --- 发送消息 ---
/// 打开发送消息弹窗（从右键菜单触发）
OpenSendMessage { topic: String, partition: i32 },
/// 输入框内容变化
SendMessageInputChanged(String),
/// 执行发送
SendMessages,
/// 发送完成（后台回调）
MessagesSent(Result<usize, String>),

// --- Topic 配置 ---
/// 打开 Topic 配置弹窗（从右键菜单触发）
OpenTopicConfig { topic: String, partitions: Vec<i32> },
/// 配置拉取完成（retention_secs, retention_gb, retention_gb_note）
TopicConfigLoaded(Result<(String, String, Option<String>), String>),
/// 保留时间输入变化
TopicConfigRetentionSecsChanged(String),
/// 磁盘占用输入变化
TopicConfigRetentionGbChanged(String),
/// 保存配置
SaveTopicConfig,
/// 保存完成
TopicConfigSaved(Result<(), String>),
/// 第一次点击清空：等待二次确认
RequestPurgeTopicData,
/// 第二次点击确认清空
ConfirmPurgeTopicData,
/// 清空完成
TopicDataPurged(Result<(), String>),
```

- [ ] **Step 3: 编译验证**

```bash
cd /home/qingteng/rust-workspace/kafkax && cargo build 2>&1 | head -40
```
期望：编译成功（可能有 dead_code warning，属正常）

- [ ] **Step 4: Commit**

```bash
git add src/state/app_state.rs src/message.rs
git commit -m "feat: AppState 添加 overlay/window_size 字段，Message 添加浮层相关变体"
```

---

## Task 3: Kafka 生产者模块

**Files:**
- Create: `src/kafka/producer.rs`
- Modify: `src/kafka/mod.rs`

- [ ] **Step 1: 新建 producer.rs，先写测试**

```rust
// src/kafka/producer.rs
use anyhow::{Context, Result};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{BaseProducer, BaseRecord, Producer};
use serde_json::Value;
use std::time::Duration;

use crate::config::ConnectionConfig;

/// 将 JSON 字符串解析为待发送的消息列表
/// Array → 多条，Object/其他 → 单条
pub fn parse_json_to_payloads(input: &str) -> Result<Vec<Vec<u8>>> {
    let value: Value = serde_json::from_str(input.trim())
        .context("JSON 解析失败")?;
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| {
                serde_json::to_vec(item).context("序列化 JSON 元素失败")
            })
            .collect(),
        other => Ok(vec![serde_json::to_vec(&other).context("序列化 JSON 失败")?]),
    }
}

/// 向指定 topic/partition 批量发送消息，返回成功发送条数
/// 阻塞函数，必须通过 tokio::task::spawn_blocking 调用
pub fn send_messages(
    config: ConnectionConfig,
    topic: String,
    partition: i32,
    payloads: Vec<Vec<u8>>,
) -> Result<usize> {
    let producer: BaseProducer = build_producer_config(&config)?.create()?;
    let mut sent = 0usize;

    for payload in &payloads {
        let mut retries = 0usize;
        loop {
            let record = BaseRecord::<(), [u8]>::to(&topic)
                .partition(partition)
                .payload(payload.as_slice());

            match producer.send(record) {
                Ok(()) => {
                    sent += 1;
                    break;
                }
                Err((rdkafka::error::KafkaError::MessageProduction(
                    rdkafka::types::RDKafkaErrorCode::QueueFull,
                ), _)) => {
                    if retries >= 3 {
                        anyhow::bail!("发送队列持续满，已重试 3 次，中止发送");
                    }
                    producer.poll(Duration::from_millis(100));
                    retries += 1;
                }
                Err((e, _)) => {
                    return Err(anyhow::anyhow!("发送失败: {e}"));
                }
            }
        }
    }

    producer.flush(Duration::from_secs(5)).context("flush 超时")?;
    Ok(sent)
}

fn build_producer_config(config: &ConnectionConfig) -> Result<ClientConfig> {
    use crate::config::SecurityProtocol;
    let mut client_config = ClientConfig::new();
    client_config
        .set("bootstrap.servers", &config.brokers)
        .set("security.protocol", config.security_protocol.as_str());

    if let Some(sasl) = &config.sasl {
        client_config
            .set("sasl.mechanism", sasl.mechanism.as_str())
            .set("sasl.username", &sasl.username)
            .set("sasl.password", &sasl.password);
    }

    if matches!(
        config.security_protocol,
        SecurityProtocol::Ssl | SecurityProtocol::SaslSsl
    ) {
        if let Some(ssl) = &config.ssl {
            if let Some(ca) = &ssl.ca_location {
                client_config.set("ssl.ca.location", ca);
            }
            if let Some(cert) = &ssl.cert_location {
                client_config.set("ssl.certificate.location", cert);
            }
            if let Some(key) = &ssl.key_location {
                client_config.set("ssl.key.location", key);
            }
            if let Some(pwd) = &ssl.key_password {
                client_config.set("ssl.key.password", pwd);
            }
        }
    }
    Ok(client_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_object_returns_single_payload() {
        let payloads = parse_json_to_payloads(r#"{"key":"value"}"#).unwrap();
        assert_eq!(payloads.len(), 1);
        let v: Value = serde_json::from_slice(&payloads[0]).unwrap();
        assert_eq!(v["key"], "value");
    }

    #[test]
    fn parse_array_returns_multiple_payloads() {
        let payloads = parse_json_to_payloads(r#"[{"a":1},{"b":2}]"#).unwrap();
        assert_eq!(payloads.len(), 2);
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        assert!(parse_json_to_payloads("not json").is_err());
    }

    #[test]
    fn parse_empty_array_returns_zero_payloads() {
        let payloads = parse_json_to_payloads("[]").unwrap();
        assert_eq!(payloads.len(), 0);
    }
}
```

- [ ] **Step 2: 修改 src/kafka/mod.rs 导出新模块**

在 `src/kafka/mod.rs` 中追加：
```rust
pub mod admin;
pub mod producer;
```

- [ ] **Step 3: 运行单元测试**

```bash
cd /home/qingteng/rust-workspace/kafkax && cargo test producer
```
期望：4 个测试全部 PASS

- [ ] **Step 4: Commit**

```bash
git add src/kafka/producer.rs src/kafka/mod.rs
git commit -m "feat: 添加 Kafka 生产者模块，支持单条/批量 JSON 发送"
```

---

## Task 4: Kafka 管理客户端模块

**Files:**
- Create: `src/kafka/admin.rs`

- [ ] **Step 1: 新建 admin.rs**

```rust
// src/kafka/admin.rs
use anyhow::{Context, Result};
use rdkafka::admin::{AdminClient, AdminOptions, AlterConfig, ResourceSpecifier};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::TopicPartitionList;
use rdkafka::topic_partition_list::Offset;
use std::time::Duration;

use crate::config::ConnectionConfig;

/// 拉取 topic 的 retention.ms 和 retention.bytes 配置
/// 返回 (retention_secs, retention_gb, retention_gb_note)
/// retention_gb_note：若 retention.bytes 不是 GB 整数倍，返回原始 bytes 说明
pub async fn describe_topic_config(
    config: ConnectionConfig,
    topic: String,
) -> Result<(String, String, Option<String>)> {
    let admin = build_admin(&config)?;
    let resource = ResourceSpecifier::Topic(topic.as_str());
    let results = admin
        .describe_configs(&[&resource], &AdminOptions::new().request_timeout(Duration::from_secs(10)))
        .await
        .context("describe_configs 失败")?;

    let topic_result = results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("未收到配置结果"))?
        .map_err(|e| anyhow::anyhow!("获取配置失败: {e}"))?;

    let get = |key: &str| -> Option<String> {
        topic_result.get(key).and_then(|e| e.value.clone())
    };

    // retention.ms → 秒
    let retention_secs = match get("retention.ms").as_deref() {
        Some("-1") | None => "-1".to_string(),
        Some(ms_str) => ms_str
            .parse::<i64>()
            .map(|ms| (ms / 1000).to_string())
            .unwrap_or_else(|_| "-1".to_string()),
    };

    // retention.bytes → GB（四舍五入）
    let (retention_gb, retention_gb_note) = match get("retention.bytes").as_deref() {
        Some("-1") | None => ("-1".to_string(), None),
        Some(bytes_str) => {
            match bytes_str.parse::<i64>() {
                Ok(bytes) => {
                    let gb = (bytes as f64 / 1_073_741_824.0).round() as i64;
                    let note = if bytes % 1_073_741_824 != 0 {
                        Some(format!("原值 {} bytes，已取整", bytes))
                    } else {
                        None
                    };
                    (gb.to_string(), note)
                }
                Err(_) => ("-1".to_string(), None),
            }
        }
    };

    Ok((retention_secs, retention_gb, retention_gb_note))
}

/// 修改 topic 的 retention.ms 和 retention.bytes
/// retention_ms: 毫秒（-1 无限制），retention_bytes: bytes（-1 无限制）
pub async fn alter_topic_config(
    config: ConnectionConfig,
    topic: String,
    retention_ms: i64,
    retention_bytes: i64,
) -> Result<()> {
    let admin = build_admin(&config)?;
    let ms_str = retention_ms.to_string();
    let bytes_str = retention_bytes.to_string();
    let alter = AlterConfig::new(ResourceSpecifier::Topic(&topic))
        .set("retention.ms", &ms_str)
        .set("retention.bytes", &bytes_str);

    let results = admin
        .alter_configs(&[alter], &AdminOptions::new().request_timeout(Duration::from_secs(15)))
        .await
        .context("alter_configs 失败")?;

    for result in results {
        result.map_err(|e| anyhow::anyhow!("修改配置失败: {e}"))?;
    }
    Ok(())
}

/// 清空 topic 所有分区数据（delete_records 到 high watermark）
pub async fn purge_topic(
    config: ConnectionConfig,
    topic: String,
    partitions: Vec<i32>,
) -> Result<()> {
    // 先用 BaseConsumer 拉取各分区的 high watermark
    let consumer: BaseConsumer = build_consumer_config(&config)?.create()?;
    let mut tpl = TopicPartitionList::new();

    for partition in &partitions {
        let (_, high) = consumer
            .fetch_watermarks(&topic, *partition, Duration::from_secs(5))
            .with_context(|| format!("获取 partition {} watermark 失败", partition))?;
        tpl.add_partition_offset(&topic, *partition, Offset::Offset(high))
            .with_context(|| format!("设置 partition {} offset 失败", partition))?;
    }

    let admin = build_admin(&config)?;
    let results = admin
        .delete_records(&tpl, &AdminOptions::new().request_timeout(Duration::from_secs(30)))
        .await
        .context("delete_records 失败")?;

    for result in results.elements() {
        if let Err(e) = result.error() {
            return Err(anyhow::anyhow!(
                "清空 partition {} 失败: {:?}",
                result.partition(),
                e
            ));
        }
    }
    Ok(())
}

fn build_admin(config: &ConnectionConfig) -> Result<AdminClient<DefaultClientContext>> {
    use crate::config::SecurityProtocol;
    let mut client_config = ClientConfig::new();
    client_config
        .set("bootstrap.servers", &config.brokers)
        .set("security.protocol", config.security_protocol.as_str());

    if let Some(sasl) = &config.sasl {
        client_config
            .set("sasl.mechanism", sasl.mechanism.as_str())
            .set("sasl.username", &sasl.username)
            .set("sasl.password", &sasl.password);
    }

    if matches!(
        config.security_protocol,
        SecurityProtocol::Ssl | SecurityProtocol::SaslSsl
    ) {
        if let Some(ssl) = &config.ssl {
            if let Some(ca) = &ssl.ca_location {
                client_config.set("ssl.ca.location", ca);
            }
        }
    }
    Ok(client_config.create()?)
}

fn build_consumer_config(config: &ConnectionConfig) -> Result<ClientConfig> {
    use crate::config::SecurityProtocol;
    let mut client_config = ClientConfig::new();
    client_config
        .set("bootstrap.servers", &config.brokers)
        .set("security.protocol", config.security_protocol.as_str())
        .set("enable.auto.commit", "false")
        .set("enable.auto.offset.store", "false");

    if let Some(sasl) = &config.sasl {
        client_config
            .set("sasl.mechanism", sasl.mechanism.as_str())
            .set("sasl.username", &sasl.username)
            .set("sasl.password", &sasl.password);
    }
    Ok(client_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_to_gb_round_trip() {
        // 精确整数 GB
        let bytes: i64 = 5 * 1_073_741_824;
        let gb = (bytes as f64 / 1_073_741_824.0).round() as i64;
        assert_eq!(gb, 5);
    }

    #[test]
    fn bytes_non_integer_gb_rounds_up() {
        // 2,000,000,000 bytes ≈ 1.86 GB → rounds to 2
        let bytes: i64 = 2_000_000_000;
        let gb = (bytes as f64 / 1_073_741_824.0).round() as i64;
        assert_eq!(gb, 2);
        assert_ne!(bytes % 1_073_741_824, 0);
    }
}
```

- [ ] **Step 2: 运行单元测试**

```bash
cd /home/qingteng/rust-workspace/kafkax && cargo test admin
```
期望：2 个测试 PASS，编译成功

- [ ] **Step 3: Commit**

```bash
git add src/kafka/admin.rs
git commit -m "feat: 添加 Kafka 管理客户端（describe/alter 配置、清空数据）"
```

---

## Task 5: 右键菜单 UI

**Files:**
- Create: `src/ui/context_menu.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: 新建 context_menu.rs**

```rust
// src/ui/context_menu.rs
use iced::widget::{button, column, container, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::message::Message;
use crate::state::overlay_state::ContextMenuTarget;
use crate::theme::AppColors;

const MENU_WIDTH: f32 = 150.0;
const ITEM_HEIGHT: f32 = 36.0;
const MENU_PADDING: f32 = 4.0;

/// 渲染右键浮层菜单
/// x, y 为鼠标点击坐标，window_size 用于边界规避
pub fn view<'a>(
    x: f32,
    y: f32,
    target: &'a ContextMenuTarget,
    window_size: (f32, f32),
) -> Element<'a, Message> {
    let items: Vec<(&str, Message)> = match target {
        ContextMenuTarget::Topic { name, partitions } => {
            let n = name.clone();
            let p = partitions.clone();
            let n2 = n.clone();
            let p2 = p.clone();
            vec![
                (
                    "Topic 配置",
                    Message::OpenTopicConfig {
                        topic: n,
                        partitions: p,
                    },
                ),
                (
                    "清空数据",
                    Message::OpenTopicConfig {
                        topic: n2,
                        partitions: p2,
                    },
                ),
                ("关闭", Message::CloseOverlay),
            ]
        }
        ContextMenuTarget::Partition { topic, partition } => vec![
            (
                "发送消息",
                Message::OpenSendMessage {
                    topic: topic.clone(),
                    partition: *partition,
                },
            ),
            ("关闭", Message::CloseOverlay),
        ],
    };

    let item_count = items.len() as f32;
    let menu_height = item_count * ITEM_HEIGHT + MENU_PADDING * 2.0;

    // 边界规避
    let (win_w, win_h) = window_size;
    let final_x = if x + MENU_WIDTH > win_w { x - MENU_WIDTH } else { x };
    let final_y = if y + menu_height > win_h { y - menu_height } else { y };

    let menu_items = items.into_iter().fold(
        column![].spacing(0),
        |col, (label, msg)| {
            let btn = button(
                text(label)
                    .size(13)
                    .color(AppColors::TEXT_PRIMARY),
            )
            .on_press(msg)
            .style(|_theme: &Theme, status| button::Style {
                background: Some(Background::Color(
                    if matches!(status, button::Status::Hovered) {
                        AppColors::ROW_HOVER
                    } else {
                        AppColors::BG_SECONDARY
                    },
                )),
                border: Border::default(),
                text_color: AppColors::TEXT_PRIMARY,
                ..Default::default()
            })
            .padding([8, 14])
            .width(Length::Fill);
            col.push(btn)
        },
    );

    let menu = container(menu_items)
        .width(MENU_WIDTH)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(AppColors::BG_SECONDARY)),
            border: Border {
                color: AppColors::BORDER,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .padding(MENU_PADDING as u16);

    // 使用绝对定位容器
    container(menu)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: final_y,
            left: final_x,
            right: 0.0,
            bottom: 0.0,
        })
        .align_x(iced::alignment::Horizontal::Left)
        .align_y(iced::alignment::Vertical::Top)
        .into()
}
```

> **注意**："清空数据"菜单项直接打开 TopicConfig 弹窗（在弹窗内操作）。Iced 0.14 没有原生绝对定位，使用 padding 偏移近似实现坐标定位。

- [ ] **Step 2: 修改 src/ui/mod.rs 导出**

追加：
```rust
pub mod context_menu;
pub mod send_message_dialog;
pub mod topic_config_dialog;
```

- [ ] **Step 3: 编译验证**

```bash
cd /home/qingteng/rust-workspace/kafkax && cargo build 2>&1 | head -30
```

- [ ] **Step 4: Commit**

```bash
git add src/ui/context_menu.rs src/ui/mod.rs
git commit -m "feat: 添加右键浮层菜单 UI"
```

---

## Task 6: 发送消息弹窗 UI

**Files:**
- Create: `src/ui/send_message_dialog.rs`

- [ ] **Step 1: 新建 send_message_dialog.rs**

```rust
// src/ui/send_message_dialog.rs
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::kafka::producer::parse_json_to_payloads;
use crate::message::Message;
use crate::theme::AppColors;

/// 渲染发送消息弹窗（居中显示）
pub fn view<'a>(
    topic: &'a str,
    partition: i32,
    input: &'a str,
    sending: bool,
    error: Option<&'a str>,
) -> Element<'a, Message> {
    // 实时解析 JSON，计算条数
    let parse_result = if input.trim().is_empty() {
        None
    } else {
        Some(parse_json_to_payloads(input))
    };

    let send_label = match &parse_result {
        Some(Ok(payloads)) if !payloads.is_empty() => {
            if sending {
                "发送中...".to_string()
            } else {
                format!("发送 ({} 条)", payloads.len())
            }
        }
        _ => {
            if sending { "发送中...".to_string() } else { "发送".to_string() }
        }
    };

    let can_send = !sending
        && matches!(&parse_result, Some(Ok(p)) if !p.is_empty());

    // JSON 解析错误提示（优先显示发送错误，再显示解析错误）
    let error_msg: Option<String> = error.map(|e| e.to_string()).or_else(|| {
        if let Some(Err(e)) = &parse_result {
            Some(format!("JSON 格式错误: {}", e))
        } else {
            None
        }
    });

    let title = text(format!("发送消息到 {} / P-{}", topic, partition))
        .size(15)
        .color(AppColors::TEXT_PRIMARY);

    let input_area = text_input("输入 JSON 对象 {} 或数组 [{},{}]", input)
        .on_input(Message::SendMessageInputChanged)
        .padding([10, 12])
        .size(13);

    let error_row = if let Some(msg) = &error_msg {
        column![
            text(msg)
                .size(12)
                .color(Color::from_rgb(0.9, 0.2, 0.2))
        ]
    } else {
        column![]
    };

    let cancel_btn = button(text("取消").size(13).color(AppColors::TEXT_SECONDARY))
        .on_press(Message::CloseOverlay)
        .style(|_theme: &Theme, status| button::Style {
            background: Some(Background::Color(
                if matches!(status, button::Status::Hovered) {
                    AppColors::ROW_HOVER
                } else {
                    AppColors::BG_TERTIARY
                },
            )),
            border: Border {
                color: AppColors::BORDER,
                width: 1.0,
                radius: 8.0.into(),
            },
            text_color: AppColors::TEXT_SECONDARY,
            ..Default::default()
        })
        .padding([8, 16]);

    let send_btn_base = button(
        text(&send_label).size(13).color(Color::WHITE),
    )
    .style(|_theme: &Theme, status| button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered) {
                AppColors::ACCENT_HOVER
            } else {
                AppColors::ACCENT
            },
        )),
        border: Border {
            color: AppColors::ACCENT,
            width: 1.0,
            radius: 8.0.into(),
        },
        text_color: Color::WHITE,
        ..Default::default()
    })
    .padding([8, 16]);

    let send_btn: iced::widget::Button<'_, Message> = if can_send {
        send_btn_base.on_press(Message::SendMessages)
    } else {
        send_btn_base
    };

    let btn_row = row![cancel_btn, send_btn].spacing(8);

    let dialog = container(
        column![
            title,
            input_area,
            error_row,
            btn_row,
        ]
        .spacing(12),
    )
    .width(480)
    .padding(24)
    .style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(AppColors::BG_SECONDARY)),
        border: Border {
            color: AppColors::BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    });

    // 居中显示
    container(dialog)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
```

- [ ] **Step 2: 编译验证**

```bash
cd /home/qingteng/rust-workspace/kafkax && cargo build 2>&1 | head -30
```

- [ ] **Step 3: Commit**

```bash
git add src/ui/send_message_dialog.rs
git commit -m "feat: 添加发送消息弹窗 UI（支持实时 JSON 解析和条数显示）"
```

---

## Task 7: Topic 配置弹窗 UI

**Files:**
- Create: `src/ui/topic_config_dialog.rs`

- [ ] **Step 1: 新建 topic_config_dialog.rs**

```rust
// src/ui/topic_config_dialog.rs
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::message::Message;
use crate::state::overlay_state::is_valid_retention_input;
use crate::theme::AppColors;

/// 渲染 Topic 配置弹窗（居中显示）
pub fn view<'a>(
    topic: &'a str,
    retention_secs: &'a str,
    retention_gb: &'a str,
    retention_gb_note: Option<&'a str>,
    loading: bool,
    saving: bool,
    purging: bool,
    purge_confirm_pending: bool,
    error: Option<&'a str>,
) -> Element<'a, Message> {
    let busy = loading || saving || purging;
    let input_valid =
        is_valid_retention_input(retention_secs) && is_valid_retention_input(retention_gb);

    let title = text(format!("Topic 配置 - {}", topic))
        .size(15)
        .color(AppColors::TEXT_PRIMARY);

    // 保留时间输入
    let secs_input = text_input(
        if loading { "加载中..." } else { "秒，-1 表示无限制" },
        retention_secs,
    )
    .on_input(Message::TopicConfigRetentionSecsChanged)
    .padding([8, 10])
    .size(13);

    // 磁盘占用输入
    let gb_input = text_input(
        if loading { "加载中..." } else { "GB，-1 表示无限制" },
        retention_gb,
    )
    .on_input(Message::TopicConfigRetentionGbChanged)
    .padding([8, 10])
    .size(13);

    // GB 取整说明
    let note_row = if let Some(note) = retention_gb_note {
        column![
            text(format!("ℹ {}", note))
                .size(11)
                .color(AppColors::TEXT_MUTED)
        ]
    } else {
        column![]
    };

    // 输入验证错误
    let validation_error = if !loading && !input_valid {
        column![
            text("请输入正整数或 -1")
                .size(12)
                .color(Color::from_rgb(0.9, 0.2, 0.2))
        ]
    } else {
        column![]
    };

    // 操作错误
    let error_row = if let Some(msg) = error {
        column![
            text(msg)
                .size(12)
                .color(Color::from_rgb(0.9, 0.2, 0.2))
        ]
    } else {
        column![]
    };

    // 保存按钮
    let can_save = !busy && input_valid;
    let save_label = if saving { "保存中..." } else { "保存配置" };
    let save_btn_base = button(text(save_label).size(13).color(Color::WHITE))
        .style(|_theme: &Theme, status| button::Style {
            background: Some(Background::Color(
                if matches!(status, button::Status::Hovered) {
                    AppColors::ACCENT_HOVER
                } else {
                    AppColors::ACCENT
                },
            )),
            border: Border {
                color: AppColors::ACCENT,
                width: 1.0,
                radius: 8.0.into(),
            },
            text_color: Color::WHITE,
            ..Default::default()
        })
        .padding([8, 16]);
    let save_btn: iced::widget::Button<'_, Message> = if can_save {
        save_btn_base.on_press(Message::SaveTopicConfig)
    } else {
        save_btn_base
    };

    let cancel_btn = button(text("取消").size(13))
        .on_press(Message::CloseOverlay)
        .style(|_theme: &Theme, status| button::Style {
            background: Some(Background::Color(
                if matches!(status, button::Status::Hovered) {
                    AppColors::ROW_HOVER
                } else {
                    AppColors::BG_TERTIARY
                },
            )),
            border: Border {
                color: AppColors::BORDER,
                width: 1.0,
                radius: 8.0.into(),
            },
            text_color: AppColors::TEXT_SECONDARY,
            ..Default::default()
        })
        .padding([8, 16]);

    let action_row = row![cancel_btn, save_btn].spacing(8);

    // 清空数据按钮
    let purge_label = if purging {
        "清空中..."
    } else if purge_confirm_pending {
        "确认清空？点击确认执行"
    } else {
        "清空当前堆积数据"
    };

    let purge_msg = if purge_confirm_pending {
        Message::ConfirmPurgeTopicData
    } else {
        Message::RequestPurgeTopicData
    };

    let purge_btn_base = button(
        text(purge_label)
            .size(13)
            .color(Color::from_rgb(0.9, 0.2, 0.2)),
    )
    .style(|_theme: &Theme, status| button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered) {
                Color::from_rgba(0.9, 0.2, 0.2, 0.1)
            } else {
                AppColors::BG_TERTIARY
            },
        )),
        border: Border {
            color: Color::from_rgb(0.9, 0.2, 0.2),
            width: 1.0,
            radius: 8.0.into(),
        },
        text_color: Color::from_rgb(0.9, 0.2, 0.2),
        ..Default::default()
    })
    .padding([8, 16]);

    let purge_btn: iced::widget::Button<'_, Message> = if !busy {
        purge_btn_base.on_press(purge_msg)
    } else {
        purge_btn_base
    };

    let danger_section = column![
        text("危险操作").size(12).color(AppColors::TEXT_MUTED),
        purge_btn,
    ]
    .spacing(8);

    let dialog = container(
        column![
            title,
            row![
                text("保留时间（秒）").size(13).color(AppColors::TEXT_SECONDARY),
                secs_input,
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            row![
                text("最大磁盘占用（GB）").size(13).color(AppColors::TEXT_SECONDARY),
                gb_input,
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            note_row,
            text("（-1 表示无限制）").size(11).color(AppColors::TEXT_MUTED),
            validation_error,
            action_row,
            iced::widget::horizontal_rule(1),
            danger_section,
            error_row,
        ]
        .spacing(12),
    )
    .width(480)
    .padding(24)
    .style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(AppColors::BG_SECONDARY)),
        border: Border {
            color: AppColors::BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    });

    container(dialog)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
```

- [ ] **Step 2: 编译验证**

```bash
cd /home/qingteng/rust-workspace/kafkax && cargo build 2>&1 | head -30
```

- [ ] **Step 3: Commit**

```bash
git add src/ui/topic_config_dialog.rs
git commit -m "feat: 添加 Topic 配置弹窗 UI（保留时间/磁盘占用/清空数据）"
```

---

## Task 8: Sidebar 右键捕获

**Files:**
- Modify: `src/ui/sidebar.rs`

- [ ] **Step 1: 修改 sidebar.rs，为 topic 和 partition 按钮包裹 mouse_area**

在 `src/ui/sidebar.rs` 顶部导入中添加：
```rust
use iced::widget::mouse_area;
use crate::state::overlay_state::ContextMenuTarget;
```

> **注意：** Iced 0.14 的 `mouse_area` 没有 `on_right_press_with`。需组合使用：
> - `on_move(|point| Message::CursorMoved(point.x, point.y))` — 持续追踪光标坐标到 `AppState::cursor_pos`
> - `on_right_press(Message::ShowContextMenu { target: ... })` — 触发菜单，坐标由 `update()` 从 `cursor_pos` 读取

修改 topic 按钮包裹（在 `content = content.push(topic_btn);` 前）：

```rust
let partition_ids: Vec<i32> = topic.partitions.iter().map(|p| p.id).collect();
let topic_name_rc = topic.name.clone();
let topic_name_rc2 = topic.name.clone();
let partition_ids2 = partition_ids.clone();

let topic_with_mouse = mouse_area(topic_btn)
    .on_move(move |point| Message::CursorMoved(point.x, point.y))
    .on_right_press(Message::ShowContextMenu {
        target: ContextMenuTarget::Topic {
            name: topic_name_rc2,
            partitions: partition_ids2,
        },
    });

content = content.push(topic_with_mouse);
```

修改 partition 按钮包裹（在 `content = content.push(part_btn);` 前）：

```rust
let part_topic = topic.name.clone();
let part_topic2 = topic.name.clone();
let part_id = part.id;

let part_with_mouse = mouse_area(part_btn)
    .on_move(move |point| Message::CursorMoved(point.x, point.y))
    .on_right_press(Message::ShowContextMenu {
        target: ContextMenuTarget::Partition {
            topic: part_topic2,
            partition: part_id,
        },
    });

content = content.push(part_with_mouse);
```

- [ ] **Step 2: 编译验证**

```bash
cd /home/qingteng/rust-workspace/kafkax && cargo build 2>&1 | head -30
```

- [ ] **Step 3: Commit**

```bash
git add src/ui/sidebar.rs
git commit -m "feat: sidebar 添加右键 mouse_area，捕获 Topic/Partition 右键坐标"
```

---

## Task 9: App update() 消息处理

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: 在 app.rs update() 末尾添加所有新 Message 的处理分支**

在 `Message::Noop => Task::none()` 之前插入：

```rust
// --- 右键菜单 ---
Message::CursorMoved(x, y) => {
    self.state.cursor_pos = (x, y);
    Task::none()
}
Message::ShowContextMenu { target } => {
    let (x, y) = self.state.cursor_pos;
    self.state.overlay = OverlayState::ContextMenu { x, y, target };
    Task::none()
}
Message::CloseOverlay => {
    self.state.overlay = OverlayState::None;
    Task::none()
}
Message::WindowResized(w, h) => {
    self.state.window_size = (w, h);
    Task::none()
}

// --- 发送消息 ---
Message::OpenSendMessage { topic, partition } => {
    self.state.overlay = OverlayState::SendMessage {
        topic,
        partition,
        input: String::new(),
        sending: false,
        error: None,
    };
    Task::none()
}
Message::SendMessageInputChanged(input) => {
    if let OverlayState::SendMessage { input: ref mut i, error: ref mut e, .. } =
        self.state.overlay
    {
        *i = input;
        *e = None; // 清除旧错误
    }
    Task::none()
}
Message::SendMessages => {
    // 连接状态守卫
    if self.consumer.is_none() {
        self.state.notice = Some(AppNotice::info("请先连接 Kafka 再发送消息"));
        return Task::none();
    }
    let (topic, partition, input) = match &self.state.overlay {
        OverlayState::SendMessage { topic, partition, input, .. } => {
            (topic.clone(), *partition, input.clone())
        }
        _ => return Task::none(),
    };

    // 解析 JSON
    let payloads = match kafkax::kafka::producer::parse_json_to_payloads(&input) {
        Ok(p) => p,
        Err(e) => {
            if let OverlayState::SendMessage { error, sending, .. } = &mut self.state.overlay {
                *error = Some(e.to_string());
                *sending = false;
            }
            return Task::none();
        }
    };

    if let OverlayState::SendMessage { sending, error, .. } = &mut self.state.overlay {
        *sending = true;
        *error = None;
    }

    let config = self.state.connection_config.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                kafkax::kafka::producer::send_messages(config, topic, partition, payloads)
            })
            .await
            .unwrap_or_else(|e| Err(anyhow::anyhow!("任务异常: {e}")))
            .map_err(|e| e.to_string())
        },
        Message::MessagesSent,
    )
}
Message::MessagesSent(result) => {
    match result {
        Ok(count) => {
            let msg = if let OverlayState::SendMessage { topic, partition, .. } =
                &self.state.overlay
            {
                format!("成功发送 {} 条消息到 {}/P-{}", count, topic, partition)
            } else {
                format!("成功发送 {} 条消息", count)
            };
            self.state.overlay = OverlayState::None;
            self.state.notice = Some(AppNotice::success(msg));
        }
        Err(e) => {
            if let OverlayState::SendMessage { sending, error, .. } = &mut self.state.overlay {
                *sending = false;
                *error = Some(e);
            }
        }
    }
    Task::none()
}

// --- Topic 配置 ---
Message::OpenTopicConfig { topic, partitions } => {
    // 连接状态守卫
    if self.consumer.is_none() {
        self.state.notice = Some(AppNotice::info("请先连接 Kafka 再修改配置"));
        return Task::none();
    }
    self.state.overlay = OverlayState::TopicConfig {
        topic: topic.clone(),
        partitions: partitions.clone(),
        retention_secs: String::new(),
        retention_gb: String::new(),
        retention_gb_note: None,
        loading: true,
        saving: false,
        error: None,
        purge_confirm_pending: false,
        purging: false,
    };
    let config = self.state.connection_config.clone();
    Task::perform(
        async move { kafkax::kafka::admin::describe_topic_config(config, topic).await.map_err(|e| e.to_string()) },
        Message::TopicConfigLoaded,
    )
}
Message::TopicConfigLoaded(result) => {
    if let OverlayState::TopicConfig {
        retention_secs,
        retention_gb,
        retention_gb_note,
        loading,
        error,
        ..
    } = &mut self.state.overlay
    {
        *loading = false;
        match result {
            Ok((secs, gb, note)) => {
                *retention_secs = secs;
                *retention_gb = gb;
                *retention_gb_note = note;
                *error = None;
            }
            Err(e) => {
                *error = Some(format!("加载配置失败: {}", e));
            }
        }
    }
    Task::none()
}
Message::TopicConfigRetentionSecsChanged(val) => {
    if let OverlayState::TopicConfig { retention_secs, .. } = &mut self.state.overlay {
        *retention_secs = val;
    }
    Task::none()
}
Message::TopicConfigRetentionGbChanged(val) => {
    if let OverlayState::TopicConfig { retention_gb, .. } = &mut self.state.overlay {
        *retention_gb = val;
    }
    Task::none()
}
Message::SaveTopicConfig => {
    let (topic, secs_str, gb_str) = match &self.state.overlay {
        OverlayState::TopicConfig { topic, retention_secs, retention_gb, .. } => {
            (topic.clone(), retention_secs.clone(), retention_gb.clone())
        }
        _ => return Task::none(),
    };

    let retention_ms = match kafkax::state::overlay_state::secs_to_ms(&secs_str) {
        Some(v) => v,
        None => {
            if let OverlayState::TopicConfig { error, .. } = &mut self.state.overlay {
                *error = Some("保留时间格式无效".to_string());
            }
            return Task::none();
        }
    };
    let retention_bytes = match kafkax::state::overlay_state::gb_to_bytes(&gb_str) {
        Some(v) => v,
        None => {
            if let OverlayState::TopicConfig { error, .. } = &mut self.state.overlay {
                *error = Some("磁盘占用格式无效或超出范围".to_string());
            }
            return Task::none();
        }
    };

    if let OverlayState::TopicConfig { saving, purge_confirm_pending, .. } = &mut self.state.overlay {
        *saving = true;
        *purge_confirm_pending = false;
    }

    let config = self.state.connection_config.clone();
    Task::perform(
        async move {
            kafkax::kafka::admin::alter_topic_config(config, topic, retention_ms, retention_bytes)
                .await
                .map_err(|e| e.to_string())
        },
        Message::TopicConfigSaved,
    )
}
Message::TopicConfigSaved(result) => {
    match result {
        Ok(()) => {
            let topic = match &self.state.overlay {
                OverlayState::TopicConfig { topic, .. } => topic.clone(),
                _ => String::new(),
            };
            self.state.overlay = OverlayState::None;
            self.state.notice = Some(AppNotice::success(format!("已更新 {} 配置", topic)));
        }
        Err(e) => {
            if let OverlayState::TopicConfig { saving, error, .. } = &mut self.state.overlay {
                *saving = false;
                *error = Some(e);
            }
        }
    }
    Task::none()
}
Message::RequestPurgeTopicData => {
    if let OverlayState::TopicConfig { purge_confirm_pending, .. } = &mut self.state.overlay {
        *purge_confirm_pending = true;
    }
    Task::none()
}
Message::ConfirmPurgeTopicData => {
    let (topic, partitions) = match &self.state.overlay {
        OverlayState::TopicConfig { topic, partitions, .. } => {
            (topic.clone(), partitions.clone())
        }
        _ => return Task::none(),
    };

    if let OverlayState::TopicConfig { purge_confirm_pending, purging, .. } = &mut self.state.overlay {
        *purge_confirm_pending = false;
        *purging = true;
    }

    let config = self.state.connection_config.clone();
    Task::perform(
        async move {
            kafkax::kafka::admin::purge_topic(config, topic, partitions)
                .await
                .map_err(|e| e.to_string())
        },
        Message::TopicDataPurged,
    )
}
Message::TopicDataPurged(result) => {
    match result {
        Ok(()) => {
            let topic = match &self.state.overlay {
                OverlayState::TopicConfig { topic, .. } => topic.clone(),
                _ => String::new(),
            };
            self.state.overlay = OverlayState::None;
            self.state.notice = Some(AppNotice::success(format!("已清空 {} 的全部数据", topic)));
        }
        Err(e) => {
            if let OverlayState::TopicConfig { purging, error, .. } = &mut self.state.overlay {
                *purging = false;
                *error = Some(e);
            }
        }
    }
    Task::none()
}
```

- [ ] **Step 2: 在 app.rs 顶部补充 use 语句**

确认已引入：
```rust
use kafkax::state::overlay_state::OverlayState;
```

- [ ] **Step 3: 编译验证**

```bash
cd /home/qingteng/rust-workspace/kafkax && cargo build 2>&1 | head -50
```

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: app.rs 处理右键菜单/发送消息/Topic 配置所有 Message 分支"
```

---

## Task 10: App view() 渲染 overlay + subscription 窗口尺寸

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: 修改 view() 添加 overlay 渲染**

在 `src/app.rs` 的 `view()` 方法中，将最后的 `stack!` 条件修改为处理所有 overlay 类型：

```rust
pub fn view(&self) -> Element<'_, Message> {
    // ... 原有代码不变，构建 page ...

    // overlay 渲染（包括已有的连接对话框 + 新的右键菜单/弹窗）
    if self.state.show_connection_dialog {
        let overlay = ui::connection_dialog::view(/* ... 原有参数 ... */);
        return iced::widget::stack![page, overlay].into();
    }

    match &self.state.overlay {
        OverlayState::None => page,

        OverlayState::ContextMenu { x, y, target } => {
            let backdrop = overlay_backdrop();
            let menu = ui::context_menu::view(*x, *y, target, self.state.window_size);
            iced::widget::stack![page, backdrop, menu].into()
        }

        OverlayState::SendMessage { topic, partition, input, sending, error } => {
            let backdrop = overlay_backdrop();
            let dialog = ui::send_message_dialog::view(
                topic,
                *partition,
                input,
                *sending,
                error.as_deref(),
            );
            iced::widget::stack![page, backdrop, dialog].into()
        }

        OverlayState::TopicConfig {
            topic,
            retention_secs,
            retention_gb,
            retention_gb_note,
            loading,
            saving,
            purging,
            purge_confirm_pending,
            error,
            ..
        } => {
            let backdrop = overlay_backdrop();
            let dialog = ui::topic_config_dialog::view(
                topic,
                retention_secs,
                retention_gb,
                retention_gb_note.as_deref(),
                *loading,
                *saving,
                *purging,
                *purge_confirm_pending,
                error.as_deref(),
            );
            iced::widget::stack![page, backdrop, dialog].into()
        }
    }
}
```

在 `view()` 内部添加辅助函数（模块级，不在 impl 内）：

```rust
/// 全屏半透明遮罩，点击触发 CloseOverlay
fn overlay_backdrop() -> iced::widget::Button<'static, Message> {
    use iced::widget::button;
    use iced::{Background, Border, Color, Length};
    button(iced::widget::Space::new(Length::Fill, Length::Fill))
        .on_press(Message::CloseOverlay)
        .style(|_theme, _status| button::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.4))),
            border: Border::default(),
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill)
}
```

- [ ] **Step 2: 修改 subscription() 添加窗口尺寸监听**

```rust
pub fn subscription(&self) -> Subscription<Message> {
    Subscription::batch([
        keyboard::listen().map(|event| match event {
            keyboard::Event::KeyPressed { key, modifiers, .. } => {
                Message::KeyPressed(key, modifiers)
            }
            _ => Message::Noop,
        }),
        window::open_events().map(Message::WindowOpened),
        window::events().map(|(_, event)| match event {
            window::Event::Resized(iced::Size { width, height }) => {
                Message::WindowResized(width as f32, height as f32)
            }
            _ => Message::Noop,
        }),
    ])
}
```

- [ ] **Step 3: 修改 handle_key()，Escape 也关闭 overlay**

在 `handle_key()` 的 Escape 分支中，同时关闭 overlay：
```rust
Key::Named(Named::Escape) => {
    // 先尝试关闭 overlay
    if !matches!(self.state.overlay, OverlayState::None) {
        self.state.overlay = OverlayState::None;
        return Task::none();
    }
    self.state.table.selected_index = None;
    self.clear_detail_content();
    Task::none()
}
```

- [ ] **Step 4: 编译并运行测试**

```bash
cd /home/qingteng/rust-workspace/kafkax && cargo build 2>&1 | head -30
cargo test 2>&1 | tail -20
```
期望：编译成功，所有测试 PASS

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: app view() 渲染右键菜单/弹窗 overlay，subscription 监听窗口尺寸"
```

---

## 完成核查

- [ ] `cargo test` 全部通过
- [ ] `cargo build --release` 成功
- [ ] 右键 Topic 弹出菜单，点击"Topic 配置"打开弹窗，配置值预填充
- [ ] 右键 Partition 弹出菜单，点击"发送消息"打开弹窗，单条/数组均可发送
- [ ] 发送成功后状态栏显示成功提示
- [ ] Topic 配置保存后状态栏显示成功提示
- [ ] 清空数据二次确认后执行，成功后关闭弹窗
- [ ] Escape 键关闭任意弹窗
- [ ] 点击遮罩层关闭弹窗
