use iced::widget::{Space, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::message::Message;
use crate::state::{AppNotice, ConnectionStatus, NoticeTone, TableState};
use crate::theme::AppColors;

/// 渲染底部状态栏
pub fn view<'a>(
    connection_status: &'a ConnectionStatus,
    notice: Option<&'a AppNotice>,
    table: &'a TableState,
    selected_topic: Option<&'a str>,
    selected_partition: Option<i32>,
) -> Element<'a, Message> {
    let (badge_label, badge_fg, badge_bg, status_text) = match connection_status {
        ConnectionStatus::Connected(name) => (
            "已连接",
            AppColors::SUCCESS,
            AppColors::SUCCESS_BG,
            name.as_str(),
        ),
        ConnectionStatus::Connecting => (
            "连接中",
            AppColors::WARNING,
            AppColors::WARNING_BG,
            "正在连接 Kafka 集群",
        ),
        ConnectionStatus::Error(e) => (
            "连接失败",
            AppColors::ERROR,
            AppColors::ERROR_BG,
            e.as_str(),
        ),
        ConnectionStatus::Disconnected => (
            "未连接",
            AppColors::TEXT_MUTED,
            AppColors::BG_PRIMARY,
            "请先新建连接",
        ),
    };

    let conn_indicator = row![
        status_badge(badge_label, badge_fg, badge_bg),
        text(status_text).size(11).color(
            if matches!(connection_status, ConnectionStatus::Error(_)) {
                AppColors::ERROR
            } else {
                AppColors::TEXT_SECONDARY
            }
        ),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let partition_info = if table.has_search_results() || table.search_in_progress {
        match selected_topic {
            Some(topic) => text(format!("{topic} / 全部分区搜索"))
                .size(11)
                .color(AppColors::TEXT_SECONDARY),
            None => text("").size(11),
        }
    } else {
        match (selected_topic, selected_partition) {
            (Some(topic), Some(part)) => text(format!("{} / P-{}", topic, part))
                .size(11)
                .color(AppColors::TEXT_SECONDARY),
            _ => text("").size(11),
        }
    };

    let message_count = if let Some(scanned_messages) = table.search_scanned_messages {
        text(format!(
            "命中: {} / 扫描: {}",
            table.total_messages, scanned_messages
        ))
        .size(11)
        .color(AppColors::TEXT_SECONDARY)
    } else if table.total_messages > 0 {
        text(format!("消息: {}", table.total_messages))
            .size(11)
            .color(AppColors::TEXT_SECONDARY)
    } else {
        text("").size(11)
    };

    let page_info = if table.total_messages > 0 {
        text(format!(
            "页码: {}/{}",
            table.current_page + 1,
            table.total_pages()
        ))
        .size(11)
        .color(AppColors::TEXT_SECONDARY)
    } else {
        text("").size(11)
    };

    let mut right_section = row![].spacing(10).align_y(iced::Alignment::Center);

    if let Some(notice) = notice {
        right_section = right_section.push(notice_badge(notice));
    }

    if let Some(ms) = table.load_time_ms {
        right_section = right_section.push(
            text(format!("加载: {}ms", ms))
                .size(11)
                .color(AppColors::TEXT_SECONDARY),
        );
    }

    let status = row![
        conn_indicator,
        Space::new().width(16),
        partition_info,
        Space::new().width(16),
        message_count,
        Space::new().width(16),
        page_info,
        Space::new().width(Length::Fill),
        right_section,
    ]
    .spacing(0)
    .padding([8, 14])
    .align_y(iced::Alignment::Center);

    container(status)
        .width(Length::Fill)
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

fn status_badge<'a>(label: &'a str, text_color: Color, background: Color) -> Element<'a, Message> {
    container(text(label).size(10).color(text_color))
        .padding([4, 8])
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(background)),
            border: Border {
                color: background,
                width: 1.0,
                radius: 999.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn notice_badge<'a>(notice: &'a AppNotice) -> Element<'a, Message> {
    let (text_color, background) = match notice.tone {
        NoticeTone::Info => (AppColors::ACCENT, AppColors::ACCENT_SOFT),
        NoticeTone::Success => (AppColors::SUCCESS, AppColors::SUCCESS_BG),
        NoticeTone::Error => (AppColors::ERROR, AppColors::ERROR_BG),
    };

    container(text(&notice.message).size(11).color(text_color))
        .padding([4, 10])
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(background)),
            border: Border {
                color: background,
                width: 1.0,
                radius: 999.0.into(),
            },
            ..Default::default()
        })
        .into()
}
