use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use iced::clipboard;
use iced::keyboard;
use iced::widget::{column, container, row};
use iced::{Background, Border, Element, Length, Subscription, Task, Theme};
use rdkafka::consumer::StreamConsumer;

use kafkax::codec::DecoderPipeline;
use kafkax::config::{AppConfig, ConnectionConfig, SaslConfig, SslConfig};
use kafkax::kafka;
use kafkax::message::Message;
use kafkax::state::{AppNotice, AppState, ConnectionStatus};
use kafkax::theme;
use kafkax::ui;

/// 顶层应用
pub struct App {
    pub state: AppState,
    /// Kafka consumer（Arc 用于在异步任务中共享）
    pub consumer: Option<Arc<StreamConsumer>>,
    page_request_id: u64,
    search_request_id: u64,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let mut state = AppState::default();

        match AppConfig::load() {
            Ok(config) => {
                state.saved_connections = config.connections;
                state.last_connection_name = config.last_connection;

                if let Some(last_connection) = state
                    .last_connection_name
                    .as_deref()
                    .and_then(|name| {
                        state
                            .saved_connections
                            .iter()
                            .find(|connection| connection.name == name)
                    })
                    .cloned()
                    .or_else(|| state.saved_connections.first().cloned())
                {
                    state.connection_draft = last_connection;
                }
            }
            Err(error) => {
                tracing::error!("加载本地连接配置失败: {error:#}");
                state.notice = Some(AppNotice::error(format!(
                    "读取本地连接配置失败，将使用默认配置: {error}"
                )));
            }
        }

        let app = Self {
            state,
            consumer: None,
            page_request_id: 0,
            search_request_id: 0,
        };
        (app, Task::none())
    }

    pub fn title(&self) -> String {
        match &self.state.connection_status {
            ConnectionStatus::Connected(name) => format!("KafkaX - [{}]", name),
            _ => "KafkaX".to_string(),
        }
    }

    pub fn theme(&self) -> Theme {
        theme::app_theme()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // --- 连接相关 ---
            Message::ShowConnectionDialog => {
                self.state.show_connection_dialog = true;
                Task::none()
            }
            Message::CloseConnectionDialog => {
                self.state.show_connection_dialog = false;
                Task::none()
            }
            Message::BrokerInputChanged(input) => {
                self.state.connection_draft.brokers = input;
                Task::none()
            }
            Message::ConnectionNameInputChanged(input) => {
                self.state.connection_draft.name = input;
                Task::none()
            }
            Message::GroupIdInputChanged(input) => {
                self.state.connection_draft.group_id = optional_text(input);
                Task::none()
            }
            Message::SecurityProtocolChanged(protocol) => {
                self.state.connection_draft.security_protocol = protocol;
                if protocol.uses_sasl() {
                    let _ = self.connection_draft_sasl_mut();
                }
                if protocol.uses_ssl() {
                    let _ = self.connection_draft_ssl_mut();
                }
                Task::none()
            }
            Message::SaslMechanismChanged(mechanism) => {
                self.connection_draft_sasl_mut().mechanism = mechanism;
                Task::none()
            }
            Message::SaslUsernameChanged(input) => {
                self.connection_draft_sasl_mut().username = input;
                Task::none()
            }
            Message::SaslPasswordChanged(input) => {
                self.connection_draft_sasl_mut().password = input;
                Task::none()
            }
            Message::SslCaLocationChanged(input) => {
                self.connection_draft_ssl_mut().ca_location = optional_text(input);
                Task::none()
            }
            Message::SslCertLocationChanged(input) => {
                self.connection_draft_ssl_mut().cert_location = optional_text(input);
                Task::none()
            }
            Message::SslKeyLocationChanged(input) => {
                self.connection_draft_ssl_mut().key_location = optional_text(input);
                Task::none()
            }
            Message::SslKeyPasswordChanged(input) => {
                self.connection_draft_ssl_mut().key_password = optional_text(input);
                Task::none()
            }
            Message::LoadSavedConnection(name) => {
                if let Some(connection) = self.saved_connection_by_name(&name) {
                    self.state.connection_draft = connection.clone();
                    self.state.show_connection_dialog = true;
                    self.state.notice = Some(AppNotice::info(format!(
                        "已载入连接配置: {}",
                        connection.name
                    )));
                }
                Task::none()
            }
            Message::ConnectSavedConnection(name) => {
                if let Some(connection) = self.saved_connection_by_name(&name) {
                    self.state.connection_draft = connection.clone();
                    self.begin_connect(connection)
                } else {
                    self.state.notice = Some(AppNotice::error(format!("未找到已保存连接: {name}")));
                    Task::none()
                }
            }
            Message::DeleteSavedConnection(name) => {
                match self.delete_saved_connection(&name) {
                    Ok(true) => {
                        self.state.notice =
                            Some(AppNotice::success(format!("已删除保存的连接: {name}")));
                    }
                    Ok(false) => {
                        self.state.notice =
                            Some(AppNotice::info(format!("连接不存在或已删除: {name}")));
                    }
                    Err(error) => {
                        tracing::error!("删除已保存连接失败: {error:#}");
                        self.state.notice =
                            Some(AppNotice::error(format!("删除连接失败: {error}")));
                    }
                }
                Task::none()
            }
            Message::Connect => {
                let config = match Self::prepare_connection_config(&self.state.connection_draft) {
                    Ok(config) => config,
                    Err(error) => {
                        self.state.connection_status = ConnectionStatus::Error(error);
                        return Task::none();
                    }
                };

                self.begin_connect(config)
            }
            Message::Connected(config, topics) => {
                self.invalidate_page_requests();
                self.invalidate_search_requests();
                let name = config.name.clone();
                // 重新创建 consumer 用于后续消费
                match kafka::connection::create_consumer(&config) {
                    Ok(consumer) => {
                        self.consumer = Some(Arc::new(consumer));
                    }
                    Err(e) => {
                        self.state.connection_status =
                            ConnectionStatus::Error(format!("创建消费者失败: {}", e));
                        return Task::none();
                    }
                }

                self.state.connection_config = config.clone();
                self.state.connection_draft = config.clone();
                self.state.connection_status = ConnectionStatus::Connected(name);
                self.state.sidebar.topics = topics;
                self.state.table.clear_feedback();
                self.state.show_connection_dialog = false;
                self.state.notice = Some(AppNotice::success(format!(
                    "连接成功，共 {} 个 Topic",
                    self.state.sidebar.topics.len()
                )));

                if let Err(error) = self.remember_connection(&config) {
                    tracing::error!("保存连接配置失败: {error:#}");
                    self.state.notice = Some(AppNotice::error(format!(
                        "连接已建立，但保存配置失败: {error}"
                    )));
                }

                tracing::info!("连接成功，共 {} 个 topic", self.state.sidebar.topics.len());
                Task::none()
            }
            Message::ConnectionFailed(error) => {
                self.invalidate_page_requests();
                self.invalidate_search_requests();
                if self.consumer.is_some() {
                    let current_name = self.state.connection_config.name.clone();
                    self.state.connection_status = ConnectionStatus::Connected(current_name);
                    self.state.notice = Some(AppNotice::error(format!(
                        "切换连接失败，已保留当前连接: {error}"
                    )));
                } else {
                    self.state.connection_status = ConnectionStatus::Error(error.clone());
                }
                tracing::error!("连接失败: {}", error);
                Task::none()
            }
            Message::Disconnect => {
                self.invalidate_page_requests();
                self.invalidate_search_requests();
                self.consumer = None;
                self.state.connection_status = ConnectionStatus::Disconnected;
                self.state.connection_draft = self.state.connection_config.clone();
                self.state.sidebar = Default::default();
                self.state.table = Default::default();
                self.state.notice = Some(AppNotice::info(
                    "已断开连接，可直接重连或切换到其他已保存配置",
                ));
                Task::none()
            }

            // --- 侧边栏 ---
            Message::TopicSearchInputChanged(query) => {
                self.state.sidebar.set_search_query(query);
                Task::none()
            }
            Message::ToggleTopic(topic) => {
                self.state.sidebar.toggle_topic(&topic);
                Task::none()
            }
            Message::SelectPartition(topic, partition) => {
                self.state.sidebar.select_partition(&topic, partition);
                if self.state.table.has_search_results() || self.state.table.search_in_progress {
                    self.invalidate_search_requests();
                    self.state.table.clear_search_results();
                }
                self.state.table.current_page = 0;
                self.state.table.selected_index = None;
                self.sync_detail_content();

                if self.state.table.search_query.trim().is_empty() {
                    self.reload_current_page()
                } else {
                    self.begin_topic_search()
                }
            }

            // --- 数据加载 ---
            Message::PageLoaded(request_id, result) => {
                if request_id != self.page_request_id {
                    return Task::none();
                }

                match result {
                    Ok((page_data, load_time)) => {
                        self.state.table.apply_page_data(page_data, load_time);
                        self.sync_detail_content();
                        Task::none()
                    }
                    Err(error) => {
                        self.state.table.fail_loading(error.clone());
                        tracing::error!("加载消息失败: {}", error);
                        Task::none()
                    }
                }
            }
            Message::SearchLoaded(request_id, result) => {
                if request_id != self.search_request_id {
                    return Task::none();
                }

                match result {
                    Ok((search_result, load_time)) => {
                        let match_count = search_result.matches.len();
                        let scanned_messages = search_result.scanned_messages;
                        let low_watermark = search_result.low_watermark;
                        let high_watermark = search_result.high_watermark;

                        self.state.table.apply_search_results(
                            search_result.matches,
                            scanned_messages,
                            low_watermark,
                            high_watermark,
                            load_time,
                        );
                        self.sync_detail_content();
                        self.state.notice = Some(AppNotice::success(format!(
                            "搜索完成：已扫描 {} 条，命中 {} 条",
                            scanned_messages, match_count
                        )));
                        Task::none()
                    }
                    Err(error) => {
                        self.state.table.fail_loading(error.clone());
                        self.state.notice = Some(AppNotice::error(format!("搜索失败: {error}")));
                        tracing::error!("搜索消息失败: {error}");
                        Task::none()
                    }
                }
            }
            Message::Loading => {
                self.state.table.begin_loading();
                Task::none()
            }

            // --- 分页 ---
            Message::FirstPage => {
                self.state.table.current_page = 0;
                self.reload_current_page()
            }
            Message::PrevPage => {
                if self.state.table.has_prev() {
                    self.state.table.current_page -= 1;
                    self.reload_current_page()
                } else {
                    Task::none()
                }
            }
            Message::NextPage => {
                if self.state.table.has_next() {
                    self.state.table.current_page += 1;
                    self.reload_current_page()
                } else {
                    Task::none()
                }
            }
            Message::LastPage => {
                let total = self.state.table.total_pages();
                if total > 0 {
                    self.state.table.current_page = total - 1;
                    self.reload_current_page()
                } else {
                    Task::none()
                }
            }
            Message::GoToPage(page) => {
                if page < self.state.table.total_pages() {
                    self.state.table.current_page = page;
                    self.reload_current_page()
                } else {
                    Task::none()
                }
            }
            Message::PageSizeChanged(size) => {
                self.state.table.page_size = size;
                self.state.table.current_page = 0;
                self.state.table.selected_index = None;
                self.sync_detail_content();
                self.reload_current_page()
            }

            // --- 消息选中 ---
            Message::SelectMessage(idx) => {
                self.state.table.selected_index = Some(idx);
                self.sync_detail_content();
                Task::none()
            }
            Message::CopySelectedMessage => self.copy_selected_json(),
            Message::DetailEditorAction(action) => {
                if !action.is_edit() {
                    self.state.table.detail_content.perform(action);
                }
                Task::none()
            }

            // --- 解码器 ---
            Message::DecoderChanged(decoder_type) => {
                self.state.decoder = DecoderPipeline::new(decoder_type);
                if self.state.table.search_query.trim().is_empty() {
                    self.invalidate_search_requests();
                    self.reload_current_page()
                } else {
                    self.begin_topic_search()
                }
            }

            // --- 搜索 ---
            Message::SearchInputChanged(query) => {
                let had_search_context =
                    self.state.table.has_search_results() || self.state.table.search_in_progress;
                self.state.table.set_search_query(query);
                if had_search_context {
                    self.invalidate_search_requests();
                    self.state.table.clear_search_results();
                    self.reload_current_page()
                } else {
                    self.sync_detail_content();
                    Task::none()
                }
            }
            Message::Search => self.begin_topic_search(),

            // --- 键盘快捷键 ---
            Message::KeyPressed(key, modifiers) => self.handle_key(key, modifiers),

            Message::Noop => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        // 主布局
        let sidebar = ui::sidebar::view(
            &self.state.sidebar,
            &self.state.connection_status,
            self.state.saved_connections.len(),
        );

        let toolbar = ui::toolbar::view(&self.state.table, self.state.decoder.selected);
        let table = ui::message_table::view(&self.state.table);
        let detail = ui::message_detail::view(&self.state.table);

        let main_content = column![toolbar, table, detail]
            .spacing(12)
            .padding(12)
            .width(Length::Fill);

        let status_bar = ui::status_bar::view(
            &self.state.connection_status,
            self.state.notice.as_ref(),
            &self.state.table,
            self.state.sidebar.selected_topic.as_deref(),
            self.state.sidebar.selected_partition,
        );

        let body = column![row![sidebar, main_content].height(Length::Fill), status_bar,];

        let page: Element<'_, Message> = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(theme::AppColors::BG_PRIMARY)),
                border: Border::default(),
                ..Default::default()
            })
            .into();

        // 连接对话框覆盖层
        if self.state.show_connection_dialog {
            let overlay = ui::connection_dialog::view(
                &self.state.connection_draft,
                &self.state.connection_status,
                &self.state.saved_connections,
                self.state.last_connection_name.as_deref(),
                self.state.notice.as_ref(),
            );
            iced::widget::stack![page, overlay].into()
        } else {
            page
        }
    }

    /// 获取当前选中 partition 的当前页数据
    fn fetch_current_page(&self, request_id: u64) -> Task<Message> {
        let consumer = match &self.consumer {
            Some(c) => Arc::clone(c),
            None => return Task::none(),
        };

        let topic = match &self.state.sidebar.selected_topic {
            Some(t) => t.clone(),
            None => return Task::none(),
        };

        let partition = match self.state.sidebar.selected_partition {
            Some(p) => p,
            None => return Task::none(),
        };

        let offset = self.state.table.current_offset();
        let page_size = self.state.table.page_size;
        let decoder = self.state.decoder.clone();

        Task::perform(
            async move {
                let start = Instant::now();
                let result = kafka::consumer::fetch_page(
                    &consumer, &topic, partition, offset, page_size, &decoder,
                )
                .await;
                let elapsed = start.elapsed().as_millis();
                result
                    .map(|data| (data, elapsed))
                    .map_err(|e| e.to_string())
            },
            move |result| Message::PageLoaded(request_id, result),
        )
    }

    /// 统一处理当前页重载，避免空状态误进入加载态
    fn reload_current_page(&mut self) -> Task<Message> {
        if self.state.table.has_search_results() {
            self.state.table.apply_search_page();
            self.sync_detail_content();
            return Task::none();
        }

        if self.consumer.is_some()
            && self.state.sidebar.selected_topic.is_some()
            && self.state.sidebar.selected_partition.is_some()
        {
            self.state.table.begin_loading();
            let request_id = self.next_page_request_id();
            self.fetch_current_page(request_id)
        } else {
            self.state.table.clear_feedback();
            Task::none()
        }
    }

    fn begin_topic_search(&mut self) -> Task<Message> {
        let query = self.state.table.search_query.trim().to_string();
        if query.is_empty() {
            self.invalidate_search_requests();
            if self.state.table.has_search_results() || self.state.table.search_in_progress {
                self.state.table.clear_search_results();
                return self.reload_current_page();
            }
            self.sync_detail_content();
            return Task::none();
        }

        if self.consumer.is_none() {
            self.state.notice = Some(AppNotice::info("请先连接 Kafka 再执行搜索"));
            return Task::none();
        }

        let Some((topic, partitions)) = self.selected_topic_partitions() else {
            self.state.notice = Some(AppNotice::info(
                "请先从左侧选择一个 Topic 对应的 Partition，再执行搜索",
            ));
            return Task::none();
        };

        if partitions.is_empty() {
            self.state.notice = Some(AppNotice::error(format!(
                "Topic {topic} 没有可搜索的 Partition"
            )));
            return Task::none();
        }

        self.invalidate_page_requests();
        self.state.table.begin_partition_search();
        self.state.notice = Some(AppNotice::info(format!(
            "正在搜索 Topic {topic} 的 {} 个 Partition...",
            partitions.len()
        )));

        let request_id = self.next_search_request_id();
        let connection_config = self.state.connection_config.clone();
        let decoder = self.state.decoder.clone();

        Task::perform(
            async move {
                let start = Instant::now();
                let result = kafka::consumer::search_topic(
                    &connection_config,
                    &topic,
                    &partitions,
                    &query,
                    &decoder,
                )
                .await;
                let elapsed = start.elapsed().as_millis();
                result
                    .map(|data| (data, elapsed))
                    .map_err(|error| error.to_string())
            },
            move |result| Message::SearchLoaded(request_id, result),
        )
    }

    fn begin_connect(&mut self, config: ConnectionConfig) -> Task<Message> {
        self.state.notice = None;
        self.state.connection_status = ConnectionStatus::Connecting;

        Task::perform(
            async move {
                let consumer =
                    kafka::connection::create_consumer(&config).map_err(|e| e.to_string())?;
                let topics =
                    kafka::metadata::fetch_metadata(&consumer).map_err(|e| e.to_string())?;
                Ok::<(ConnectionConfig, Vec<kafkax::kafka::types::TopicMeta>), String>((
                    config, topics,
                ))
            },
            |result| match result {
                Ok((config, topics)) => Message::Connected(config, topics),
                Err(error) => Message::ConnectionFailed(error),
            },
        )
    }

    fn copy_selected_json(&mut self) -> Task<Message> {
        let Some(message) = self.state.table.selected_message().cloned() else {
            self.state.notice = Some(AppNotice::info("请先选择一条消息再执行复制"));
            return Task::none();
        };

        self.state.notice = Some(AppNotice::success("已复制当前消息 JSON"));
        clipboard::write::<Message>(message.copyable_json())
    }

    fn sync_detail_content(&mut self) {
        let detail_text = self
            .state
            .table
            .selected_message()
            .map(|message| message.copyable_json())
            .unwrap_or_default();

        self.state.table.set_detail_text(detail_text);
    }

    fn selected_topic_partitions(&self) -> Option<(String, Vec<i32>)> {
        let topic = self.state.sidebar.selected_topic.as_ref()?;
        let mut partitions = self
            .state
            .sidebar
            .topics
            .iter()
            .find(|meta| meta.name == *topic)?
            .partitions
            .iter()
            .map(|partition| partition.id)
            .collect::<Vec<_>>();

        partitions.sort_unstable();
        Some((topic.clone(), partitions))
    }

    fn next_page_request_id(&mut self) -> u64 {
        self.page_request_id = self.page_request_id.wrapping_add(1);
        self.page_request_id
    }

    fn invalidate_page_requests(&mut self) {
        self.page_request_id = self.page_request_id.wrapping_add(1);
    }

    fn next_search_request_id(&mut self) -> u64 {
        self.search_request_id = self.search_request_id.wrapping_add(1);
        self.search_request_id
    }

    fn invalidate_search_requests(&mut self) {
        self.search_request_id = self.search_request_id.wrapping_add(1);
    }

    fn remember_connection(&mut self, connection: &ConnectionConfig) -> Result<()> {
        let mut config = AppConfig {
            connections: self.state.saved_connections.clone(),
            last_connection: self.state.last_connection_name.clone(),
        };

        config.upsert_connection(connection.clone());
        config.save()?;

        self.state.saved_connections = config.connections;
        self.state.last_connection_name = config.last_connection;
        Ok(())
    }

    fn delete_saved_connection(&mut self, name: &str) -> Result<bool> {
        let mut config = AppConfig {
            connections: self.state.saved_connections.clone(),
            last_connection: self.state.last_connection_name.clone(),
        };

        let removed = config.remove_connection(name);
        if !removed {
            return Ok(false);
        }

        config.save()?;
        self.state.saved_connections = config.connections;
        self.state.last_connection_name = config.last_connection;

        if self.state.connection_draft.name == name {
            if let Some(connection) = self.state.saved_connections.first().cloned() {
                self.state.connection_draft = connection;
            } else if self.consumer.is_some() && self.state.connection_config.name == name {
                self.state.connection_draft = self.state.connection_config.clone();
            } else {
                self.state.connection_draft = ConnectionConfig::default();
            }
        }

        Ok(true)
    }

    fn saved_connection_by_name(&self, name: &str) -> Option<ConnectionConfig> {
        self.state
            .saved_connections
            .iter()
            .find(|connection| connection.name == name)
            .cloned()
    }

    fn connection_draft_sasl_mut(&mut self) -> &mut SaslConfig {
        self.state
            .connection_draft
            .sasl
            .get_or_insert_with(SaslConfig::default)
    }

    fn connection_draft_ssl_mut(&mut self) -> &mut SslConfig {
        self.state
            .connection_draft
            .ssl
            .get_or_insert_with(SslConfig::default)
    }

    fn prepare_connection_config(draft: &ConnectionConfig) -> Result<ConnectionConfig, String> {
        let brokers = draft.brokers.trim().to_string();
        if brokers.is_empty() {
            return Err("Broker 地址不能为空".to_string());
        }

        let name = if draft.name.trim().is_empty() {
            brokers.clone()
        } else {
            draft.name.trim().to_string()
        };

        let group_id = draft
            .group_id
            .as_deref()
            .map(str::trim)
            .filter(|group_id| !group_id.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("kafkax-{}", uuid::Uuid::new_v4()));

        let sasl = if draft.security_protocol.uses_sasl() {
            let sasl = draft.sasl.clone().unwrap_or_default();
            let username = sasl.username.trim().to_string();
            if username.is_empty() {
                return Err("SASL 用户名不能为空".to_string());
            }
            if sasl.password.is_empty() {
                return Err("SASL 密码不能为空".to_string());
            }

            Some(SaslConfig {
                mechanism: sasl.mechanism,
                username,
                password: sasl.password,
            })
        } else {
            None
        };

        let ssl = if draft.security_protocol.uses_ssl() {
            let ssl = draft.ssl.clone().unwrap_or_default();
            let ca_location = normalize_optional_string(ssl.ca_location);
            let cert_location = normalize_optional_string(ssl.cert_location);
            let key_location = normalize_optional_string(ssl.key_location);
            let key_password = normalize_optional_string(ssl.key_password);

            if cert_location.is_some() != key_location.is_some() {
                return Err("启用客户端证书时，请同时填写证书路径和私钥路径".to_string());
            }
            if key_password.is_some() && key_location.is_none() {
                return Err("填写私钥口令前，请先填写私钥路径".to_string());
            }

            if ca_location.is_some()
                || cert_location.is_some()
                || key_location.is_some()
                || key_password.is_some()
            {
                Some(SslConfig {
                    ca_location,
                    cert_location,
                    key_location,
                    key_password,
                })
            } else {
                None
            }
        } else {
            None
        };

        Ok(ConnectionConfig {
            name,
            brokers,
            security_protocol: draft.security_protocol,
            sasl,
            ssl,
            group_id: Some(group_id),
        })
    }

    /// 键盘订阅
    pub fn subscription(&self) -> Subscription<Message> {
        keyboard::listen().map(|event| match event {
            keyboard::Event::KeyPressed { key, modifiers, .. } => {
                Message::KeyPressed(key, modifiers)
            }
            _ => Message::Noop,
        })
    }

    /// 处理键盘快捷键
    fn handle_key(&mut self, key: keyboard::Key, modifiers: keyboard::Modifiers) -> Task<Message> {
        use keyboard::Key;
        use keyboard::key::Named;

        if self.state.show_connection_dialog {
            return match key {
                Key::Named(Named::Escape) => {
                    self.state.show_connection_dialog = false;
                    Task::none()
                }
                _ => Task::none(),
            };
        }

        match key {
            // Escape - 关闭对话框 / 取消选中
            Key::Named(Named::Escape) => {
                self.state.table.selected_index = None;
                self.sync_detail_content();
                Task::none()
            }

            // Ctrl+F - 聚焦搜索（暂时只清空触发）
            Key::Character(ref c) if c.eq_ignore_ascii_case("f") && modifiers.command() => {
                // 搜索框会自动获取焦点，此处清空搜索以提示用户
                Task::none()
            }

            // 方向键上 - 选中上一条
            Key::Named(Named::ArrowUp) => {
                if !modifiers.is_empty() {
                    return Task::none();
                }
                if let Some(idx) = self.state.table.selected_index {
                    if idx > 0 {
                        self.state.table.selected_index = Some(idx - 1);
                    }
                } else if !self.state.table.messages.is_empty() {
                    self.state.table.selected_index = Some(0);
                }
                self.sync_detail_content();
                Task::none()
            }

            // 方向键下 - 选中下一条
            Key::Named(Named::ArrowDown) => {
                if !modifiers.is_empty() {
                    return Task::none();
                }
                let len = self.state.table.messages.len();
                if let Some(idx) = self.state.table.selected_index {
                    if idx + 1 < len {
                        self.state.table.selected_index = Some(idx + 1);
                    }
                } else if len > 0 {
                    self.state.table.selected_index = Some(0);
                }
                self.sync_detail_content();
                Task::none()
            }

            // 方向键左 - 上一页
            Key::Named(Named::ArrowLeft) if modifiers.alt() => self.update(Message::PrevPage),

            // 方向键右 - 下一页
            Key::Named(Named::ArrowRight) if modifiers.alt() => self.update(Message::NextPage),

            // Home - 首页
            Key::Named(Named::Home) if modifiers.command() => self.update(Message::FirstPage),

            // End - 末页
            Key::Named(Named::End) if modifiers.command() => self.update(Message::LastPage),

            _ => Task::none(),
        }
    }
}

fn optional_text(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::App;
    use kafkax::config::{
        ConnectionConfig, SaslConfig, SaslMechanism, SecurityProtocol, SslConfig,
    };
    use kafkax::message::Message;

    #[test]
    fn page_size_change_without_selection_does_not_leave_loading_state() {
        let (mut app, _) = App::new();

        let _task = app.update(Message::PageSizeChanged(200));

        assert_eq!(app.state.table.page_size, 200);
        assert!(!app.state.table.loading);
        assert!(app.state.table.error_message.is_none());
    }

    #[test]
    fn page_load_error_is_kept_in_state_for_ui_feedback() {
        let (mut app, _) = App::new();
        app.state.table.begin_loading();
        app.page_request_id = 1;

        let _task = app.update(Message::PageLoaded(1, Err("load failed".to_string())));

        assert!(!app.state.table.loading);
        assert_eq!(
            app.state.table.error_message.as_deref(),
            Some("load failed")
        );
    }

    #[test]
    fn clearing_search_query_restores_browse_mode() {
        let (mut app, _) = App::new();
        app.state.table.current_page = 3;
        app.state.table.search_query = "order".to_string();
        app.state.table.begin_partition_search();
        app.state.table.apply_search_results(vec![], 20, 0, 20, 9);

        let _task = app.update(Message::SearchInputChanged(String::new()));

        assert!(app.state.table.search_results.is_none());
        assert_eq!(app.state.table.current_page, 3);
    }

    #[test]
    fn prepare_connection_config_requires_sasl_credentials() {
        let draft = ConnectionConfig {
            brokers: "localhost:9092".to_string(),
            security_protocol: SecurityProtocol::SaslPlaintext,
            sasl: Some(SaslConfig {
                mechanism: SaslMechanism::Plain,
                username: String::new(),
                password: String::new(),
            }),
            ..ConnectionConfig::default()
        };

        let error = App::prepare_connection_config(&draft).unwrap_err();

        assert_eq!(error, "SASL 用户名不能为空");
    }

    #[test]
    fn prepare_connection_config_generates_group_id_and_validates_ssl_pair() {
        let valid = ConnectionConfig {
            name: String::new(),
            brokers: "localhost:9092".to_string(),
            security_protocol: SecurityProtocol::Plaintext,
            ..ConnectionConfig::default()
        };

        let normalized = App::prepare_connection_config(&valid).unwrap();
        assert_eq!(normalized.name, "localhost:9092");
        assert!(
            normalized
                .group_id
                .as_deref()
                .is_some_and(|value| value.starts_with("kafkax-"))
        );

        let invalid_ssl = ConnectionConfig {
            brokers: "localhost:9092".to_string(),
            security_protocol: SecurityProtocol::SaslSsl,
            sasl: Some(SaslConfig {
                mechanism: SaslMechanism::Plain,
                username: "user".to_string(),
                password: "pass".to_string(),
            }),
            ssl: Some(SslConfig {
                cert_location: Some("/tmp/client.crt".to_string()),
                ..SslConfig::default()
            }),
            ..ConnectionConfig::default()
        };

        let error = App::prepare_connection_config(&invalid_ssl).unwrap_err();
        assert_eq!(error, "启用客户端证书时，请同时填写证书路径和私钥路径");
    }
}
