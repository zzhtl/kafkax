# KafkaX

**高性能 Kafka 桌面客户端**，基于 Rust + iced 构建，专为海量数据查询场景设计。

---

## 核心优势

### 极致性能
- **多分区并行搜索**：基于 [rayon](https://github.com/rayon-rs/rayon) 并行扫描所有分区，N 个分区同时运行，速度提升 N 倍
- **流式增量显示**：搜索结果实时推送至 UI，无需等待全部扫描完成即可看到命中消息
- **内存零拷贝架构**：搜索结果使用 `Arc<Vec<MessageSummary>>` 包装，翻页切片只做原子计数 +1，彻底消除大对象 Clone
- **预计算显示标签**：`partition_label`、`offset_label`、`ts_label` 等在后台线程一次性计算，UI 渲染帧零字符串分配
- **按需详情拉取**：列表只存轻量摘要（< 1KB/条），点击时精确按 `partition + offset` 拉取单条完整消息，支持 10MB+ 大消息不卡顿

### 双重扫描限制，杜绝 OOM
- 消息数上限：**100 万条**
- 累计 payload 上限：**2 GB**
- 任一条件触发即自动停止，状态栏给出精确提示

### 多格式自动解码
支持以下格式，可手动选择或开启智能自动检测：

| 格式 | 说明 |
|------|------|
| **JSON** | 自动 pretty-print，支持嵌套搜索 |
| **Protobuf** | 启发式字段解析 |
| **Avro** | 识别 `Obj\x01` 容器头，自动解码 |
| **MessagePack** | 字节特征匹配后解码为 JSON 树 |
| **Text** | 多编码自动检测（`chardetng` + `encoding_rs`）|
| **Binary** | Hex dump 格式展示 |

### 完整安全支持
- 明文（PLAINTEXT）
- TLS 加密（SSL）
- SASL_PLAINTEXT / SASL_SSL
- SASL 机制：PLAIN、SCRAM-SHA-256、SCRAM-SHA-512
- 双向 TLS（Mutual TLS，客户端证书）

---

## 功能一览

- **连接管理**：多连接配置持久化存储，启动自动恢复上次连接
- **Topic 浏览**：侧边栏展示所有 Topic 及分区列表，watermark 实时显示
- **Topic 配置**：右键 Topic 可查看并修改 `retention.ms` / `retention.bytes`
- **堆积清理**：支持按 Topic 一键清空当前全部分区的堆积数据
- **消息翻页**：支持正序 / 倒序分页，基于 watermark 精确计算页起始 offset
- **全文搜索**：Key、Value、Offset 三合一大小写不敏感搜索
- **搜索范围切换**：Checkbox 一键切换「单分区」/「全分区」扫描模式
- **消息详情**：完整格式化内容，滚动浏览，一键复制 JSON / 原始文本
- **消息发送**：右键 Partition 直接向指定分区发送 JSON 消息，支持批量发送
- **解码器切换**：运行时切换解码格式，即时刷新显示

---

## 常用操作

### 发送消息到 Kafka

1. 在左侧展开 Topic，右键某个 `Partition`
2. 点击 `发送消息`
3. 在弹窗中输入 JSON，并按需点击 `格式化`
4. 点击 `发送`

发送规则：

- 输入 **JSON 对象** 时，会发送为 **1 条消息**
- 输入 **JSON 数组** 时，数组中的每个元素会分别发送为 **1 条消息**
- 当前版本按 **指定 Partition** 发送，不做自动分区
- 输入为空、JSON 非法或数组为空时，不会触发发送

示例：

```json
{"event":"user_login","user_id":1001}
```

```json
[
  {"event":"order_created","order_id":1},
  {"event":"order_paid","order_id":1}
]
```

### Topic 配置

1. 在左侧右键某个 `Topic`
2. 点击 `Topic 配置`
3. 在弹窗中修改以下配置并保存：

- `保留时间（秒）`：对应 Kafka 的 `retention.ms`
- `最大磁盘占用（GB）`：对应 Kafka 的 `retention.bytes`
- 输入 `-1` 表示无限制

说明：

- 程序会先读取 Topic 当前配置，再回填到弹窗中
- `retention.bytes` 会以 GB 为单位展示；如果原始值不是整数 GB，会显示取整提示

### 清空 Topic 堆积数据

1. 在左侧右键某个 `Topic`
2. 点击 `清空数据`，会打开 `Topic 配置` 弹窗
3. 在弹窗底部 `危险操作` 区域点击 `清空当前堆积数据`
4. 再点击一次确认执行

行为说明：

- 会对该 Topic 的 **全部分区** 执行清理
- 清理范围是当前 high watermark 之前的已有数据，也就是“当前堆积数据”
- 这是高风险操作，执行后已有消息将不可继续消费，请谨慎使用

---

## 技术栈

| 组件 | 说明 |
|------|------|
| **Rust 2024 Edition** | 内存安全、零成本抽象、编译期保证 |
| **iced 0.14** | Elm 架构 GUI 框架，基于 tokio 异步运行时 |
| **rdkafka 0.39** | librdkafka 的 Rust 绑定，SSL / zstd / libz 特性全开 |
| **rayon** | 数据并行库，多分区无锁并行搜索 |
| **tokio** | 异步运行时，处理网络 I/O 和后台任务 |
| **tracing** | 结构化日志，支持 `RUST_LOG` 环境变量过滤 |

发布版本编译配置（`Cargo.toml`）：
```toml
[profile.release]
opt-level = 3       # 最高优化级别
lto = "thin"        # 跨 crate 链接时优化
codegen-units = 1   # 单编译单元，最大化内联
strip = true        # 剥离调试符号，最小化二进制体积
```

---

## 快速开始

### 依赖

- Rust 1.85+（需支持 Rust 2024 Edition）
- librdkafka 系统库（或通过 rdkafka-sys 自动编译）
- Linux：需要 X11 开发库（`libx11-dev`）

### 构建

```bash
# 调试构建
cargo build

# 发布构建（推荐，完整性能优化）
cargo build --release
```

### 运行

```bash
cargo run --release
```

启动后弹出连接配置对话框，填入 Kafka Broker 地址（如 `localhost:9092`）即可连接。

### 环境变量

```bash
# 调整日志级别
RUST_LOG=kafkax=debug cargo run --release

# 关闭日志
RUST_LOG=error cargo run --release
```

---

## 性能指标

| 场景 | 表现 |
|------|------|
| 搜索 100 万条消息（8 分区）| 并行 8x 加速，且流式展示，首批结果秒级可见 |
| 搜索结果内存（100 万条）| ~500 MB（每条摘要 ~500B，无原始 payload） |
| 点击详情（10MB JSON）| 后台 blocking 线程格式化，UI 线程零阻塞 |
| 每帧渲染开销 | 预计算标签，零字符串分配 |
| 翻页 / 切换搜索结果页 | `Arc` 原子计数 +1，无数据复制 |

---

## 配置文件

配置自动保存在系统标准路径：
- **Linux**：`~/.config/kafkax/config.toml`
- **macOS**：`~/Library/Application Support/com.kafkax.KafkaX/config.toml`
- **Windows**：`%APPDATA%\kafkax\KafkaX\config\config.toml`

---

## 运行测试

```bash
cargo test
```

---

## License

本项目基于 [LICENSE](LICENSE) 授权。
