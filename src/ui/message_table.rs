use iced::widget::{button, column, container, rich_text, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::kafka::types::{MessageSummary, SortOrder};
use crate::message::Message;
use crate::state::TableState;
use crate::theme::AppColors;
use crate::ui::syntax;

/// 渲染消息列表表格
pub fn view(state: &TableState) -> Element<'_, Message> {
    let mut content = column![table_header(state)].spacing(8);

    if let Some(error) = &state.error_message {
        content = content.push(feedback_banner(
            error,
            AppColors::ERROR,
            AppColors::ERROR_BG,
        ));
    }

    if state.loading {
        let (title, subtitle) = if state.search_in_progress {
            (
                "正在搜索消息...",
                "正在扫描当前 Topic 全部分区中仍保留的数据，已消费过但尚未删除的消息也会被检索。",
            )
        } else {
            ("正在加载消息...", "请稍候，当前分区的数据正在拉取和解码。")
        };
        content = content.push(empty_state(title, subtitle));
    } else if state.messages.is_empty() {
        let (title, subtitle) = if state.error_message.is_some() {
            (
                if state.has_search_results() {
                    "消息搜索失败"
                } else {
                    "消息加载失败"
                },
                if state.has_search_results() {
                    "请检查连接状态、权限配置，或确认当前 Topic 的全部 Partition 是否可读。"
                } else {
                    "请检查连接状态、权限配置或当前 Partition 是否可读。"
                },
            )
        } else if state.has_search_results() {
            (
                "没有匹配消息",
                "当前 Topic 全部分区中仍保留的数据里，没有包含该关键字的消息。",
            )
        } else {
            (
                "暂无消息",
                "从左侧选择一个 Partition，或确认当前分区是否已有数据。",
            )
        };
        content = content.push(empty_state(title, subtitle));
    } else {
        let search_query = state.search_query.trim();
        let rows = column(
            state
                .messages
                .iter()
                .enumerate()
                .map(|(idx, msg)| {
                    let is_selected = state.selected_index == Some(idx);
                    let partition_str = msg.partition_label.clone();
                    let offset_str = msg.offset_label.clone();
                    let ts_str = msg.ts_label.clone();
                    let key_str = msg.key_label.clone();
                    let value_str = msg.value_label.clone();

                    let bg_color = if is_selected {
                        AppColors::ROW_SELECTED
                    } else if idx % 2 == 0 {
                        AppColors::BG_PRIMARY
                    } else {
                        AppColors::BG_SECONDARY
                    };

                    button(
                        row![
                            cell_rich_text(
                                syntax::highlight_search(
                                    &partition_str,
                                    search_query,
                                    AppColors::TEXT_MUTED,
                                ),
                                72,
                            ),
                            cell_rich_text(
                                syntax::highlight_search(
                                    &offset_str,
                                    search_query,
                                    AppColors::TEXT_MUTED,
                                ),
                                80,
                            ),
                            cell_rich_text(
                                syntax::highlight_search(
                                    &ts_str,
                                    search_query,
                                    AppColors::TEXT_SECONDARY,
                                ),
                                150,
                            ),
                            cell_rich_text(
                                syntax::highlight_search(
                                    &key_str,
                                    search_query,
                                    AppColors::ACCENT,
                                ),
                                120,
                            ),
                            rich_text(syntax::highlight_search(
                                &value_str,
                                search_query,
                                AppColors::TEXT_PRIMARY,
                            ))
                            .size(12)
                            .width(Length::Fill),
                        ]
                        .spacing(8)
                        .padding([6, 12]),
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

        if state.has_search_results() {
            content = content.push(feedback_banner(
                format!(
                    "已扫描 {} 条，命中 {} 条。结果覆盖当前 Topic 的全部分区；翻页仅在搜索命中结果内切换。",
                    state.search_scanned_messages.unwrap_or(0),
                    state.total_messages
                ),
                AppColors::TEXT_SECONDARY,
                AppColors::BG_TERTIARY,
            ));
        } else if !search_query.is_empty() {
            content = content.push(feedback_banner(
                format!(
                    "已输入“{}”。按回车执行当前 Topic 全部分区搜索；未执行前仅对当前页做关键词高亮。",
                    search_query
                ),
                AppColors::TEXT_SECONDARY,
                AppColors::BG_TERTIARY,
            ));
        }
    }

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(12)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(AppColors::BG_SECONDARY)),
            border: Border {
                color: AppColors::BORDER,
                width: 1.0,
                radius: 12.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn table_header(state: &TableState) -> Element<'_, Message> {
    container(
        row![
            cell_text("分区", 72, AppColors::TEXT_SECONDARY),
            cell_text("Offset", 80, AppColors::TEXT_SECONDARY),
            time_sort_header(state.sort_order),
            cell_text("Key", 120, AppColors::TEXT_SECONDARY),
            text("消息摘要").size(12).color(AppColors::TEXT_SECONDARY),
        ]
        .spacing(8)
        .padding([8, 12]),
    )
    .width(Length::Fill)
    .style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(AppColors::BG_TERTIARY)),
        border: Border {
            color: AppColors::BORDER,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn time_sort_header(sort_order: SortOrder) -> Element<'static, Message> {
    let next_order = sort_order.toggle();

    button(
        row![
            text("时间").size(12).color(AppColors::TEXT_SECONDARY),
            text(sort_order.indicator())
                .size(11)
                .color(AppColors::ACCENT),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .width(150),
    )
    .on_press(Message::SortOrderChanged(next_order))
    .style(|_theme: &Theme, status| button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered) {
                AppColors::ROW_HOVER
            } else {
                Color::TRANSPARENT
            },
        )),
        border: Border::default(),
        text_color: AppColors::TEXT_SECONDARY,
        ..Default::default()
    })
    .padding(0)
    .into()
}

fn empty_state<'a>(title: &'a str, subtitle: &'a str) -> Element<'a, Message> {
    container(
        column![
            text(title).size(15).color(AppColors::TEXT_PRIMARY),
            text(subtitle).size(12).color(AppColors::TEXT_MUTED),
        ]
        .spacing(6),
    )
    .padding(28)
    .width(Length::Fill)
    .style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(AppColors::BG_PRIMARY)),
        border: Border {
            color: AppColors::BORDER,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn feedback_banner(
    message: impl Into<String>,
    text_color: Color,
    background: Color,
) -> Element<'static, Message> {
    container(text(message.into()).size(11).color(text_color))
        .padding([8, 12])
        .width(Length::Fill)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(background)),
            border: Border {
                color: background,
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn value_summary(msg: &MessageSummary, _query: &str, _max_len: usize) -> String {
    msg.value_label.clone()
}

fn cell_text(value: &'static str, width: u16, color: Color) -> Element<'static, Message> {
    container(text(value).size(12).color(color))
        .width(u32::from(width))
        .into()
}

fn cell_rich_text(
    spans: Vec<iced::widget::text::Span<'static>>,
    width: u16,
) -> Element<'static, Message> {
    container(rich_text(spans).size(12))
        .width(u32::from(width))
        .into()
}
