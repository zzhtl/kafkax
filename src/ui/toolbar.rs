use iced::widget::{Space, button, checkbox, container, row, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::codec::DecoderType;
use crate::message::Message;
use crate::state::TableState;
use crate::theme::AppColors;

/// 渲染工具栏
pub fn view<'a>(
    table: &'a TableState,
    selected_decoder: DecoderType,
    selected_partition: Option<i32>,
) -> Element<'a, Message> {
    let placeholder = if table.has_search_results() || table.search_in_progress {
        "正在显示搜索结果".to_string()
    } else if !table.search_all_partitions {
        match selected_partition {
            Some(p) => format!("输入关键词后回车搜索 Partition {p}"),
            None => "输入关键词后回车搜索全部分区".to_string(),
        }
    } else {
        "输入关键词后回车搜索全部分区".to_string()
    };

    let search_input = text_input(&placeholder, &table.search_query)
        .on_input(Message::SearchInputChanged)
        .on_submit(Message::Search)
        .padding([8, 10])
        .size(14)
        .width(320);

    let all_partitions_toggle = checkbox(table.search_all_partitions)
        .label("全分区")
        .on_toggle(Message::SetSearchAllPartitions)
        .size(14)
        .text_size(13);

    // 解码器选择按钮组
    let decoder_row = DecoderType::ALL.iter().fold(row![].spacing(4), |r, &dt| {
        let is_active = dt == selected_decoder;
        let btn = button(text(dt.label()).size(11).color(if is_active {
            Color::WHITE
        } else {
            AppColors::TEXT_SECONDARY
        }))
        .on_press(Message::DecoderChanged(dt))
        .style(move |_theme: &Theme, status| button::Style {
            background: Some(Background::Color(if is_active {
                AppColors::ACCENT
            } else if matches!(status, button::Status::Hovered) {
                AppColors::ROW_HOVER
            } else {
                AppColors::BG_PRIMARY
            })),
            border: Border {
                color: if is_active {
                    AppColors::ACCENT
                } else {
                    AppColors::BORDER
                },
                width: 1.0,
                radius: 8.0.into(),
            },
            text_color: if is_active {
                Color::WHITE
            } else {
                AppColors::TEXT_SECONDARY
            },
            ..Default::default()
        })
        .padding([5, 10]);
        r.push(btn)
    });

    let decoder_section = row![
        text("解码").size(12).color(AppColors::TEXT_MUTED),
        decoder_row,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    // 分页控件
    let page_controls = {
        let total_pages = table.total_pages();
        let current = table.current_page + 1;

        let first_btn = page_button("<<", Message::FirstPage, table.has_prev());
        let prev_btn = page_button("<", Message::PrevPage, table.has_prev());
        let next_btn = page_button(">", Message::NextPage, table.has_next());
        let last_btn = page_button(">>", Message::LastPage, table.has_next());

        let page_info = text(format!("第 {} / {} 页", current, total_pages.max(1)))
            .size(12)
            .color(AppColors::TEXT_PRIMARY);

        row![first_btn, prev_btn, page_info, next_btn, last_btn]
            .spacing(4)
            .align_y(iced::Alignment::Center)
    };

    // 每页大小选择
    let page_sizes = [50, 100, 200, 500];
    let page_size_row = page_sizes.iter().fold(row![].spacing(4), |r, &size| {
        let is_active = size == table.page_size;
        let btn = button(text(format!("{}", size)).size(11).color(if is_active {
            Color::WHITE
        } else {
            AppColors::TEXT_SECONDARY
        }))
        .on_press(Message::PageSizeChanged(size))
        .style(move |_theme: &Theme, status| button::Style {
            background: Some(Background::Color(if is_active {
                AppColors::ACCENT
            } else if matches!(status, button::Status::Hovered) {
                AppColors::ROW_HOVER
            } else {
                AppColors::BG_PRIMARY
            })),
            border: Border {
                color: if is_active {
                    AppColors::ACCENT
                } else {
                    AppColors::BORDER
                },
                width: 1.0,
                radius: 8.0.into(),
            },
            text_color: if is_active {
                Color::WHITE
            } else {
                AppColors::TEXT_SECONDARY
            },
            ..Default::default()
        })
        .padding([5, 10]);
        r.push(btn)
    });

    let page_size_section = row![
        text("每页").size(12).color(AppColors::TEXT_MUTED),
        page_size_row,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let toolbar = row![
        search_input,
        all_partitions_toggle,
        decoder_section,
        Space::new().width(Length::Fill),
        page_controls,
        page_size_section,
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center)
    .padding([10, 12]);

    container(toolbar)
        .width(Length::Fill)
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

fn page_button(label: &str, msg: Message, enabled: bool) -> iced::widget::Button<'_, Message> {
    let btn = button(text(label).size(12).color(if enabled {
        AppColors::TEXT_PRIMARY
    } else {
        AppColors::TEXT_MUTED
    }))
    .padding([3, 6])
    .style(move |_theme: &Theme, status| button::Style {
        background: Some(Background::Color(if !enabled {
            AppColors::BG_PRIMARY
        } else if matches!(status, button::Status::Hovered) {
            AppColors::ROW_HOVER
        } else {
            AppColors::BG_TERTIARY
        })),
        border: Border {
            color: if enabled {
                AppColors::BORDER
            } else {
                AppColors::BG_TERTIARY
            },
            width: 1.0,
            radius: 8.0.into(),
        },
        text_color: if enabled {
            AppColors::TEXT_PRIMARY
        } else {
            AppColors::TEXT_MUTED
        },
        ..Default::default()
    });

    if enabled { btn.on_press(msg) } else { btn }
}
