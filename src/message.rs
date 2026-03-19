use crate::codec::DecoderType;
use crate::kafka::types::{PageData, TopicMeta};

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
    /// 发起连接
    Connect,
    /// 连接成功，返回 topic 列表
    Connected(Vec<TopicMeta>),
    /// 连接失败
    ConnectionFailed(String),
    /// 断开连接
    Disconnect,

    // --- 侧边栏 ---
    /// 展开/折叠 topic
    ToggleTopic(String),
    /// 选中 partition
    SelectPartition(String, i32),

    // --- 数据加载 ---
    /// 页数据加载完成
    PageLoaded(Result<(PageData, u128), String>),
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
