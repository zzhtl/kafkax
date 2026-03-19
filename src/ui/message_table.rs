use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::message::Message;
use crate::state::TableState;
use crate::theme::AppColors;

/// 渲染消息列表表格
pub fn view(state: &TableState) -> Element<'_, Message> {
    let mut content = column![].spacing(0);

    // 表头
    let header = container(
        row![
            cell_text("Offset".to_string(), 80, AppColors::TEXT_SECONDARY),
            cell_text("Timestamp".to_string(), 150, AppColors::TEXT_SECONDARY),
            cell_text("Key".to_string(), 120, AppColors::TEXT_SECONDARY),
            text("Value").size(12).color(AppColors::TEXT_SECONDARY),
        ]
        .spacing(8)
        .padding([6, 12]),
    )
    .width(Length::Fill)
    .style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(AppColors::BG_TERTIARY)),
        border: Border {
            color: AppColors::BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });

    content = content.push(header);

    if state.loading {
        content = content.push(
            container(text("加载中...").size(14).color(AppColors::TEXT_MUTED))
                .padding(20)
                .width(Length::Fill)
                .center_x(Length::Fill),
        );
    } else if state.messages.is_empty() {
        content = content.push(
            container(
                text("暂无消息数据，请从侧边栏选择一个 Partition")
                    .size(14)
                    .color(AppColors::TEXT_MUTED),
            )
            .padding(40)
            .width(Length::Fill)
            .center_x(Length::Fill),
        );
    } else {
        let search_query = &state.search_query;
        let has_search = !search_query.is_empty();
        let search_lower = search_query.to_lowercase();

        let rows = column(
            state
                .messages
                .iter()
                .enumerate()
                .map(|(idx, msg)| {
                    let is_selected = state.selected_index == Some(idx);

                    let offset_str = format!("{}", msg.raw.offset);
                    let ts_str = msg
                        .raw
                        .timestamp
                        .map(|ts| ts.format("%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "-".to_string());
                    let key_str = msg
                        .decoded_key
                        .as_deref()
                        .unwrap_or("-")
                        .to_string();
                    let value_str = msg.decoded_value.summary(200);

                    // 搜索匹配检测
                    let is_match = if has_search {
                        key_str.to_lowercase().contains(&search_lower)
                            || value_str.to_lowercase().contains(&search_lower)
                            || offset_str.contains(&search_lower)
                    } else {
                        false
                    };

                    let bg_color = if is_selected {
                        AppColors::ROW_SELECTED
                    } else if is_match {
                        // 搜索匹配行用微弱高亮
                        Color::from_rgb(0.20, 0.28, 0.18)
                    } else if idx % 2 == 0 {
                        AppColors::BG_PRIMARY
                    } else {
                        AppColors::BG_SECONDARY
                    };

                    let value_color = if is_match && !is_selected {
                        AppColors::WARNING
                    } else {
                        AppColors::TEXT_PRIMARY
                    };

                    button(
                        row![
                            cell_text(offset_str, 80, AppColors::TEXT_MUTED),
                            cell_text(ts_str, 150, AppColors::TEXT_SECONDARY),
                            cell_text(key_str, 120, AppColors::ACCENT),
                            text(value_str).size(12).color(value_color),
                        ]
                        .spacing(8)
                        .padding([4, 12]),
                    )
                    .on_press(Message::SelectMessage(idx))
                    .style(move |_theme: &Theme, status| {
                        let bg = if matches!(status, button::Status::Hovered) && !is_selected {
                            AppColors::ROW_HOVER
                        } else {
                            bg_color
                        };
                        button::Style {
                            background: Some(Background::Color(bg)),
                            border: Border::default(),
                            text_color: AppColors::TEXT_PRIMARY,
                            ..Default::default()
                        }
                    })
                    .width(Length::Fill)
                    .into()
                })
                .collect::<Vec<Element<'_, Message>>>(),
        )
        .spacing(0);

        content = content.push(scrollable(rows).height(Length::Fill));

        // 搜索结果统计
        if has_search {
            let match_count = state
                .messages
                .iter()
                .filter(|msg| {
                    let key = msg.decoded_key.as_deref().unwrap_or("");
                    let value = msg.decoded_value.summary(200);
                    key.to_lowercase().contains(&search_lower)
                        || value.to_lowercase().contains(&search_lower)
                })
                .count();

            content = content.push(
                container(
                    text(format!("搜索 \"{}\" - 匹配 {} 条", search_query, match_count))
                        .size(11)
                        .color(AppColors::TEXT_SECONDARY),
                )
                .padding([4, 12])
                .width(Length::Fill)
                .style(|_theme: &Theme| container::Style {
                    background: Some(Background::Color(AppColors::BG_TERTIARY)),
                    ..Default::default()
                }),
            );
        }
    }

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn cell_text(value: String, width: u16, color: Color) -> Element<'static, Message> {
    container(text(value).size(12).color(color))
        .width(width)
        .into()
}
