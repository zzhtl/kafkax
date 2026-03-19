use crate::codec::DecoderType;
use crate::config::{ConnectionConfig, SaslMechanism, SecurityProtocol};
use crate::kafka::types::{PageData, SearchResult, TopicMeta};
use iced::widget::text_editor;

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
    /// 全分区搜索完成
    SearchLoaded(u64, Result<(SearchResult, u128), String>),
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

    // --- 消息 ---
    /// 选中消息
    SelectMessage(usize),
    /// 复制当前消息 JSON
    CopySelectedMessage,
    /// 详情区编辑器动作
    DetailEditorAction(text_editor::Action),

    // --- 解码器 ---
    /// 切换解码器
    DecoderChanged(DecoderType),

    // --- 搜索 ---
    /// 搜索输入变化
    SearchInputChanged(String),
    /// 执行搜索
    Search,

    // --- 其他 ---
    /// 键盘事件
    KeyPressed(iced::keyboard::Key, iced::keyboard::Modifiers),
    /// 无操作
    Noop,
}
