use iced::widget::{
    button, column, container, row, text, text_input, Space,
};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::message::Message;
use crate::state::ConnectionStatus;
use crate::theme::AppColors;

/// 渲染连接对话框
pub fn view<'a>(
    broker_input: &'a str,
    connection_status: &'a ConnectionStatus,
) -> Element<'a, Message> {
    let status_text = match connection_status {
        ConnectionStatus::Disconnected => text("未连接").color(AppColors::TEXT_MUTED).size(12),
        ConnectionStatus::Connecting => text("连接中...").color(AppColors::WARNING).size(12),
        ConnectionStatus::Connected(name) => {
            text(format!("已连接: {}", name)).color(AppColors::SUCCESS).size(12)
        }
        ConnectionStatus::Error(e) => text(format!("错误: {}", e)).color(AppColors::ERROR).size(12),
    };

    let broker_label = text("Broker 地址:").size(13).color(AppColors::TEXT_PRIMARY);
    let broker_field = text_input("localhost:9092", broker_input)
        .on_input(Message::BrokerInputChanged)
        .on_submit(Message::Connect)
        .padding(8)
        .size(14);

    let connect_btn = button(
        text("连接")
            .size(14)
            .color(Color::WHITE),
    )
    .on_press(Message::Connect)
    .style(|_theme: &Theme, status| {
        let bg = match status {
            button::Status::Hovered => AppColors::ACCENT_HOVER,
            _ => AppColors::ACCENT,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            text_color: Color::WHITE,
            ..Default::default()
        }
    })
    .padding([8, 24]);

    let close_btn = button(
        text("取消").size(13).color(AppColors::TEXT_SECONDARY),
    )
    .on_press(Message::CloseConnectionDialog)
    .style(|theme: &Theme, status| {
        let mut style = button::text(theme, status);
        style.background = Some(Background::Color(Color::TRANSPARENT));
        style
    })
    .padding([8, 16]);

    let dialog_content = column![
        text("连接 Kafka 集群").size(18).color(AppColors::TEXT_PRIMARY),
        Space::with_height(16),
        broker_label,
        broker_field,
        Space::with_height(8),
        status_text,
        Space::with_height(16),
        row![connect_btn, Space::with_width(8), close_btn].align_y(iced::Alignment::Center),
    ]
    .spacing(6)
    .padding(24)
    .max_width(400);

    // 居中的对话框
    let dialog_box = container(dialog_content)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(AppColors::BG_SECONDARY)),
            border: Border {
                color: AppColors::BORDER,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        });

    container(dialog_box)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.5))),
            ..Default::default()
        })
        .into()
}
