use crate::api_client::ApiClient;
use crate::stats::TrainingStats;

#[derive(PartialEq, Clone, Copy)]
pub enum ViewMode {
    Menu,
    Normal,
    MonthlyReport,
    WeeklyReport,
}

/// Application state
pub struct App {
    pub api_client: Option<ApiClient>,
    pub is_editing: bool,
    pub original_text: String,
    pub original_text_scroll: u16,
    pub evaluation_text: String,
    pub evaluation_text_scroll: u16,
    pub evaluation_passed: bool,
    pub status_message: String,
    pub should_quit: bool,
    pub summary_input: String,
    pub cursor_position: usize,
    pub is_evaluating: bool,
    pub show_evaluation: bool,
    pub view_mode: ViewMode,
    pub stats: TrainingStats,
    pub character_count: u16,
    pub selected_menu_item: usize,
}

impl Default for App {
    fn default() -> Self {
        let stats = TrainingStats::load().unwrap_or_else(|_| TrainingStats::new());
        Self {
            api_client: None,
            is_editing: false,
            original_text: "Authenticating...".to_string(),
            original_text_scroll: 0,
            evaluation_text: String::new(),
            evaluation_text_scroll: 0,
            evaluation_passed: false,
            status_message: "Select character count and press Enter to start".to_string(),
            should_quit: false,
            summary_input: String::new(),
            cursor_position: 0,
            is_evaluating: false,
            show_evaluation: false,
            view_mode: ViewMode::Menu,
            stats,
            character_count: 400,
            selected_menu_item: 0,
        }
    }
}
