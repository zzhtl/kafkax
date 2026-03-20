use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::config::{ConnectionConfig, SaslMechanism, SecurityProtocol};
use crate::message::Message;
use crate::state::{AppNotice, ConnectionStatus, NoticeTone};
use crate::theme::AppColors;

/// 渲染连接对话框
pub fn view<'a>(
    draft: &'a ConnectionConfig,
    connection_status: &'a ConnectionStatus,
    saved_connections: &'a [ConnectionConfig],
    last_connection_name: Option<&'a str>,
    notice: Option<&'a AppNotice>,
) -> Element<'a, Message> {
    let status_banner = status_banner(connection_status);
    let can_connect = can_connect(draft, connection_status);
    let uses_sasl = draft.security_protocol.uses_sasl();
    let uses_ssl = draft.security_protocol.uses_ssl();

    let sasl_mechanism = draft
        .sasl
        .as_ref()
        .map(|sasl| sasl.mechanism)
        .unwrap_or(SaslMechanism::Plain);
    let sasl_username = draft
        .sasl
        .as_ref()
        .map(|sasl| sasl.username.as_str())
        .unwrap_or("");
    let sasl_password = draft
        .sasl
        .as_ref()
        .map(|sasl| sasl.password.as_str())
        .unwrap_or("");
    let ssl_ca = draft
        .ssl
        .as_ref()
        .and_then(|ssl| ssl.ca_location.as_deref())
        .unwrap_or("");
    let ssl_cert = draft
        .ssl
        .as_ref()
        .and_then(|ssl| ssl.cert_location.as_deref())
        .unwrap_or("");
    let ssl_key = draft
        .ssl
        .as_ref()
        .and_then(|ssl| ssl.key_location.as_deref())
        .unwrap_or("");
    let ssl_key_password = draft
        .ssl
        .as_ref()
        .and_then(|ssl| ssl.key_password.as_deref())
        .unwrap_or("");

    let protocol_row = SecurityProtocol::ALL
        .iter()
        .fold(row![].spacing(6), |row, protocol| {
            row.push(option_button(
                protocol.label(),
                *protocol == draft.security_protocol,
                Message::SecurityProtocolChanged(*protocol),
            ))
        });

    let mechanism_row = SaslMechanism::ALL
        .iter()
        .fold(row![].spacing(6), |row, mechanism| {
            row.push(option_button(
                mechanism.label(),
                *mechanism == sasl_mechanism,
                Message::SaslMechanismChanged(*mechanism),
            ))
        });

    let saved_section = if saved_connections.is_empty() {
        None
    } else {
        let items = saved_connections
            .iter()
            .fold(column![].spacing(10), |column, connection| {
                let is_last_used = last_connection_name == Some(connection.name.as_str());
                column.push(saved_connection_card(connection, is_last_used))
            });

        Some(section_card(
            "已保存连接",
            "连接成功后会自动保存在本地，下次可一键载入或直接连接。",
            items,
        ))
    };

    let basic_section = section_card(
        "基本信息",
        "连接名用于界面显示；当前浏览模式不会加入消费组，消费组 ID 仅保留作配置备注。",
        column![
            row![
                form_field(
                    "连接名称",
                    "可选，默认使用 Broker 地址",
                    text_input("生产集群 / 测试环境", &draft.name)
                        .on_input(Message::ConnectionNameInputChanged)
                        .padding([10, 12])
                        .size(14),
                ),
                form_field(
                    "Broker 地址",
                    "必填，多个地址用逗号分隔",
                    text_input("10.0.0.1:9092,10.0.0.2:9092", &draft.brokers)
                        .on_input(Message::BrokerInputChanged)
                        .on_submit(Message::Connect)
                        .padding([10, 12])
                        .size(14),
                ),
            ]
            .spacing(12),
            form_field(
                "消费组 ID",
                "可选，仅作配置保存，不参与当前浏览拉取",
                text_input(
                    "kafkax-readonly-group",
                    draft.group_id.as_deref().unwrap_or(""),
                )
                .on_input(Message::GroupIdInputChanged)
                .on_submit(Message::Connect)
                .padding([10, 12])
                .size(14),
            ),
        ]
        .spacing(12),
    );

    let mut security_content = column![field_with_element(
        "安全协议",
        "根据集群配置选择协议，下面会展示对应认证字段。",
        protocol_row.into(),
    )]
    .spacing(12);

    if uses_sasl {
        security_content = security_content
            .push(field_with_element(
                "SASL 机制",
                "常见为 PLAIN、SCRAM-SHA-256、SCRAM-SHA-512。",
                mechanism_row.into(),
            ))
            .push(
                row![
                    form_field(
                        "SASL 用户名",
                        "SASL 协议下必填",
                        text_input("kafka-user", sasl_username)
                            .on_input(Message::SaslUsernameChanged)
                            .on_submit(Message::Connect)
                            .padding([10, 12])
                            .size(14),
                    ),
                    form_field(
                        "SASL 密码",
                        "SASL 协议下必填",
                        text_input("请输入密码", sasl_password)
                            .on_input(Message::SaslPasswordChanged)
                            .on_submit(Message::Connect)
                            .secure(true)
                            .padding([10, 12])
                            .size(14),
                    ),
                ]
                .spacing(12),
            );
    }

    if uses_ssl {
        security_content = security_content
            .push(form_field(
                "CA 证书路径",
                "服务端启用 TLS 时通常需要填写。",
                text_input("/etc/kafka/ca.crt", ssl_ca)
                    .on_input(Message::SslCaLocationChanged)
                    .on_submit(Message::Connect)
                    .padding([10, 12])
                    .size(14),
            ))
            .push(
                row![
                    form_field(
                        "客户端证书路径",
                        "如果集群开启双向 TLS，请填写。",
                        text_input("/etc/kafka/client.crt", ssl_cert)
                            .on_input(Message::SslCertLocationChanged)
                            .on_submit(Message::Connect)
                            .padding([10, 12])
                            .size(14),
                    ),
                    form_field(
                        "客户端私钥路径",
                        "与客户端证书成对出现。",
                        text_input("/etc/kafka/client.key", ssl_key)
                            .on_input(Message::SslKeyLocationChanged)
                            .on_submit(Message::Connect)
                            .padding([10, 12])
                            .size(14),
                    ),
                ]
                .spacing(12),
            )
            .push(form_field(
                "私钥口令",
                "如果私钥带口令，请在这里填写。",
                text_input("请输入私钥口令", ssl_key_password)
                    .on_input(Message::SslKeyPasswordChanged)
                    .on_submit(Message::Connect)
                    .secure(true)
                    .padding([10, 12])
                    .size(14),
            ));
    }

    let security_section = section_card(
        "安全认证",
        "只展示当前协议相关的字段，未使用的配置不会参与连接。",
        security_content,
    );

    let connect_btn = button(text("连接").size(14).color(Color::WHITE))
        .on_press_maybe(can_connect.then_some(Message::Connect))
        .style(|_theme: &Theme, status| {
            let bg = match status {
                button::Status::Disabled => AppColors::BG_TERTIARY,
                button::Status::Hovered => AppColors::ACCENT_HOVER,
                _ => AppColors::ACCENT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: bg,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                text_color: Color::WHITE,
                ..Default::default()
            }
        })
        .padding([10, 24]);

    let close_btn = button(text("取消").size(13).color(AppColors::TEXT_SECONDARY))
        .on_press(Message::CloseConnectionDialog)
        .style(|theme: &Theme, status| {
            let mut style = button::text(theme, status);
            style.background = Some(Background::Color(Color::TRANSPARENT));
            style
        })
        .padding([10, 16]);

    let mut form_content = column![].spacing(12);

    if let Some(saved_section) = saved_section {
        form_content = form_content.push(saved_section);
    }

    form_content = form_content.push(basic_section).push(security_section);

    let dialog_content = column![
        text("连接 Kafka 集群")
            .size(20)
            .color(AppColors::TEXT_PRIMARY),
        text("把连接参数一次填完整，减少重复录入；已连接成功的配置会自动保存在本地。")
            .size(12)
            .color(AppColors::TEXT_MUTED),
        Space::new().height(16),
        scrollable(form_content).height(Length::Fixed(460.0)),
        Space::new().height(12),
        status_banner,
        if let Some(notice) = notice {
            feedback_banner(notice)
        } else {
            Space::new().height(0).into()
        },
        Space::new().height(16),
        row![connect_btn, Space::new().width(8), close_btn].align_y(iced::Alignment::Center),
    ]
    .spacing(0)
    .padding(28)
    .max_width(820);

    let dialog_box = container(dialog_content).style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(AppColors::BG_SECONDARY)),
        border: Border {
            color: AppColors::BORDER,
            width: 1.0,
            radius: 16.0.into(),
        },
        ..Default::default()
    });

    container(dialog_box)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.55))),
            ..Default::default()
        })
        .into()
}

fn can_connect(draft: &ConnectionConfig, status: &ConnectionStatus) -> bool {
    if matches!(status, ConnectionStatus::Connecting) || draft.brokers.trim().is_empty() {
        return false;
    }

    if draft.security_protocol.uses_sasl() {
        let Some(sasl) = draft.sasl.as_ref() else {
            return false;
        };

        !sasl.username.trim().is_empty() && !sasl.password.is_empty()
    } else {
        true
    }
}

fn section_card<'a>(
    title: &'a str,
    description: &'a str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(
        column![
            text(title).size(15).color(AppColors::TEXT_PRIMARY),
            text(description).size(12).color(AppColors::TEXT_MUTED),
            Space::new().height(12),
            content.into(),
        ]
        .spacing(0),
    )
    .padding(16)
    .style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(AppColors::BG_PRIMARY)),
        border: Border {
            color: AppColors::BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn saved_connection_card<'a>(
    connection: &'a ConnectionConfig,
    is_last_used: bool,
) -> Element<'a, Message> {
    let name = connection.name.clone();
    let load_name = name.clone();
    let connect_name = name.clone();
    let delete_name = name.clone();

    let mut badges = row![protocol_badge(connection.security_summary())]
        .spacing(8)
        .align_y(iced::Alignment::Center);

    if is_last_used {
        badges = badges.push(accent_badge("最近使用"));
    }

    if let Some(group_id) = connection.group_id.as_deref() {
        badges = badges.push(subtle_badge(format!("Group {}", group_id)));
    }

    let actions = row![
        text_action_button("载入", Message::LoadSavedConnection(load_name), false),
        text_action_button("连接", Message::ConnectSavedConnection(connect_name), true),
        text_action_button("删除", Message::DeleteSavedConnection(delete_name), false),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    container(
        column![
            row![
                column![
                    text(&connection.name)
                        .size(13)
                        .color(AppColors::TEXT_PRIMARY),
                    text(&connection.brokers)
                        .size(12)
                        .color(AppColors::TEXT_SECONDARY),
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                actions,
            ]
            .align_y(iced::Alignment::Center),
            Space::new().height(8),
            badges,
        ]
        .spacing(0),
    )
    .padding(14)
    .style(move |_theme: &Theme| container::Style {
        background: Some(Background::Color(AppColors::BG_SECONDARY)),
        border: Border {
            color: if is_last_used {
                AppColors::ACCENT
            } else {
                AppColors::BORDER
            },
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn form_field<'a>(
    title: &'a str,
    hint: &'a str,
    input: iced::widget::TextInput<'a, Message>,
) -> Element<'a, Message> {
    field_with_element(title, hint, input.into())
}

fn field_with_element<'a>(
    title: &'a str,
    hint: &'a str,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    container(
        column![
            text(title).size(13).color(AppColors::TEXT_PRIMARY),
            text(hint).size(11).color(AppColors::TEXT_MUTED),
            Space::new().height(6),
            content,
        ]
        .spacing(0),
    )
    .width(Length::Fill)
    .into()
}

fn option_button<'a>(label: &'a str, is_active: bool, message: Message) -> Element<'a, Message> {
    button(text(label).size(11).color(if is_active {
        Color::WHITE
    } else {
        AppColors::TEXT_SECONDARY
    }))
    .on_press(message)
    .style(move |_theme: &Theme, status| button::Style {
        background: Some(Background::Color(if is_active {
            AppColors::ACCENT
        } else if matches!(status, button::Status::Hovered) {
            AppColors::ROW_HOVER
        } else {
            AppColors::BG_SECONDARY
        })),
        border: Border {
            color: if is_active {
                AppColors::ACCENT
            } else {
                AppColors::BORDER
            },
            width: 1.0,
            radius: 999.0.into(),
        },
        text_color: if is_active {
            Color::WHITE
        } else {
            AppColors::TEXT_SECONDARY
        },
        ..Default::default()
    })
    .padding([6, 12])
    .into()
}

fn text_action_button<'a>(
    label: &'a str,
    message: Message,
    emphasized: bool,
) -> Element<'a, Message> {
    button(text(label).size(11).color(if emphasized {
        Color::WHITE
    } else {
        AppColors::TEXT_PRIMARY
    }))
    .on_press(message)
    .style(move |_theme: &Theme, status| {
        let background = if emphasized {
            match status {
                button::Status::Hovered => AppColors::ACCENT_HOVER,
                _ => AppColors::ACCENT,
            }
        } else if matches!(status, button::Status::Hovered) {
            AppColors::ROW_HOVER
        } else {
            AppColors::BG_PRIMARY
        };

        let border_color = if emphasized {
            background
        } else {
            AppColors::BORDER
        };

        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 999.0.into(),
            },
            text_color: if emphasized {
                Color::WHITE
            } else {
                AppColors::TEXT_PRIMARY
            },
            ..Default::default()
        }
    })
    .padding([6, 12])
    .into()
}

fn protocol_badge<'a>(label: String) -> Element<'a, Message> {
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

fn subtle_badge<'a>(label: String) -> Element<'a, Message> {
    container(text(label).size(11).color(AppColors::TEXT_MUTED))
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

fn accent_badge<'a>(label: &'a str) -> Element<'a, Message> {
    container(text(label).size(11).color(AppColors::ACCENT))
        .padding([4, 10])
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(AppColors::ACCENT_SOFT)),
            border: Border {
                color: AppColors::ACCENT,
                width: 1.0,
                radius: 999.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn status_banner<'a>(status: &'a ConnectionStatus) -> Element<'a, Message> {
    let (message, text_color, background) = match status {
        ConnectionStatus::Disconnected => (
            "请选择安全协议并填写对应认证字段，然后发起连接。",
            AppColors::TEXT_SECONDARY,
            AppColors::BG_TERTIARY,
        ),
        ConnectionStatus::Connecting => (
            "正在建立连接并拉取元数据，请稍候。",
            AppColors::WARNING,
            AppColors::WARNING_BG,
        ),
        ConnectionStatus::Connected(name) => {
            (name.as_str(), AppColors::SUCCESS, AppColors::SUCCESS_BG)
        }
        ConnectionStatus::Error(error) => (error.as_str(), AppColors::ERROR, AppColors::ERROR_BG),
    };

    let prefix = match status {
        ConnectionStatus::Connected(_) => "已连接",
        ConnectionStatus::Connecting => "连接中",
        ConnectionStatus::Error(_) => "连接失败",
        ConnectionStatus::Disconnected => "连接提示",
    };

    container(
        row![
            text(prefix).size(11).color(text_color),
            Space::new().width(8),
            text(message).size(11).color(text_color),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 12])
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

fn feedback_banner<'a>(notice: &'a AppNotice) -> Element<'a, Message> {
    let (text_color, background) = match notice.tone {
        NoticeTone::Info => (AppColors::ACCENT, AppColors::ACCENT_SOFT),
        NoticeTone::Success => (AppColors::SUCCESS, AppColors::SUCCESS_BG),
        NoticeTone::Error => (AppColors::ERROR, AppColors::ERROR_BG),
    };

    container(text(&notice.message).size(11).color(text_color))
        .padding([10, 12])
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
