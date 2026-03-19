use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::message::Message;
use crate::state::SidebarState;
use crate::theme::AppColors;

/// 渲染侧边栏
pub fn view(state: &SidebarState) -> Element<'_, Message> {
    let mut content = column![].spacing(2).padding(8);

    if state.topics.is_empty() {
        content = content.push(
            text("无 Topic 数据")
                .size(13)
                .color(AppColors::TEXT_MUTED),
        );
    } else {
        for topic in &state.topics {
            let is_expanded = state.expanded.contains(&topic.name);
            let icon = if is_expanded { "▼" } else { "▶" };

            // Topic 行
            let topic_btn = button(
                row![
                    text(icon).size(11),
                    text(&topic.name).size(13).color(AppColors::TEXT_PRIMARY),
                ]
                .spacing(6),
            )
            .on_press(Message::ToggleTopic(topic.name.clone()))
            .style(|theme: &Theme, status| {
                let mut style = button::text(theme, status);
                style.background = Some(Background::Color(Color::TRANSPARENT));
                style
            })
            .padding([4, 8])
            .width(Length::Fill);

            content = content.push(topic_btn);

            // 展开时显示 partition
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
                            Space::with_width(16),
                            text(indicator).size(10).color(AppColors::ACCENT),
                            text(format!("P-{}", part.id))
                                .size(12)
                                .color(if is_selected {
                                    AppColors::TEXT_PRIMARY
                                } else {
                                    AppColors::TEXT_SECONDARY
                                }),
                        ]
                        .spacing(6),
                    )
                    .on_press(Message::SelectPartition(
                        topic.name.clone(),
                        part.id,
                    ))
                    .style(move |theme: &Theme, status| {
                        let mut style = button::text(theme, status);
                        style.background = Some(Background::Color(bg_color));
                        if matches!(status, button::Status::Hovered) && !is_selected {
                            style.background = Some(Background::Color(AppColors::ROW_HOVER));
                        }
                        style
                    })
                    .padding([3, 8])
                    .width(Length::Fill);

                    content = content.push(part_btn);
                }
            }
        }
    }

    // 底部添加新连接按钮
    let new_conn_btn = button(
        row![
            text("+").size(14).color(AppColors::ACCENT),
            text("新建连接").size(13).color(AppColors::TEXT_SECONDARY),
        ]
        .spacing(6),
    )
    .on_press(Message::ShowConnectionDialog)
    .style(|theme: &Theme, status| {
        let mut style = button::text(theme, status);
        style.background = Some(Background::Color(Color::TRANSPARENT));
        style
    })
    .padding([8, 8]);

    let sidebar_content = column![
        scrollable(content).height(Length::Fill),
        container(new_conn_btn).width(Length::Fill),
    ];

    container(sidebar_content)
        .width(200)
        .height(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(AppColors::BG_SECONDARY)),
            border: Border {
                color: AppColors::BORDER,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}
