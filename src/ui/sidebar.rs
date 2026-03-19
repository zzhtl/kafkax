use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::message::Message;
use crate::state::{ConnectionStatus, SidebarState};
use crate::theme::AppColors;

/// 渲染侧边栏
pub fn view<'a>(
    state: &'a SidebarState,
    connection_status: &'a ConnectionStatus,
    saved_connection_count: usize,
) -> Element<'a, Message> {
    let search_query = state.search_query.trim();
    let search_active = !search_query.is_empty();
    let visible_topics = state
        .topics
        .iter()
        .filter(|topic| state.topic_matches(&topic.name))
        .collect::<Vec<_>>();

    let mut content = column![].spacing(4).padding([4, 0]);

    if state.topics.is_empty() {
        content = content.push(info_card(
            "暂无 Topic",
            "连接 Kafka 后会在这里显示 Topic 和 Partition。",
        ));
    } else if visible_topics.is_empty() {
        content = content.push(info_card(
            "没有匹配的 Topic",
            "换个关键词试试，支持大小写不敏感匹配。",
        ));
    } else {
        for topic in visible_topics {
            let is_expanded = state.expanded.contains(&topic.name);
            let icon = if is_expanded { "▾" } else { "▸" };

            let topic_btn = button(
                row![
                    text(icon).size(12).color(AppColors::TEXT_MUTED),
                    text(&topic.name).size(13).color(AppColors::TEXT_PRIMARY),
                    Space::new().width(Length::Fill),
                    text(format!("{}", topic.partitions.len()))
                        .size(11)
                        .color(AppColors::TEXT_MUTED),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::ToggleTopic(topic.name.clone()))
            .style(|theme: &Theme, status| {
                let mut style = button::text(theme, status);
                style.background = Some(Background::Color(
                    if matches!(status, button::Status::Hovered) {
                        AppColors::ROW_HOVER
                    } else {
                        Color::TRANSPARENT
                    },
                ));
                style
            })
            .padding([6, 10])
            .width(Length::Fill);

            content = content.push(topic_btn);

            if is_expanded {
                for part in &topic.partitions {
                    let is_selected = state.is_partition_selected(&topic.name, part.id);
                    let bg_color = if is_selected {
                        AppColors::ROW_SELECTED
                    } else {
                        Color::TRANSPARENT
                    };
                    let indicator = if is_selected { "●" } else { " " };

                    let part_btn = button(
                        row![
                            Space::new().width(20),
                            text(indicator).size(10).color(AppColors::ACCENT),
                            text(format!("P-{}", part.id))
                                .size(12)
                                .color(if is_selected {
                                    AppColors::TEXT_PRIMARY
                                } else {
                                    AppColors::TEXT_SECONDARY
                                }),
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    )
                    .on_press(Message::SelectPartition(topic.name.clone(), part.id))
                    .style(move |theme: &Theme, status| {
                        let mut style = button::text(theme, status);
                        style.background = Some(Background::Color(bg_color));
                        if matches!(status, button::Status::Hovered) && !is_selected {
                            style.background = Some(Background::Color(AppColors::ROW_HOVER));
                        }
                        style
                    })
                    .padding([5, 10])
                    .width(Length::Fill);

                    content = content.push(part_btn);
                }
            }
        }
    }

    let search_input = text_input("搜索 Topic", &state.search_query)
        .on_input(Message::TopicSearchInputChanged)
        .padding([8, 10])
        .size(13)
        .width(Length::Fill);

    let search_summary = if state.topics.is_empty() {
        "连接后可按 Topic 名称快速过滤".to_string()
    } else if search_active {
        format!(
            "匹配 {} / {} 个 Topic",
            visible_topics_len(state),
            state.topics.len()
        )
    } else {
        format!("共 {} 个 Topic", state.topics.len())
    };

    let new_conn_btn = button(
        row![
            text("+").size(15).color(Color::WHITE),
            text(
                if matches!(connection_status, ConnectionStatus::Connected(_)) {
                    "切换连接"
                } else {
                    "新建连接"
                }
            )
            .size(13)
            .color(Color::WHITE),
        ]
        .spacing(6),
    )
    .on_press(Message::ShowConnectionDialog)
    .style(|_theme: &Theme, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => AppColors::ACCENT_HOVER,
            _ => AppColors::ACCENT,
        })),
        border: Border {
            color: AppColors::ACCENT,
            width: 1.0,
            radius: 10.0.into(),
        },
        text_color: Color::WHITE,
        ..Default::default()
    })
    .padding([10, 12])
    .width(Length::Fill);

    let connection_summary = match connection_status {
        ConnectionStatus::Connected(name) => format!("当前连接: {}", name),
        ConnectionStatus::Connecting => "当前连接: 正在连接".to_string(),
        ConnectionStatus::Error(_) => "当前连接: 最近一次连接失败".to_string(),
        ConnectionStatus::Disconnected => "当前连接: 未连接".to_string(),
    };

    let saved_summary = if saved_connection_count > 0 {
        format!("已保存 {} 个连接，可直接复用", saved_connection_count)
    } else {
        "连接成功后会自动保存到本机".to_string()
    };

    let header = column![
        text("KafkaX").size(20).color(AppColors::TEXT_PRIMARY),
        text("选择 Topic / Partition 后查看消息")
            .size(12)
            .color(AppColors::TEXT_MUTED),
        container(
            column![
                text(connection_summary)
                    .size(12)
                    .color(AppColors::TEXT_PRIMARY),
                text(saved_summary).size(11).color(AppColors::TEXT_MUTED),
            ]
            .spacing(4),
        )
        .padding(12)
        .style(card_style),
        container(
            column![
                search_input,
                text(search_summary).size(11).color(AppColors::TEXT_MUTED),
            ]
            .spacing(8),
        )
        .padding(12)
        .style(card_style),
    ]
    .spacing(10);

    let sidebar_content = column![
        header,
        container(scrollable(content).height(Length::Fill))
            .padding([12, 0])
            .width(Length::Fill),
        new_conn_btn,
    ];

    container(sidebar_content)
        .width(240)
        .height(Length::Fill)
        .padding(14)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(AppColors::BG_SECONDARY)),
            border: Border {
                color: AppColors::BORDER,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn info_card<'a>(title: &'a str, subtitle: &'a str) -> Element<'a, Message> {
    container(
        column![
            text(title).size(14).color(AppColors::TEXT_SECONDARY),
            text(subtitle).size(12).color(AppColors::TEXT_MUTED),
        ]
        .spacing(6),
    )
    .padding(12)
    .style(card_style)
    .into()
}

fn card_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(AppColors::BG_PRIMARY)),
        border: Border {
            color: AppColors::BORDER,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    }
}

fn visible_topics_len(state: &SidebarState) -> usize {
    state
        .topics
        .iter()
        .filter(|topic| state.topic_matches(&topic.name))
        .count()
}
