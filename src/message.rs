use crate::codec::DecoderType;
use crate::config::{ConnectionConfig, SaslMechanism, SecurityProtocol};
use crate::kafka::types::{MessageSummary, PageData, SortOrder, TopicMeta};
use crate::state::overlay_state::ContextMenuTarget;

/// 全局消息枚举
#[derive(Debug, Clone)]
pub enum Message {
    // --- 连接相关 ---
    /// 打开连接对话框
    ShowConnectionDialog,
    /// 关闭连接对话框
    CloseConnectionDialog,
    /// Broker 地址输入变化
    BrokerInputChanged(String),
    /// 连接名称输入变化
    ConnectionNameInputChanged(String),
    /// 消费组输入变化
    GroupIdInputChanged(String),
    /// 安全协议切换
    SecurityProtocolChanged(SecurityProtocol),
    /// SASL 机制切换
    SaslMechanismChanged(SaslMechanism),
    /// SASL 用户名输入变化
    SaslUsernameChanged(String),
    /// SASL 密码输入变化
    SaslPasswordChanged(String),
    /// SSL CA 路径输入变化
    SslCaLocationChanged(String),
    /// SSL 客户端证书路径输入变化
    SslCertLocationChanged(String),
    /// SSL 客户端私钥路径输入变化
    SslKeyLocationChanged(String),
    /// SSL 私钥口令输入变化
    SslKeyPasswordChanged(String),
    /// 载入已保存连接
    LoadSavedConnection(String),
    /// 直接连接已保存连接
    ConnectSavedConnection(String),
    /// 删除已保存连接
    DeleteSavedConnection(String),
    /// 发起连接
    Connect,
    /// 连接成功，返回 topic 列表
    Connected(ConnectionConfig, Vec<TopicMeta>),
    /// 连接失败
    ConnectionFailed(String),
    /// 断开连接
    Disconnect,

    // --- 侧边栏 ---
    /// Topic 搜索输入变化
    TopicSearchInputChanged(String),
    /// 展开/折叠 topic
    ToggleTopic(String),
    /// 选中 partition
    SelectPartition(String, i32),

    // --- 数据加载 ---
    /// 页数据加载完成
    PageLoaded(u64, Result<(PageData, u128), String>),
    /// 单个分区搜索完成（并行搜索中每个分区独立返回）
    SearchProgress {
        request_id: u64,
        partition: i32,
        hits: Vec<MessageSummary>,
        local_scanned: usize,
        local_bytes: usize,
        stopped_early: bool,
    },
    /// 消息详情后台生成完成
    DetailLoaded(u64, Result<String, String>),
    /// 加载中状态
    Loading,

    // --- 分页 ---
    /// 首页
    FirstPage,
    /// 上一页
    PrevPage,
    /// 下一页
    NextPage,
    /// 末页
    LastPage,
    /// 跳转到指定页
    GoToPage(usize),
    /// 修改每页大小
    PageSizeChanged(usize),
    /// 修改排序方向
    SortOrderChanged(SortOrder),

    // --- 消息 ---
    /// 选中消息
    SelectMessage(usize),
    /// 复制当前消息 JSON
    CopySelectedMessage,
    // --- 解码器 ---
    /// 切换解码器
    DecoderChanged(DecoderType),

    // --- 搜索 ---
    /// 搜索输入变化
    SearchInputChanged(String),
    /// 执行搜索
    Search,
    /// 设置"全分区搜索"开关（直接传目标值，避免多次触发竞态）
    SetSearchAllPartitions(bool),

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
    /// 文本编辑器动作
    SendMessageAction(iced::widget::text_editor::Action),
    /// 格式化 JSON
    FormatSendMessageJson,
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

    // --- 其他 ---
    /// 主窗口已打开
    WindowOpened(iced::window::Id),
    /// 同步平台窗口标题
    ApplyPlatformWindowTitle,
    /// 键盘事件
    KeyPressed(iced::keyboard::Key, iced::keyboard::Modifiers),
    /// 无操作
    Noop,
}
