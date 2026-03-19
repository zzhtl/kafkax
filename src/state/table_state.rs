use iced::widget::text_editor;

use crate::kafka::types::{DecodedMessage, PageData};

/// 表格状态（分页、搜索）
#[derive(Debug, Clone)]
pub struct TableState {
    /// 当前页消息
    pub messages: Vec<DecodedMessage>,
    /// 全分区搜索的全部命中结果
    pub search_results: Option<Vec<DecodedMessage>>,
    /// 最近一次全分区搜索扫描的消息数量
    pub search_scanned_messages: Option<usize>,
    /// 进入搜索前的普通浏览页码
    pub browse_page_before_search: Option<usize>,
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
    /// 详情区只读文本内容
    pub detail_content: text_editor::Content,
    /// 搜索关键词
    pub search_query: String,
    /// 是否正在加载
    pub loading: bool,
    /// 当前是否在执行全分区搜索
    pub search_in_progress: bool,
    /// 加载耗时(ms)
    pub load_time_ms: Option<u128>,
    /// 最近一次加载错误
    pub error_message: Option<String>,
}

impl Default for TableState {
    fn default() -> Self {
        Self {
            messages: vec![],
            search_results: None,
            search_scanned_messages: None,
            browse_page_before_search: None,
            current_page: 0,
            page_size: 100,
            total_messages: 0,
            low_watermark: 0,
            high_watermark: 0,
            selected_index: None,
            detail_content: text_editor::Content::new(),
            search_query: String::new(),
            loading: false,
            search_in_progress: false,
            load_time_ms: None,
            error_message: None,
        }
    }
}

impl TableState {
    /// 更新搜索词，并清除旧选中避免展示被过滤掉的消息
    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query;
        self.selected_index = None;
        self.detail_content = text_editor::Content::new();
    }

    /// 更新详情区只读文本
    pub fn set_detail_text(&mut self, text: impl AsRef<str>) {
        self.detail_content = text_editor::Content::with_text(text.as_ref());
    }

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

    /// 进入加载态并清理旧反馈
    pub fn begin_loading(&mut self) {
        self.loading = true;
        self.search_in_progress = false;
        self.load_time_ms = None;
        self.error_message = None;
    }

    /// 进入全分区搜索态
    pub fn begin_partition_search(&mut self) {
        if self.search_results.is_none() {
            self.browse_page_before_search = Some(self.current_page);
        }
        self.loading = true;
        self.search_in_progress = true;
        self.load_time_ms = None;
        self.error_message = None;
        self.selected_index = None;
        self.detail_content = text_editor::Content::new();
    }

    /// 清理临时反馈
    pub fn clear_feedback(&mut self) {
        self.loading = false;
        self.search_in_progress = false;
        self.load_time_ms = None;
        self.error_message = None;
    }

    /// 清理全分区搜索结果并恢复浏览态
    pub fn clear_search_results(&mut self) {
        self.search_results = None;
        self.search_scanned_messages = None;
        self.current_page = self.browse_page_before_search.take().unwrap_or(0);
        self.selected_index = None;
        self.detail_content = text_editor::Content::new();
        self.loading = false;
        self.search_in_progress = false;
        self.error_message = None;
    }

    /// 应用页数据
    pub fn apply_page_data(&mut self, data: PageData, load_time_ms: u128) {
        self.search_results = None;
        self.search_scanned_messages = None;
        self.browse_page_before_search = None;
        self.messages = data.messages;
        self.current_page = data.page;
        self.total_messages = data.total_messages;
        self.low_watermark = data.low_watermark;
        self.high_watermark = data.high_watermark;
        self.loading = false;
        self.search_in_progress = false;
        self.load_time_ms = Some(load_time_ms);
        self.selected_index = None;
        self.detail_content = text_editor::Content::new();
        self.error_message = None;
    }

    /// 应用全分区搜索结果
    pub fn apply_search_results(
        &mut self,
        results: Vec<DecodedMessage>,
        scanned_messages: usize,
        low_watermark: i64,
        high_watermark: i64,
        load_time_ms: u128,
    ) {
        self.search_results = Some(results);
        self.search_scanned_messages = Some(scanned_messages);
        self.current_page = 0;
        self.total_messages = self
            .search_results
            .as_ref()
            .map_or(0, |items| items.len() as i64);
        self.low_watermark = low_watermark;
        self.high_watermark = high_watermark;
        self.loading = false;
        self.search_in_progress = false;
        self.load_time_ms = Some(load_time_ms);
        self.selected_index = None;
        self.detail_content = text_editor::Content::new();
        self.error_message = None;
        self.apply_search_page();
    }

    /// 在本地分页展示全分区搜索结果
    pub fn apply_search_page(&mut self) {
        let Some(results) = &self.search_results else {
            return;
        };

        let start = self.current_page.saturating_mul(self.page_size);
        let end = (start + self.page_size).min(results.len());

        self.messages = if start < end {
            results[start..end].to_vec()
        } else {
            Vec::new()
        };
        self.total_messages = results.len() as i64;
        self.selected_index = None;
        self.detail_content = text_editor::Content::new();
        self.loading = false;
        self.search_in_progress = false;
    }

    /// 记录加载失败
    pub fn fail_loading(&mut self, error: impl Into<String>) {
        self.loading = false;
        self.search_in_progress = false;
        self.load_time_ms = None;
        self.error_message = Some(error.into());
    }

    /// 是否处于全分区搜索结果模式
    pub fn has_search_results(&self) -> bool {
        self.search_results.is_some()
    }

    /// 获取当前选中的消息
    pub fn selected_message(&self) -> Option<&DecodedMessage> {
        self.selected_index.and_then(|idx| self.messages.get(idx))
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::TableState;
    use crate::kafka::types::{DecodedMessage, DecodedPayload, KafkaMessage};

    fn sample_message(offset: i64) -> DecodedMessage {
        DecodedMessage {
            raw: KafkaMessage {
                topic: "orders".to_string(),
                partition: 0,
                offset,
                timestamp: Some(chrono::Utc.with_ymd_and_hms(2026, 3, 19, 10, 0, 0).unwrap()),
                key: None,
                payload: None,
            },
            decoded_key: Some(format!("order-{offset}")),
            decoded_value: DecodedPayload::Text(format!("value-{offset}")),
        }
    }

    #[test]
    fn loading_feedback_is_updated_consistently() {
        let mut state = TableState::default();

        state.begin_loading();
        assert!(state.loading);
        assert!(state.error_message.is_none());
        assert!(state.load_time_ms.is_none());

        state.fail_loading("load failed");
        assert!(!state.loading);
        assert_eq!(state.error_message.as_deref(), Some("load failed"));
        assert!(state.load_time_ms.is_none());

        state.clear_feedback();
        assert!(!state.loading);
        assert!(state.error_message.is_none());
        assert!(state.load_time_ms.is_none());
    }

    #[test]
    fn setting_search_query_clears_selection() {
        let mut state = TableState {
            selected_index: Some(3),
            ..TableState::default()
        };

        state.set_search_query("orders".to_string());

        assert_eq!(state.search_query, "orders");
        assert!(state.selected_index.is_none());
    }

    #[test]
    fn search_results_can_paginate_and_restore_browse_page() {
        let mut state = TableState {
            current_page: 2,
            page_size: 2,
            ..TableState::default()
        };

        state.begin_partition_search();
        state.apply_search_results(
            vec![sample_message(1), sample_message(2), sample_message(3)],
            12,
            0,
            12,
            8,
        );

        assert_eq!(state.current_page, 0);
        assert_eq!(state.total_messages, 3);
        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.search_scanned_messages, Some(12));

        state.current_page = 1;
        state.apply_search_page();
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].raw.offset, 3);

        state.clear_search_results();
        assert_eq!(state.current_page, 2);
        assert!(state.search_results.is_none());
        assert!(state.search_scanned_messages.is_none());
    }
}
