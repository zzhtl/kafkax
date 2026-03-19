use iced::{Color, Theme};

/// 应用主题色常量
pub struct AppColors;

impl AppColors {
    /// 背景色
    pub const BG_PRIMARY: Color = Color::from_rgb(0.11, 0.12, 0.14);
    pub const BG_SECONDARY: Color = Color::from_rgb(0.15, 0.16, 0.18);
    pub const BG_TERTIARY: Color = Color::from_rgb(0.18, 0.19, 0.22);

    /// 前景色
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.9, 0.91, 0.92);
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.6, 0.62, 0.65);
    pub const TEXT_MUTED: Color = Color::from_rgb(0.4, 0.42, 0.45);

    /// 强调色
    pub const ACCENT: Color = Color::from_rgb(0.24, 0.56, 0.95);
    pub const ACCENT_HOVER: Color = Color::from_rgb(0.30, 0.62, 1.0);

    /// 状态色
    pub const SUCCESS: Color = Color::from_rgb(0.30, 0.78, 0.47);
    pub const WARNING: Color = Color::from_rgb(0.95, 0.75, 0.25);
    pub const ERROR: Color = Color::from_rgb(0.91, 0.30, 0.24);

    /// 边框色
    pub const BORDER: Color = Color::from_rgb(0.25, 0.27, 0.30);
    pub const BORDER_FOCUS: Color = Color::from_rgb(0.24, 0.56, 0.95);

    /// 选中行
    pub const ROW_SELECTED: Color = Color::from_rgb(0.20, 0.35, 0.55);
    pub const ROW_HOVER: Color = Color::from_rgb(0.16, 0.17, 0.20);

    /// JSON 语法高亮色
    pub const JSON_KEY: Color = Color::from_rgb(0.60, 0.80, 1.0);
    pub const JSON_STRING: Color = Color::from_rgb(0.80, 0.90, 0.50);
    pub const JSON_NUMBER: Color = Color::from_rgb(0.75, 0.60, 0.95);
    pub const JSON_BOOL: Color = Color::from_rgb(0.95, 0.60, 0.40);
    pub const JSON_NULL: Color = Color::from_rgb(0.60, 0.60, 0.60);
}

/// 返回应用暗色主题
pub fn app_theme() -> Theme {
    Theme::Dark
}
