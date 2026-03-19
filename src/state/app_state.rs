use crate::codec::DecoderPipeline;
use crate::config::ConnectionConfig;
use crate::state::sidebar_state::SidebarState;
use crate::state::table_state::TableState;

/// 连接状态
#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected(String),
    Error(String),
}

/// 界面反馈级别
#[derive(Debug, Clone, Copy)]
pub enum NoticeTone {
    Info,
    Success,
    Error,
}

/// 界面反馈消息
#[derive(Debug, Clone)]
pub struct AppNotice {
    pub tone: NoticeTone,
    pub message: String,
}

impl AppNotice {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            tone: NoticeTone::Info,
            message: message.into(),
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self {
            tone: NoticeTone::Success,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            tone: NoticeTone::Error,
            message: message.into(),
        }
    }
}

/// 全局应用状态
#[derive(Debug, Clone)]
pub struct AppState {
    pub connection_status: ConnectionStatus,
    pub connection_config: ConnectionConfig,
    pub connection_draft: ConnectionConfig,
    pub saved_connections: Vec<ConnectionConfig>,
    pub last_connection_name: Option<String>,
    pub sidebar: SidebarState,
    pub table: TableState,
    pub decoder: DecoderPipeline,
    pub show_connection_dialog: bool,
    pub notice: Option<AppNotice>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            connection_status: ConnectionStatus::Disconnected,
            connection_config: ConnectionConfig::default(),
            connection_draft: ConnectionConfig::default(),
            saved_connections: Vec::new(),
            last_connection_name: None,
            sidebar: SidebarState::default(),
            table: TableState::default(),
            decoder: DecoderPipeline::default(),
            show_connection_dialog: true,
            notice: None,
        }
    }
}
