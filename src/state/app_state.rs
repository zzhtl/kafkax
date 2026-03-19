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

/// 全局应用状态
#[derive(Debug, Clone)]
pub struct AppState {
    pub connection_status: ConnectionStatus,
    pub connection_config: ConnectionConfig,
    pub sidebar: SidebarState,
    pub table: TableState,
    pub decoder: DecoderPipeline,
    pub show_connection_dialog: bool,
    pub broker_input: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            connection_status: ConnectionStatus::Disconnected,
            connection_config: ConnectionConfig::default(),
            sidebar: SidebarState::default(),
            table: TableState::default(),
            decoder: DecoderPipeline::default(),
            show_connection_dialog: true,
            broker_input: "localhost:9092".to_string(),
        }
    }
}
