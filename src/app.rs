use std::sync::Arc;
use std::time::Instant;

use iced::widget::{column, container, row};
use iced::{Element, Length, Subscription, Task, Theme};
use iced::keyboard;
use rdkafka::consumer::StreamConsumer;

use kafkax::codec::DecoderPipeline;
use kafkax::config::ConnectionConfig;
use kafkax::kafka;
use kafkax::message::Message;
use kafkax::state::{AppState, ConnectionStatus};
use kafkax::theme;
use kafkax::ui;

/// 顶层应用
pub struct App {
    pub state: AppState,
    /// Kafka consumer（Arc 用于在异步任务中共享）
    pub consumer: Option<Arc<StreamConsumer>>,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let app = Self {
            state: AppState::default(),
            consumer: None,
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
                self.state.broker_input = input;
                Task::none()
            }
            Message::Connect => {
                let brokers = self.state.broker_input.clone();
                self.state.connection_status = ConnectionStatus::Connecting;

                let config = ConnectionConfig {
                    name: brokers.clone(),
                    brokers: brokers.clone(),
                    ..Default::default()
                };
                self.state.connection_config = config.clone();

                Task::perform(
                    async move {
                        // 在异步块中创建 consumer 并获取元数据
                        let consumer = kafka::connection::create_consumer(&config)
                            .map_err(|e| e.to_string())?;
                        let topics = kafka::metadata::fetch_metadata(&consumer)
                            .map_err(|e| e.to_string())?;
                        Ok::<(StreamConsumer, Vec<kafkax::kafka::types::TopicMeta>), String>(
                            (consumer, topics),
                        )
                    },
                    |result| match result {
                        Ok((_consumer, topics)) => Message::Connected(topics),
                        Err(e) => Message::ConnectionFailed(e),
                    },
                )
            }
            Message::Connected(topics) => {
                let name = self.state.connection_config.name.clone();

                // 重新创建 consumer 用于后续消费
                match kafka::connection::create_consumer(&self.state.connection_config) {
                    Ok(consumer) => {
                        self.consumer = Some(Arc::new(consumer));
                    }
                    Err(e) => {
                        self.state.connection_status =
                            ConnectionStatus::Error(format!("创建消费者失败: {}", e));
                        return Task::none();
                    }
                }

                self.state.connection_status = ConnectionStatus::Connected(name);
                self.state.sidebar.topics = topics;
                self.state.show_connection_dialog = false;
                tracing::info!(
                    "连接成功，共 {} 个 topic",
                    self.state.sidebar.topics.len()
                );
                Task::none()
            }
            Message::ConnectionFailed(error) => {
                self.state.connection_status = ConnectionStatus::Error(error.clone());
                tracing::error!("连接失败: {}", error);
                Task::none()
            }
            Message::Disconnect => {
                self.consumer = None;
                self.state.connection_status = ConnectionStatus::Disconnected;
                self.state.sidebar = Default::default();
                self.state.table = Default::default();
                Task::none()
            }

            // --- 侧边栏 ---
            Message::ToggleTopic(topic) => {
                self.state.sidebar.toggle_topic(&topic);
                Task::none()
            }
            Message::SelectPartition(topic, partition) => {
                self.state.sidebar.select_partition(&topic, partition);
                self.state.table.loading = true;
                self.state.table.current_page = 0;
                self.state.table.selected_index = None;

                self.fetch_current_page()
            }

            // --- 数据加载 ---
            Message::PageLoaded(result) => match result {
                Ok((page_data, load_time)) => {
                    self.state.table.apply_page_data(page_data, load_time);
                    Task::none()
                }
                Err(e) => {
                    self.state.table.loading = false;
                    tracing::error!("加载消息失败: {}", e);
                    Task::none()
                }
            },
            Message::Loading => {
                self.state.table.loading = true;
                Task::none()
            }

            // --- 分页 ---
            Message::FirstPage => {
                self.state.table.current_page = 0;
                self.state.table.loading = true;
                self.fetch_current_page()
            }
            Message::PrevPage => {
                if self.state.table.has_prev() {
                    self.state.table.current_page -= 1;
                    self.state.table.loading = true;
                    self.fetch_current_page()
                } else {
                    Task::none()
                }
            }
            Message::NextPage => {
                if self.state.table.has_next() {
                    self.state.table.current_page += 1;
                    self.state.table.loading = true;
                    self.fetch_current_page()
                } else {
                    Task::none()
                }
            }
            Message::LastPage => {
                let total = self.state.table.total_pages();
                if total > 0 {
                    self.state.table.current_page = total - 1;
                    self.state.table.loading = true;
                    self.fetch_current_page()
                } else {
                    Task::none()
                }
            }
            Message::GoToPage(page) => {
                if page < self.state.table.total_pages() {
                    self.state.table.current_page = page;
                    self.state.table.loading = true;
                    self.fetch_current_page()
                } else {
                    Task::none()
                }
            }
            Message::PageSizeChanged(size) => {
                self.state.table.page_size = size;
                self.state.table.current_page = 0;
                self.state.table.loading = true;
                self.fetch_current_page()
            }

            // --- 消息选中 ---
            Message::SelectMessage(idx) => {
                self.state.table.selected_index = Some(idx);
                Task::none()
            }

            // --- 解码器 ---
            Message::DecoderChanged(decoder_type) => {
                self.state.decoder = DecoderPipeline::new(decoder_type);
                // 重新加载当前页以应用新解码器
                if self.state.sidebar.selected_topic.is_some() {
                    self.state.table.loading = true;
                    self.fetch_current_page()
                } else {
                    Task::none()
                }
            }

            // --- 搜索 ---
            Message::SearchInputChanged(query) => {
                self.state.table.search_query = query;
                Task::none()
            }
            Message::Search => {
                // 搜索在表格 view 中已实时过滤高亮，此处无需额外操作
                Task::none()
            }

            // --- 键盘快捷键 ---
            Message::KeyPressed(key, modifiers) => self.handle_key(key, modifiers),

            Message::Noop => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        // 主布局
        let sidebar = ui::sidebar::view(&self.state.sidebar);

        let toolbar = ui::toolbar::view(&self.state.table, self.state.decoder.selected);
        let table = ui::message_table::view(&self.state.table);
        let detail = ui::message_detail::view(&self.state.table);

        let main_content = column![toolbar, table, detail].spacing(0);

        let status_bar = ui::status_bar::view(
            &self.state.connection_status,
            &self.state.table,
            self.state.sidebar.selected_topic.as_deref(),
            self.state.sidebar.selected_partition,
        );

        let body = column![
            row![sidebar, main_content].height(Length::Fill),
            status_bar,
        ];

        let page: Element<'_, Message> = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        // 连接对话框覆盖层
        if self.state.show_connection_dialog {
            let overlay = ui::connection_dialog::view(
                &self.state.broker_input,
                &self.state.connection_status,
            );
            iced::widget::stack![page, overlay].into()
        } else {
            page
        }
    }

    /// 获取当前选中 partition 的当前页数据
    fn fetch_current_page(&self) -> Task<Message> {
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
                result.map(|data| (data, elapsed)).map_err(|e| e.to_string())
            },
            Message::PageLoaded,
        )
    }

    /// 键盘订阅
    pub fn subscription(&self) -> Subscription<Message> {
        keyboard::on_key_press(|key, modifiers| {
            Some(Message::KeyPressed(key, modifiers))
        })
    }

    /// 处理键盘快捷键
    fn handle_key(&mut self, key: keyboard::Key, modifiers: keyboard::Modifiers) -> Task<Message> {
        use keyboard::key::Named;
        use keyboard::Key;

        match key {
            // Escape - 关闭对话框 / 取消选中
            Key::Named(Named::Escape) => {
                if self.state.show_connection_dialog {
                    self.state.show_connection_dialog = false;
                } else {
                    self.state.table.selected_index = None;
                }
                Task::none()
            }

            // Ctrl+F - 聚焦搜索（暂时只清空触发）
            Key::Character(ref c) if c.as_str() == "f" && modifiers.command() => {
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
                Task::none()
            }

            // 方向键左 - 上一页
            Key::Named(Named::ArrowLeft) if modifiers.alt() => {
                self.update(Message::PrevPage)
            }

            // 方向键右 - 下一页
            Key::Named(Named::ArrowRight) if modifiers.alt() => {
                self.update(Message::NextPage)
            }

            // Home - 首页
            Key::Named(Named::Home) if modifiers.command() => {
                self.update(Message::FirstPage)
            }

            // End - 末页
            Key::Named(Named::End) if modifiers.command() => {
                self.update(Message::LastPage)
            }

            _ => Task::none(),
        }
    }
}
