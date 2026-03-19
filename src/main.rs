#![allow(dead_code)]

mod app;

use app::App;
use kafkax::theme;

fn main() -> iced::Result {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("kafkax=info".parse().unwrap()),
        )
        .init();

    tracing::info!("KafkaX 启动中...");

    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .default_font(theme::ui_font())
        .antialiasing(true)
        .window_size((1200.0, 800.0))
        .centered()
        .run()
}
