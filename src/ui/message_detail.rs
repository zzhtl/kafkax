use iced::widget::{column, container, scrollable, text};
use iced::{Background, Border, Element, Length, Theme};

use crate::kafka::types::DecodedPayload;
use crate::message::Message;
use crate::state::TableState;
use crate::theme::AppColors;

/// 渲染消息详情面板
pub fn view(state: &TableState) -> Element<'_, Message> {
    match state.selected_message() {
        None => container(
            text("选择一条消息查看详情")
                .size(13)
                .color(AppColors::TEXT_MUTED),
        )
        .padding(16)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into(),

        Some(msg) => {
            let header = text(format!(
                "详情 (Offset: {} | Partition: {} | 格式: {})",
                msg.raw.offset,
                msg.raw.partition,
                msg.decoded_value.format_name()
            ))
            .size(12)
            .color(AppColors::TEXT_SECONDARY);

            let body_text = msg.decoded_value.full_display();

            let body = text(body_text)
                .size(13)
                .color(match &msg.decoded_value {
                    DecodedPayload::Json(_) => AppColors::JSON_KEY,
                    DecodedPayload::Error(_) => AppColors::ERROR,
                    _ => AppColors::TEXT_PRIMARY,
                })
                .font(iced::Font::MONOSPACE);

            let detail = column![header, scrollable(container(body).padding(8))]
                .spacing(8)
                .padding(12);

            container(detail)
                .width(Length::Fill)
                .height(200)
                .style(|_theme: &Theme| container::Style {
                    background: Some(Background::Color(AppColors::BG_SECONDARY)),
                    border: Border {
                        color: AppColors::BORDER,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .into()
        }
    }
}
