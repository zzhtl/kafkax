use crate::kafka::types::{DecodedMessage, PageData};

/// 表格状态（分页、搜索）
#[derive(Debug, Clone)]
pub struct TableState {
    /// 当前页消息
    pub messages: Vec<DecodedMessage>,
    /// 当前页码（从 0 开始）
    pub current_page: usize,
    /// 每页消息数
    pub page_size: usize,
    /// 总消息数
    pub total_messages: i64,
    /// Low watermark
    pub low_watermark: i64,
    /// High watermark
    pub high_watermark: i64,
    /// 当前选中的消息索引
    pub selected_index: Option<usize>,
    /// 搜索关键词
    pub search_query: String,
    /// 是否正在加载
    pub loading: bool,
    /// 加载耗时(ms)
    pub load_time_ms: Option<u128>,
}

impl Default for TableState {
    fn default() -> Self {
        Self {
            messages: vec![],
            current_page: 0,
            page_size: 100,
            total_messages: 0,
            low_watermark: 0,
            high_watermark: 0,
            selected_index: None,
            search_query: String::new(),
            loading: false,
            load_time_ms: None,
        }
    }
}

impl TableState {
    /// 总页数
    pub fn total_pages(&self) -> usize {
        if self.total_messages <= 0 {
            return 0;
        }
        (self.total_messages as usize).div_ceil(self.page_size)
    }

    /// 当前页的起始 offset
    pub fn current_offset(&self) -> i64 {
        self.low_watermark + (self.current_page as i64 * self.page_size as i64)
    }

    /// 是否可以翻到上一页
    pub fn has_prev(&self) -> bool {
        self.current_page > 0
    }

    /// 是否可以翻到下一页
    pub fn has_next(&self) -> bool {
        self.current_page + 1 < self.total_pages()
    }

    /// 应用页数据
    pub fn apply_page_data(&mut self, data: PageData, load_time_ms: u128) {
        self.messages = data.messages;
        self.current_page = data.page;
        self.total_messages = data.total_messages;
        self.low_watermark = data.low_watermark;
        self.high_watermark = data.high_watermark;
        self.loading = false;
        self.load_time_ms = Some(load_time_ms);
        self.selected_index = None;
    }

    /// 获取当前选中的消息
    pub fn selected_message(&self) -> Option<&DecodedMessage> {
        self.selected_index
            .and_then(|idx| self.messages.get(idx))
    }
}
