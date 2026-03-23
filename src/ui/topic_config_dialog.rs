// src/ui/topic_config_dialog.rs
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::message::Message;
use crate::state::overlay_state::is_valid_retention_input;
use crate::theme::AppColors;

/// 渲染 Topic 配置弹窗（居中显示）
#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    topic: &'a str,
    retention_secs: &'a str,
    retention_gb: &'a str,
    retention_gb_note: Option<&'a str>,
    loading: bool,
    saving: bool,
    purging: bool,
    purge_confirm_pending: bool,
    error: Option<&'a str>,
) -> Element<'a, Message> {
    let busy = loading || saving || purging;
    let input_valid =
        is_valid_retention_input(retention_secs) && is_valid_retention_input(retention_gb);

    let title = text(format!("Topic 配置 - {}", topic))
        .size(15)
        .color(AppColors::TEXT_PRIMARY);

    // 保留时间输入
    let secs_input = text_input(
        if loading { "加载中..." } else { "秒，-1 表示无限制" },
        retention_secs,
    )
    .on_input(Message::TopicConfigRetentionSecsChanged)
    .padding([8, 10])
    .size(13);

    // 磁盘占用输入
    let gb_input = text_input(
        if loading { "加载中..." } else { "GB，-1 表示无限制" },
        retention_gb,
    )
    .on_input(Message::TopicConfigRetentionGbChanged)
    .padding([8, 10])
    .size(13);

    // GB 取整说明
    let note_row = if let Some(note) = retention_gb_note {
        column![
            text(format!("i {}", note))
                .size(11)
                .color(AppColors::TEXT_MUTED)
        ]
    } else {
        column![]
    };

    // 输入验证错误
    let validation_error = if !loading && !input_valid {
        column![
            text("请输入正整数或 -1")
                .size(12)
                .color(Color::from_rgb(0.9, 0.2, 0.2))
        ]
    } else {
        column![]
    };

    // 操作错误
    let error_row = if let Some(msg) = error {
        column![
            text(msg)
                .size(12)
                .color(Color::from_rgb(0.9, 0.2, 0.2))
        ]
    } else {
        column![]
    };

    // 保存按钮
    let can_save = !busy && input_valid;
    let save_label = if saving { "保存中..." } else { "保存配置" };
    let save_btn_base = button(text(save_label).size(13).color(Color::WHITE))
        .style(|_theme: &Theme, status| button::Style {
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
        })
        .padding([8, 16]);
    let save_btn: iced::widget::Button<'_, Message> = if can_save {
        save_btn_base.on_press(Message::SaveTopicConfig)
    } else {
        save_btn_base
    };

    let cancel_btn = button(text("取消").size(13))
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

    let action_row = row![cancel_btn, save_btn].spacing(8);

    // 清空数据按钮
    let purge_label = if purging {
        "清空中..."
    } else if purge_confirm_pending {
        "确认清空？点击确认执行"
    } else {
        "清空当前堆积数据"
    };

    let purge_msg = if purge_confirm_pending {
        Message::ConfirmPurgeTopicData
    } else {
        Message::RequestPurgeTopicData
    };

    let purge_btn_base = button(
        text(purge_label)
            .size(13)
            .color(Color::from_rgb(0.9, 0.2, 0.2)),
    )
    .style(|_theme: &Theme, status| button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered) {
                Color::from_rgba(0.9, 0.2, 0.2, 0.1)
            } else {
                AppColors::BG_TERTIARY
            },
        )),
        border: Border {
            color: Color::from_rgb(0.9, 0.2, 0.2),
            width: 1.0,
            radius: 8.0.into(),
        },
        text_color: Color::from_rgb(0.9, 0.2, 0.2),
        ..Default::default()
    })
    .padding([8, 16]);

    let purge_btn: iced::widget::Button<'_, Message> = if !busy {
        purge_btn_base.on_press(purge_msg)
    } else {
        purge_btn_base
    };

    let danger_section = column![
        text("危险操作").size(12).color(AppColors::TEXT_MUTED),
        purge_btn,
    ]
    .spacing(8);

    let dialog = container(
        column![
            title,
            row![
                text("保留时间（秒）").size(13).color(AppColors::TEXT_SECONDARY),
                secs_input,
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            row![
                text("最大磁盘占用（GB）").size(13).color(AppColors::TEXT_SECONDARY),
                gb_input,
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            note_row,
            text("（-1 表示无限制）").size(11).color(AppColors::TEXT_MUTED),
            validation_error,
            action_row,
            iced::widget::rule::horizontal(1),
            danger_section,
            error_row,
        ]
        .spacing(12),
    )
    .width(Length::Fixed(480.0))
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
