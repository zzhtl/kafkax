// src/state/overlay_state.rs

/// 右键菜单目标对象
#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    Topic {
        name: String,
        partitions: Vec<i32>,
    },
    Partition {
        topic: String,
        partition: i32,
    },
}

/// 全局弹窗/浮层状态机
#[derive(Debug, Clone, Default)]
pub enum OverlayState {
    #[default]
    None,

    ContextMenu {
        x: f32,
        y: f32,
        target: ContextMenuTarget,
    },

    SendMessage {
        topic: String,
        partition: i32,
        input: String,
        sending: bool,
        error: Option<String>,
    },

    TopicConfig {
        topic: String,
        partitions: Vec<i32>,
        retention_secs: String,
        retention_gb: String,
        retention_gb_note: Option<String>,
        loading: bool,
        saving: bool,
        error: Option<String>,
        purge_confirm_pending: bool,
        purging: bool,
    },
}

/// 验证 retention 输入是否合法（正整数或 -1）
pub fn is_valid_retention_input(s: &str) -> bool {
    let s = s.trim();
    if s == "-1" {
        return true;
    }
    s.parse::<u64>().is_ok()
}

/// 将 retention_secs 字符串换算为毫秒 i64
/// -1 → -1，其余按 secs * 1000
pub fn secs_to_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    if s == "-1" {
        return Some(-1);
    }
    s.parse::<i64>().ok().map(|v| v * 1000)
}

/// 将 retention_gb 字符串换算为 bytes i64
/// -1 → -1，其余按 checked_mul(1_073_741_824)
pub fn gb_to_bytes(s: &str) -> Option<i64> {
    let s = s.trim();
    if s == "-1" {
        return Some(-1);
    }
    s.parse::<i64>()
        .ok()
        .and_then(|v| v.checked_mul(1_073_741_824_i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_retention_inputs() {
        assert!(is_valid_retention_input("-1"));
        assert!(is_valid_retention_input("0"));
        assert!(is_valid_retention_input("604800"));
        assert!(!is_valid_retention_input("abc"));
        assert!(!is_valid_retention_input("1.5"));
        assert!(!is_valid_retention_input("-2"));
        assert!(!is_valid_retention_input(""));
    }

    #[test]
    fn secs_to_ms_converts_correctly() {
        assert_eq!(secs_to_ms("-1"), Some(-1));
        assert_eq!(secs_to_ms("604800"), Some(604_800_000));
        assert_eq!(secs_to_ms("abc"), None);
    }

    #[test]
    fn gb_to_bytes_converts_correctly() {
        assert_eq!(gb_to_bytes("-1"), Some(-1));
        assert_eq!(gb_to_bytes("1"), Some(1_073_741_824));
        assert_eq!(gb_to_bytes("10"), Some(10_737_418_240));
        // 溢出应返回 None
        assert_eq!(gb_to_bytes("9999999999999"), None);
    }
}
