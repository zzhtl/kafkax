use iced::widget::{Space, button, column, container, row, scrollable, text, text_editor};
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
                .raw
                .timestamp
                .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M:%S").to_string())
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
                    meta_badge(msg.raw.topic.clone()),
                    meta_badge(format!("P-{}", msg.raw.partition)),
                    meta_badge(format!("Offset {}", msg.raw.offset)),
                    meta_badge(timestamp),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(10);

            let editor = text_editor(&state.detail_content)
                .on_action(Message::DetailEditorAction)
                .font(theme::detail_font())
                .size(13)
                .height(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .style(|theme: &Theme, status| {
                    let mut style = text_editor::default(theme, status);
                    style.background = Background::Color(AppColors::BG_PRIMARY);
                    style.border = Border {
                        color: match status {
                            text_editor::Status::Focused { .. } => AppColors::SEARCH_HIGHLIGHT,
                            _ => AppColors::BORDER,
                        },
                        width: 1.0,
                        radius: 10.0.into(),
                    };
                    style.value = AppColors::TEXT_PRIMARY;
                    style.placeholder = AppColors::TEXT_MUTED;
                    style.selection = AppColors::SEARCH_HIGHLIGHT_BG;
                    style
                });

            let detail = column![
                header,
                text("支持鼠标拖拽选区并直接复制；整条复制统一输出 JSON。")
                    .size(11)
                    .color(AppColors::TEXT_MUTED),
                scrollable(container(editor).height(Length::Fill)),
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
