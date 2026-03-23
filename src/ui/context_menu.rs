// src/ui/context_menu.rs
use iced::widget::{button, column, container, text};
use iced::{Background, Border, Element, Length, Theme};

use crate::message::Message;
use crate::state::overlay_state::ContextMenuTarget;
use crate::theme::AppColors;

const MENU_WIDTH: f32 = 150.0;
const ITEM_HEIGHT: f32 = 36.0;
const MENU_PADDING: f32 = 4.0;

/// 渲染右键浮层菜单
/// x, y 为鼠标点击坐标，window_size 用于边界规避
pub fn view<'a>(
    x: f32,
    y: f32,
    target: &'a ContextMenuTarget,
    window_size: (f32, f32),
) -> Element<'a, Message> {
    let items: Vec<(&str, Message)> = match target {
        ContextMenuTarget::Topic { name, partitions } => {
            vec![
                (
                    "Topic 配置",
                    Message::OpenTopicConfig {
                        topic: name.clone(),
                        partitions: partitions.clone(),
                    },
                ),
                // "清空数据" 直接打开 TopicConfig 弹窗，在弹窗内执行清空操作
                (
                    "清空数据",
                    Message::OpenTopicConfig {
                        topic: name.clone(),
                        partitions: partitions.clone(),
                    },
                ),
                ("关闭", Message::CloseOverlay),
            ]
        }
        ContextMenuTarget::Partition { topic, partition } => vec![
            (
                "发送消息",
                Message::OpenSendMessage {
                    topic: topic.clone(),
                    partition: *partition,
                },
            ),
            ("关闭", Message::CloseOverlay),
        ],
    };

    let item_count = items.len() as f32;
    let menu_height = item_count * ITEM_HEIGHT + MENU_PADDING * 2.0;

    // 边界规避：防止菜单超出窗口范围
    let (win_w, win_h) = window_size;
    let final_x = if x + MENU_WIDTH > win_w { x - MENU_WIDTH } else { x };
    let final_y = if y + menu_height > win_h { y - menu_height } else { y };

    let menu_items = items.into_iter().fold(
        column![].spacing(0),
        |col, (label, msg)| {
            let btn = button(
                text(label)
                    .size(13)
                    .color(AppColors::TEXT_PRIMARY),
            )
            .on_press(msg)
            .style(|_theme: &Theme, status| button::Style {
                background: Some(Background::Color(
                    if matches!(status, button::Status::Hovered) {
                        AppColors::ROW_HOVER
                    } else {
                        AppColors::BG_SECONDARY
                    },
                )),
                border: Border::default(),
                text_color: AppColors::TEXT_PRIMARY,
                ..Default::default()
            })
            .padding([8, 14])
            .width(Length::Fill);
            col.push(btn)
        },
    );

    let menu = container(menu_items)
        .width(Length::Fixed(MENU_WIDTH))
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(AppColors::BG_SECONDARY)),
            border: Border {
                color: AppColors::BORDER,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .padding(MENU_PADDING);

    // 使用绝对定位容器（padding 偏移近似坐标定位）
    container(menu)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: final_y,
            left: final_x,
            right: 0.0,
            bottom: 0.0,
        })
        .align_x(iced::alignment::Horizontal::Left)
        .align_y(iced::alignment::Vertical::Top)
        .into()
}
