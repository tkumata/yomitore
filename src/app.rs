use crate::api_client::ApiClientLike;
use crate::stats::TrainingStats;
use rand::Rng;
use rat_text::text_area::{TextAreaState, TextWrap};

#[derive(PartialEq, Clone, Copy)]
pub enum ViewMode {
    Menu,
    Normal,
    Report,
    Help,
}

/// Menu options for character count selection
pub const MENU_OPTIONS: [u16; 4] = [400, 720, 1440, 2880];

/// Application state
pub struct App {
    pub api_client: Option<Box<dyn ApiClientLike>>,
    pub is_editing: bool,
    pub original_text: String,
    pub original_text_scroll: u16,
    pub evaluation_text: String,
    pub evaluation_passed: bool,
    pub status_message: String,
    pub should_quit: bool,
    pub text_area_state: TextAreaState,
    pub is_evaluating: bool,
    pub show_evaluation_overlay: bool,
    pub evaluation_overlay_scroll: u16,
    pub view_mode: ViewMode,
    pub stats: TrainingStats,
    pub character_count: u16,
    pub selected_menu_item: usize,
    pub help_scroll: u16,
    pub terminal_width: u16,
    pub terminal_height: u16,
}

impl Default for App {
    fn default() -> Self {
        let stats = TrainingStats::load().unwrap_or_else(|_| TrainingStats::new());

        // Initialize TextAreaState for rat-text
        let text_area_state = Self::new_text_area_state();

        Self {
            api_client: None,
            is_editing: false,
            original_text: "Authenticating...".to_string(),
            original_text_scroll: 0,
            evaluation_text: String::new(),
            evaluation_passed: false,
            status_message: "Select character count and press Enter to start".to_string(),
            should_quit: false,
            text_area_state,
            is_evaluating: false,
            show_evaluation_overlay: false,
            evaluation_overlay_scroll: 0,
            view_mode: ViewMode::Menu,
            stats,
            character_count: 400,
            selected_menu_item: 0,
            help_scroll: 0,
            terminal_width: 100, // Default, will be updated on first render
            terminal_height: 30, // Default, will be updated on first render
        }
    }
}

impl App {
    pub fn new_text_area_state() -> TextAreaState {
        let mut state = TextAreaState::default();
        state.set_text_wrap(TextWrap::Word(2)); // prefer safe word-wrap
        state
    }

    /// Generate the text generation prompt based on current character count
    pub fn generate_text_prompt(&self) -> String {
        let mut rng = rand::rng();

        let style_prompt = if rng.random_bool(0.7) {
            // 公的文書
            "日本の公的文書（省庁や自治体が発行する通知や報告書）の文体で、感情表現や口語表現を避け、形式的かつ客観的な文章を"
        } else {
            // 新聞記事
            "日本の新聞記事の本文として、事実関係を中心に客観的かつ簡潔な文体で文章を"
        };

        format!(
            "{}{}文字程度で生成してください。",
            style_prompt, self.character_count
        )
        .repeat(2)
    }

    /// Check if the current state indicates no training has started
    pub fn has_training_started(&self) -> bool {
        self.original_text != "Authenticating..."
            && !self.original_text.starts_with("Failed to generate")
    }

    /// Return to the appropriate view mode (Menu if no training, Normal otherwise)
    pub fn return_from_aux_view(&mut self) {
        if self.has_training_started() {
            self.view_mode = ViewMode::Normal;
            self.status_message = "Normal Mode. Press 'i' to edit.".to_string();
        } else {
            self.view_mode = ViewMode::Menu;
            self.status_message = "Select character count and press Enter to start".to_string();
        }
    }
}
