use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::message::Message;
use crate::state::TableState;
use crate::theme::{self, AppColors};

/// 渲染消息详情面板
pub fn view(state: &TableState) -> Element<'_, Message> {
    match state.selected_message() {
        None => container(
            column![
                text("消息详情").size(15).color(AppColors::TEXT_PRIMARY),
                text("选择一条消息后，在这里查看完整 JSON，并可拖拽选择局部内容。")
                    .size(12)
                    .color(AppColors::TEXT_MUTED),
                text("拖拽选中后使用 Ctrl/Cmd+C 复制选区；右上角按钮可复制整条消息 JSON。")
                    .size(11)
                    .color(AppColors::TEXT_MUTED),
            ]
            .spacing(6),
        )
        .padding(16)
        .width(Length::Fill)
        .style(panel_style)
        .into(),

        Some(msg) => {
            let timestamp = msg
                .timestamp
                .map(|ts: chrono::DateTime<chrono::Utc>| {
                    ts.format("%Y-%m-%d %H:%M:%S").to_string()
                })
                .unwrap_or_else(|| "无时间戳".to_string());

            let header = column![
                row![
                    column![
                        text("消息详情").size(15).color(AppColors::TEXT_PRIMARY),
                        text("拖拽选中局部内容时按原文复制，点击按钮时复制完整 JSON。")
                            .size(12)
                            .color(AppColors::TEXT_MUTED),
                    ]
                    .spacing(4),
                    Space::new().width(Length::Fill),
                    compact_button("复制 JSON", Message::CopySelectedMessage),
                ]
                .align_y(iced::Alignment::Center),
                row![
                    meta_badge(msg.topic.clone()),
                    meta_badge(msg.partition_label.clone()),
                    meta_badge(format!("Offset {}", msg.offset)),
                    meta_badge(timestamp),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(10);

            let detail_str = state.detail_text.as_ref().clone();
            let editor = scrollable(
                container(
                    text(detail_str)
                        .font(theme::detail_font())
                        .size(13)
                        .color(AppColors::TEXT_PRIMARY),
                )
                .padding(12)
                .width(Length::Fill)
                .style(|_theme: &Theme| container::Style {
                    background: Some(Background::Color(AppColors::BG_PRIMARY)),
                    border: Border {
                        color: AppColors::BORDER,
                        width: 1.0,
                        radius: 10.0.into(),
                    },
                    ..Default::default()
                }),
            )
            .height(Length::Fill);

            let detail = column![
                header,
                text(if state.detail_loading {
                    "正在后台格式化当前消息，完成后会自动刷新。"
                } else {
                    "点击右上角按钮可复制整条消息 JSON。"
                })
                .size(11)
                .color(AppColors::TEXT_MUTED),
                editor,
            ]
            .spacing(10)
            .padding(12);

            container(detail)
                .width(Length::Fill)
                .height(280)
                .style(panel_style)
                .into()
        }
    }
}

fn compact_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    button(text(label).size(11).color(Color::WHITE))
        .on_press(message)
        .style(move |_theme: &Theme, status| {
            let background = match status {
                button::Status::Hovered => AppColors::ACCENT_HOVER,
                _ => AppColors::ACCENT,
            };

            button::Style {
                background: Some(Background::Color(background)),
                border: Border {
                    color: background,
                    width: 1.0,
                    radius: 999.0.into(),
                },
                text_color: Color::WHITE,
                ..Default::default()
            }
        })
        .padding([6, 12])
        .into()
}

fn meta_badge<'a>(label: String) -> Element<'a, Message> {
    container(text(label).size(11).color(AppColors::TEXT_SECONDARY))
        .padding([4, 10])
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(AppColors::BG_PRIMARY)),
            border: Border {
                color: AppColors::BORDER,
                width: 1.0,
                radius: 999.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn panel_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(AppColors::BG_SECONDARY)),
        border: Border {
            color: AppColors::BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}
