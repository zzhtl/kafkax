use crate::kafka::types::TopicMeta;

/// 侧边栏树状态
#[derive(Debug, Clone, Default)]
pub struct SidebarState {
    /// 所有 topic 元数据
    pub topics: Vec<TopicMeta>,
    /// 展开的 topic 名称集合
    pub expanded: std::collections::HashSet<String>,
    /// 当前选中的 topic
    pub selected_topic: Option<String>,
    /// 当前选中的 partition
    pub selected_partition: Option<i32>,
}

impl SidebarState {
    /// 切换 topic 展开/折叠
    pub fn toggle_topic(&mut self, topic: &str) {
        if self.expanded.contains(topic) {
            self.expanded.remove(topic);
        } else {
            self.expanded.insert(topic.to_string());
        }
    }

    /// 选中一个 partition
    pub fn select_partition(&mut self, topic: &str, partition: i32) {
        self.selected_topic = Some(topic.to_string());
        self.selected_partition = Some(partition);
        // 确保该 topic 是展开的
        self.expanded.insert(topic.to_string());
    }

    /// 判断某个 partition 是否被选中
    pub fn is_partition_selected(&self, topic: &str, partition: i32) -> bool {
        self.selected_topic.as_deref() == Some(topic) && self.selected_partition == Some(partition)
    }
}
