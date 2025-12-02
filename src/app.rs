use crate::api_client::ApiClient;

/// Application state
pub struct App {
    pub api_client: Option<ApiClient>,
    pub is_editing: bool,
    pub original_text: String,
    pub original_text_scroll: u16,
    pub evaluation_text: String,
    pub evaluation_text_scroll: u16,
    pub status_message: String,
    pub should_quit: bool,
    pub summary_input: String,
    pub cursor_position: usize,
    pub is_evaluating: bool,
    pub show_evaluation: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            api_client: None,
            is_editing: false,
            original_text: "Authenticating...".to_string(),
            original_text_scroll: 0,
            evaluation_text: String::new(),
            evaluation_text_scroll: 0,
            status_message: "Authenticating... please wait.".to_string(),
            should_quit: false,
            summary_input: String::new(),
            cursor_position: 0,
            is_evaluating: false,
            show_evaluation: false,
        }
    }
}
