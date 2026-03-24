// src/ui/send_message_dialog.rs
use iced::widget::{button, column, container, row, text, text_editor};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::kafka::producer::parse_json_to_payloads;
use crate::message::Message;
use crate::theme::AppColors;

/// 渲染发送消息弹窗（居中显示，使用多行 JSON 编辑器）
#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    topic: &'a str,
    partition: i32,
    input: &'a str,
    content: &'a text_editor::Content,
    sending: bool,
    error: Option<&'a str>,
) -> Element<'a, Message> {
    // 实时解析 JSON，计算条数
    let parse_result = if input.trim().is_empty() {
        None
    } else {
        Some(parse_json_to_payloads(input))
    };

    let send_label = match &parse_result {
        Some(Ok(payloads)) if !payloads.is_empty() => {
            if sending {
                "发送中...".to_string()
            } else {
                format!("发送 ({} 条)", payloads.len())
            }
        }
        _ => {
            if sending {
                "发送中...".to_string()
            } else {
                "发送".to_string()
            }
        }
    };

    let can_send = !sending && matches!(&parse_result, Some(Ok(p)) if !p.is_empty());
    let can_format = !sending && matches!(&parse_result, Some(Ok(_)));

    // 错误提示（优先显示发送错误，再显示解析错误）
    let error_msg: Option<String> = error.map(|e| e.to_string()).or_else(|| {
        if let Some(Err(e)) = &parse_result {
            Some(format!("JSON 格式错误: {}", e))
        } else {
            None
        }
    });

    let title = text(format!("发送消息到 {} / P-{}", topic, partition))
        .size(15)
        .color(AppColors::TEXT_PRIMARY);

    let editor = text_editor(content)
        .on_action(Message::SendMessageAction)
        .placeholder("输入 JSON 对象 {} 或数组 [{},{}]")
        .height(340)
        .padding([10, 12])
        .size(13)
        .style(|_theme: &Theme, status| text_editor::Style {
            background: Background::Color(AppColors::BG_PRIMARY),
            border: Border {
                color: if matches!(status, text_editor::Status::Focused { .. }) {
                    AppColors::ACCENT
                } else {
                    AppColors::BORDER
                },
                width: 1.0,
                radius: 8.0.into(),
            },
            placeholder: AppColors::TEXT_MUTED,
            value: AppColors::TEXT_PRIMARY,
            selection: Color::from_rgba(
                AppColors::ACCENT.r,
                AppColors::ACCENT.g,
                AppColors::ACCENT.b,
                0.3,
            ),
        });

    let error_row = if let Some(msg) = error_msg {
        column![text(msg).size(12).color(AppColors::ERROR)]
    } else {
        column![]
    };

    let cancel_btn = button(text("取消").size(13).color(AppColors::TEXT_SECONDARY))
        .on_press(Message::CloseOverlay)
        .style(|_theme: &Theme, status| button::Style {
            background: Some(Background::Color(
                if matches!(status, button::Status::Hovered) {
                    AppColors::ROW_HOVER
                } else {
                    AppColors::BG_TERTIARY
                },
            )),
            border: Border {
                color: AppColors::BORDER,
                width: 1.0,
                radius: 8.0.into(),
            },
            text_color: AppColors::TEXT_SECONDARY,
            ..Default::default()
        })
        .padding([8, 16]);

    let format_btn_base =
        button(text("格式化").size(13).color(AppColors::TEXT_SECONDARY)).style(
            |_theme: &Theme, status| button::Style {
                background: Some(Background::Color(
                    if matches!(status, button::Status::Hovered) {
                        AppColors::ROW_HOVER
                    } else {
                        AppColors::BG_TERTIARY
                    },
                )),
                border: Border {
                    color: AppColors::BORDER,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                text_color: AppColors::TEXT_SECONDARY,
                ..Default::default()
            },
        )
        .padding([8, 16]);

    let format_btn: iced::widget::Button<'_, Message> = if can_format {
        format_btn_base.on_press(Message::FormatSendMessageJson)
    } else {
        format_btn_base
    };

    let send_btn_base = button(text(send_label).size(13).color(Color::WHITE)).style(
        |_theme: &Theme, status| button::Style {
            background: Some(Background::Color(
                if matches!(status, button::Status::Hovered) {
                    AppColors::ACCENT_HOVER
                } else {
                    AppColors::ACCENT
                },
            )),
            border: Border {
                color: AppColors::ACCENT,
                width: 1.0,
                radius: 8.0.into(),
            },
            text_color: Color::WHITE,
            ..Default::default()
        },
    )
    .padding([8, 16]);

    let send_btn: iced::widget::Button<'_, Message> = if can_send {
        send_btn_base.on_press(Message::SendMessages)
    } else {
        send_btn_base
    };

    let btn_row = row![cancel_btn, format_btn, send_btn].spacing(8);

    let dialog = container(
        column![title, editor, error_row, btn_row].spacing(12),
    )
    .width(Length::Fixed(780.0))
    .padding(24)
    .style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(AppColors::BG_SECONDARY)),
        border: Border {
            color: AppColors::BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    });

    // 居中显示
    container(dialog)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
