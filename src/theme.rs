use iced::{Color, Font, Theme};

/// 应用主题色常量
pub struct AppColors;

impl AppColors {
    /// 背景色
    pub const BG_PRIMARY: Color = Color::from_rgb(0.08, 0.10, 0.12);
    pub const BG_SECONDARY: Color = Color::from_rgb(0.11, 0.13, 0.16);
    pub const BG_TERTIARY: Color = Color::from_rgb(0.15, 0.18, 0.22);

    /// 前景色
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.94, 0.96, 0.98);
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.70, 0.74, 0.79);
    pub const TEXT_MUTED: Color = Color::from_rgb(0.47, 0.52, 0.58);

    /// 强调色
    pub const ACCENT: Color = Color::from_rgb(0.23, 0.55, 0.92);
    pub const ACCENT_HOVER: Color = Color::from_rgb(0.31, 0.63, 0.98);
    pub const ACCENT_SOFT: Color = Color::from_rgba(0.23, 0.55, 0.92, 0.18);
    pub const SEARCH_HIGHLIGHT: Color = Color::from_rgb(0.22, 0.66, 0.60);
    pub const SEARCH_HIGHLIGHT_BG: Color = Color::from_rgba(0.22, 0.66, 0.60, 0.24);
    pub const SEARCH_HIGHLIGHT_TEXT: Color = Color::from_rgb(0.89, 0.98, 0.96);

    /// 状态色
    pub const SUCCESS: Color = Color::from_rgb(0.30, 0.78, 0.47);
    pub const SUCCESS_BG: Color = Color::from_rgba(0.30, 0.78, 0.47, 0.18);
    pub const WARNING: Color = Color::from_rgb(0.95, 0.75, 0.25);
    pub const WARNING_BG: Color = Color::from_rgba(0.95, 0.75, 0.25, 0.18);
    pub const ERROR: Color = Color::from_rgb(0.91, 0.39, 0.35);
    pub const ERROR_BG: Color = Color::from_rgba(0.91, 0.39, 0.35, 0.18);

    /// 边框色
    pub const BORDER: Color = Color::from_rgb(0.22, 0.26, 0.31);
    pub const BORDER_FOCUS: Color = Color::from_rgb(0.33, 0.63, 0.96);

    /// 选中行
    pub const ROW_SELECTED: Color = Color::from_rgb(0.18, 0.33, 0.52);
    pub const ROW_HOVER: Color = Color::from_rgb(0.14, 0.17, 0.21);

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

/// 返回应用默认字体
pub fn ui_font() -> Font {
    #[cfg(target_os = "windows")]
    {
        Font::with_name("Microsoft YaHei UI")
    }

    #[cfg(target_os = "macos")]
    {
        Font::with_name("PingFang SC")
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Font::with_name("Noto Sans CJK SC")
    }
}

/// 返回详情面板字体，优先兼顾中文和结构化文本可读性
pub fn detail_font() -> Font {
    #[cfg(target_os = "windows")]
    {
        Font::with_name("Microsoft YaHei UI")
    }

    #[cfg(target_os = "macos")]
    {
        Font::with_name("PingFang SC")
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Font::with_name("Noto Sans Mono CJK SC")
    }
}
