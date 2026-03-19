#![allow(dead_code)]

mod app;

use app::App;

fn main() -> iced::Result {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("kafkax=info".parse().unwrap()),
        )
        .init();

    tracing::info!("KafkaX 启动中...");

    iced::application("KafkaX", App::update, App::view)
        .theme(App::theme)
        .subscription(App::subscription)
        .window_size((1200.0, 800.0))
        .centered()
        .run_with(App::new)
}
