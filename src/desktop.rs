//! 运行时自动生成桌面入口和安装图标（支持 Linux 和 macOS）

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const APP_NAME: &str = "kafkax";
const APP_DISPLAY_NAME: &str = "KafkaX";
const APP_COMMENT: &str = "高性能 Kafka 桌面客户端";
const APP_WM_CLASS: &str = "kafkax";
const APP_BUNDLE_ID: &str = "com.kafkax.KafkaX";
const ICON_SVG: &[u8] = include_bytes!("../assets/kafkax.svg");

/// 自动注册桌面入口和图标（Linux / macOS）。
/// 其他平台直接跳过。
pub fn register_desktop_entry() {
    let result = if cfg!(target_os = "linux") {
        register_linux()
    } else if cfg!(target_os = "macos") {
        register_macos()
    } else {
        return;
    };

    if let Err(e) = result {
        tracing::warn!("自动注册桌面入口失败: {e}");
    }
}

// ---------------------------------------------------------------------------
// Linux: .desktop 文件 + SVG 图标
// ---------------------------------------------------------------------------

fn register_linux() -> Result<(), Box<dyn std::error::Error>> {
    let home = home_dir()?;
    let exe_path = current_exe_path()?;

    // 安装 SVG 图标
    let icon_dir = home.join(".local/share/icons/hicolor/scalable/apps");
    fs::create_dir_all(&icon_dir)?;
    let icon_path = icon_dir.join(format!("{APP_NAME}.svg"));
    write_if_changed(&icon_path, ICON_SVG)?;

    // 生成 .desktop 文件
    let applications_dir = home.join(".local/share/applications");
    fs::create_dir_all(&applications_dir)?;
    let desktop_path = applications_dir.join(format!("{APP_NAME}.desktop"));

    let desktop_content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={APP_DISPLAY_NAME}\n\
         Comment={APP_COMMENT}\n\
         Exec={exe_path}\n\
         Icon={icon}\n\
         Terminal=false\n\
         StartupWMClass={APP_WM_CLASS}\n\
         Categories=Development;Utility;\n",
        icon = icon_path.display(),
    );

    if write_if_changed(&desktop_path, desktop_content.as_bytes())? {
        tracing::info!("已生成桌面入口: {}", desktop_path.display());
        let _ = std::process::Command::new("update-desktop-database")
            .arg(applications_dir.to_string_lossy().as_ref())
            .status();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// macOS: .app bundle
// ---------------------------------------------------------------------------

fn register_macos() -> Result<(), Box<dyn std::error::Error>> {
    let home = home_dir()?;
    let exe_path = current_exe_path()?;

    let app_dir = home.join(format!("Applications/{APP_DISPLAY_NAME}.app"));
    let contents_dir = app_dir.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");

    fs::create_dir_all(&macos_dir)?;
    fs::create_dir_all(&resources_dir)?;

    // Info.plist
    let info_plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>{APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>{APP_BUNDLE_ID}</string>
    <key>CFBundleName</key>
    <string>{APP_DISPLAY_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>{APP_DISPLAY_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleIconFile</key>
    <string>{APP_NAME}.svg</string>
    <key>CFBundleVersion</key>
    <string>1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>"#,
    );
    write_if_changed(&contents_dir.join("Info.plist"), info_plist.as_bytes())?;

    // 图标
    write_if_changed(&resources_dir.join(format!("{APP_NAME}.svg")), ICON_SVG)?;

    // 启动脚本：exec 真实二进制
    let launcher = format!(
        "#!/bin/bash\nexec \"{exe_path}\" \"$@\"\n",
    );
    let launcher_path = macos_dir.join(APP_NAME);
    if write_if_changed(&launcher_path, launcher.as_bytes())? {
        fs::set_permissions(&launcher_path, fs::Permissions::from_mode(0o755))?;
    }

    tracing::info!("已生成 macOS 应用包: {}", app_dir.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

/// 仅在内容变化时写入文件，返回是否实际写入。
fn write_if_changed(path: &PathBuf, content: &[u8]) -> Result<bool, Box<dyn std::error::Error>> {
    let needs_update = match fs::read(path) {
        Ok(existing) => existing != content,
        Err(_) => true,
    };
    if needs_update {
        fs::write(path, content)?;
        tracing::info!("已写入: {}", path.display());
    }
    Ok(needs_update)
}

fn current_exe_path() -> Result<String, Box<dyn std::error::Error>> {
    Ok(std::env::current_exe()?
        .canonicalize()?
        .to_string_lossy()
        .to_string())
}

fn home_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "无法获取用户主目录 (HOME 环境变量未设置)".into())
}
