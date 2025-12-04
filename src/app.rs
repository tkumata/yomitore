use crate::api_client::ApiClient;
use crate::stats::TrainingStats;

#[derive(PartialEq, Clone, Copy)]
pub enum ViewMode {
    Menu,
    Normal,
    MonthlyReport,
    WeeklyReport,
}

/// Menu options for character count selection
pub const MENU_OPTIONS: [u16; 4] = [400, 720, 1440, 2880];

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

impl App {
    /// Generate the text generation prompt based on current character count
    pub fn generate_text_prompt(&self) -> String {
        format!(
            "日本語の公的文書のようなお堅い文章を{}文字程度で生成してください。",
            self.character_count
        )
    }

    /// Check if the current state indicates no training has started
    pub fn has_training_started(&self) -> bool {
        self.original_text != "Authenticating..." && !self.original_text.starts_with("Failed to generate")
    }

    /// Return to the appropriate view mode (Menu if no training, Normal otherwise)
    pub fn return_from_report(&mut self) {
        if self.has_training_started() {
            self.view_mode = ViewMode::Normal;
            self.status_message = "Normal Mode. Press 'i' to edit.".to_string();
        } else {
            self.view_mode = ViewMode::Menu;
            self.status_message = "Select character count and press Enter to start".to_string();
        }
    }
}
