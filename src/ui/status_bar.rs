use iced::widget::{container, row, text, Space};
use iced::{Background, Border, Element, Length, Theme};

use crate::message::Message;
use crate::state::{ConnectionStatus, TableState};
use crate::theme::AppColors;

/// 渲染底部状态栏
pub fn view<'a>(
    connection_status: &'a ConnectionStatus,
    table: &'a TableState,
    selected_topic: Option<&'a str>,
    selected_partition: Option<i32>,
) -> Element<'a, Message> {
    let conn_indicator = match connection_status {
        ConnectionStatus::Connected(name) => row![
            text("✓").size(12).color(AppColors::SUCCESS),
            text(format!(" 已连接: {}", name)).size(11).color(AppColors::TEXT_SECONDARY),
        ],
        ConnectionStatus::Connecting => row![
            text("◌").size(12).color(AppColors::WARNING),
            text(" 连接中...").size(11).color(AppColors::TEXT_SECONDARY),
        ],
        ConnectionStatus::Error(e) => row![
            text("✗").size(12).color(AppColors::ERROR),
            text(format!(" {}", e)).size(11).color(AppColors::ERROR),
        ],
        ConnectionStatus::Disconnected => row![
            text("○").size(12).color(AppColors::TEXT_MUTED),
            text(" 未连接").size(11).color(AppColors::TEXT_MUTED),
        ],
    };

    let partition_info = match (selected_topic, selected_partition) {
        (Some(topic), Some(part)) => {
            text(format!("{} P-{}", topic, part))
                .size(11)
                .color(AppColors::TEXT_SECONDARY)
        }
        _ => text("").size(11),
    };

    let message_count = if table.total_messages > 0 {
        text(format!("消息: {}", table.total_messages))
            .size(11)
            .color(AppColors::TEXT_SECONDARY)
    } else {
        text("").size(11)
    };

    let load_time = match table.load_time_ms {
        Some(ms) => text(format!("加载: {}ms", ms))
            .size(11)
            .color(AppColors::TEXT_SECONDARY),
        None => text("").size(11),
    };

    let status = row![
        conn_indicator,
        Space::with_width(16),
        partition_info,
        Space::with_width(16),
        message_count,
        Space::with_width(Length::Fill),
        load_time,
    ]
    .spacing(0)
    .padding([4, 12])
    .align_y(iced::Alignment::Center);

    container(status)
        .width(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(AppColors::BG_TERTIARY)),
            border: Border {
                color: AppColors::BORDER,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}
