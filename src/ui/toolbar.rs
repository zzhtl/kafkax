use iced::widget::{button, container, row, text, text_input, Space};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::codec::DecoderType;
use crate::message::Message;
use crate::state::TableState;
use crate::theme::AppColors;

/// 渲染工具栏
pub fn view<'a>(table: &'a TableState, selected_decoder: DecoderType) -> Element<'a, Message> {
    let search_input = text_input("搜索...", &table.search_query)
        .on_input(Message::SearchInputChanged)
        .on_submit(Message::Search)
        .padding(6)
        .size(13)
        .width(200);

    // 解码器选择按钮组
    let decoder_row = DecoderType::ALL.iter().fold(
        row![].spacing(2),
        |r, &dt| {
            let is_active = dt == selected_decoder;
            let btn = button(text(dt.label()).size(11).color(if is_active {
                Color::WHITE
            } else {
                AppColors::TEXT_SECONDARY
            }))
            .on_press(Message::DecoderChanged(dt))
            .style(move |_theme: &Theme, _status| button::Style {
                background: Some(Background::Color(if is_active {
                    AppColors::ACCENT
                } else {
                    AppColors::BG_TERTIARY
                })),
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                text_color: if is_active {
                    Color::WHITE
                } else {
                    AppColors::TEXT_SECONDARY
                },
                ..Default::default()
            })
            .padding([3, 8]);
            r.push(btn)
        },
    );

    // 分页控件
    let page_controls = {
        let total_pages = table.total_pages();
        let current = table.current_page + 1;

        let first_btn = page_button("◄◄", Message::FirstPage, table.has_prev());
        let prev_btn = page_button("◄", Message::PrevPage, table.has_prev());
        let next_btn = page_button("►", Message::NextPage, table.has_next());
        let last_btn = page_button("►►", Message::LastPage, table.has_next());

        let page_info = text(format!("{} / {}", current, total_pages.max(1)))
            .size(12)
            .color(AppColors::TEXT_PRIMARY);

        row![first_btn, prev_btn, page_info, next_btn, last_btn].spacing(4)
    };

    // 每页大小选择
    let page_sizes = [50, 100, 200, 500];
    let page_size_row = page_sizes.iter().fold(row![].spacing(2), |r, &size| {
        let is_active = size == table.page_size;
        let btn = button(text(format!("{}", size)).size(11).color(if is_active {
            Color::WHITE
        } else {
            AppColors::TEXT_SECONDARY
        }))
        .on_press(Message::PageSizeChanged(size))
        .style(move |_theme: &Theme, _status| button::Style {
            background: Some(Background::Color(if is_active {
                AppColors::ACCENT
            } else {
                AppColors::BG_TERTIARY
            })),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            text_color: if is_active {
                Color::WHITE
            } else {
                AppColors::TEXT_SECONDARY
            },
            ..Default::default()
        })
        .padding([3, 6]);
        r.push(btn)
    });

    let toolbar = row![
        search_input,
        Space::with_width(Length::Fixed(12.0)),
        decoder_row,
        Space::with_width(Length::Fill),
        page_controls,
        Space::with_width(Length::Fixed(12.0)),
        text("每页:").size(12).color(AppColors::TEXT_SECONDARY),
        page_size_row,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .padding([6, 12]);

    container(toolbar)
        .width(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(AppColors::BG_TERTIARY)),
            border: Border {
                color: AppColors::BORDER,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn page_button(label: &str, msg: Message, enabled: bool) -> iced::widget::Button<'_, Message> {
    let btn = button(
        text(label)
            .size(12)
            .color(if enabled {
                AppColors::TEXT_PRIMARY
            } else {
                AppColors::TEXT_MUTED
            }),
    )
    .padding([3, 6])
    .style(move |_theme: &Theme, _status| button::Style {
        background: Some(Background::Color(AppColors::BG_SECONDARY)),
        border: Border {
            radius: 3.0.into(),
            ..Default::default()
        },
        text_color: if enabled {
            AppColors::TEXT_PRIMARY
        } else {
            AppColors::TEXT_MUTED
        },
        ..Default::default()
    });

    if enabled {
        btn.on_press(msg)
    } else {
        btn
    }
}
