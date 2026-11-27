use crate::{api_client::ApiClient, evaluation::Outcome};
use crate::{models::EvaluationScores, stats::TrainingStats};
use rand::RngExt;
use ratatui::{layout::Rect, style::Style};
use ratatui_textarea::{TextArea, WrapMode};

#[derive(PartialEq, Clone, Copy)]
pub enum ViewMode {
    Menu,
    Normal,
    Editing,
    Report,
    Help,
    Settings,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SettingsFocus {
    Groq,
    Jev,
    Save,
    Cancel,
}

pub struct SettingsState {
    pub focus: SettingsFocus,
    pub groq_input: String,
    pub jev_input: String,
    pub groq_from_env: bool,
    pub groq_saved: bool,
    pub jev_saved: bool,
    return_mode: ViewMode,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            focus: SettingsFocus::Groq,
            groq_input: String::new(),
            jev_input: String::new(),
            groq_from_env: false,
            groq_saved: false,
            jev_saved: false,
            return_mode: ViewMode::Menu,
        }
    }
}

pub const MENU_OPTIONS: [u16; 4] = [400, 720, 1440, 2880];
pub const OVERLAY_SIZE_PERCENT: u16 = 75;
pub const SUMMARY_LINE_ENDING: &str = if cfg!(windows) { "\r\n" } else { "\n" };
pub const OVERLAY_MARGIN: u16 = 2;
pub const MIN_OVERLAY_WIDTH: u16 = 40;
pub const MIN_OVERLAY_HEIGHT: u16 = 10;
const BLOCK_BORDER_SIZE: u16 = 2;

const GENERATION_ERROR_PREFIX: &str = "文章生成に失敗しました: ";
pub const STATUS_MENU: &str = "文字数を選び ENTER で開始";
pub const STATUS_NORMAL: &str = "通常モード。'i' で要約を入力。";
pub const STATUS_EDITING: &str = "入力モード。Esc で戻る。";
pub const STATUS_REPORT: &str = "レポート表示中。'r' で閉じる。";
pub const STATUS_HELP: &str = "ヘルプ表示中。'h' で閉じる";
pub const STATUS_GENERATING: &str = "文章を生成中...";
pub const STATUS_NEXT_GENERATING: &str = "次の文章を生成中...";
const STATUS_GENERATION_FAILED: &str = "文章を生成できませんでした。'n' で再試行。";
pub const STATUS_EVALUATING: &str = "要約を評価中...";
pub const STATUS_EVALUATED: &str = "評価が完了。'e' で切替、'n' で次の問題。";
pub const STATUS_INVALID_EVALUATION: &str = "評価結果の形式が不正。";
pub const STATUS_RUNTIME_ERROR: &str = "エラーが発生しました。";

pub struct App {
    pub api_client: Option<ApiClient>,
    pub jev_api_client: Option<ApiClient>,
    pub original_text: Option<String>,
    pub generation_error: Option<String>,
    pub original_text_scroll: u16,
    pub evaluation_text: String,
    pub status_message: String,
    pub summary_input: TextArea<'static>,
    pub evaluation_overlay_scroll: u16,
    pub view_mode: ViewMode,
    pub stats: TrainingStats,
    pub stats_load_error: Option<String>,
    pub pending_evaluation: Option<EvaluationScores>,
    pub character_count: u16,
    pub selected_menu_item: usize,
    pub diagnosis_mode: bool,
    pub help_scroll: u16,
    pub should_quit: bool,
    pub evaluation_outcome: Option<Outcome>,
    pub show_evaluation_overlay: bool,
    pub terminal_width: u16,
    pub terminal_height: u16,
    pub settings: SettingsState,
}

impl Default for App {
    fn default() -> Self {
        Self {
            api_client: None,
            jev_api_client: None,
            original_text: None,
            generation_error: None,
            original_text_scroll: 0,
            evaluation_text: String::new(),
            status_message: STATUS_MENU.to_string(),
            summary_input: Self::new_summary_input(),
            evaluation_overlay_scroll: 0,
            view_mode: ViewMode::Menu,
            stats: TrainingStats::default(),
            stats_load_error: None,
            pending_evaluation: None,
            character_count: 400,
            selected_menu_item: 0,
            diagnosis_mode: false,
            help_scroll: 0,
            should_quit: false,
            evaluation_outcome: None,
            show_evaluation_overlay: false,
            terminal_width: 100,
            terminal_height: 30,
            settings: SettingsState::default(),
        }
    }
}

impl App {
    fn new_summary_input() -> TextArea<'static> {
        let mut input = TextArea::default();
        input.set_wrap_mode(WrapMode::WordOrGlyph);
        input.set_cursor_line_style(Style::default());
        input.set_cursor_style(Style::default());
        input.set_tab_length(8);
        input
    }

    pub fn generate_text_prompt(&self) -> String {
        let mut rng = rand::rng();

        let style_prompt = if rng.random_bool(0.7) {
            "日本の公的文書（省庁や自治体が発行する通知や報告書）の文体で、感情表現や口語表現を避け、形式的かつ客観的な文章を"
        } else {
            "日本の新聞記事の本文として、事実関係を中心に客観的かつ簡潔な文体で文章を"
        };

        format!(
            "{}{}文字程度で生成してください。",
            style_prompt, self.character_count
        )
        .repeat(2)
    }

    pub fn return_from_aux_view(&mut self) {
        if self.original_text.is_some() || self.generation_error.is_some() {
            self.view_mode = ViewMode::Normal;
            self.status_message = if self.generation_error.is_some() {
                STATUS_GENERATION_FAILED
            } else {
                STATUS_NORMAL
            }
            .to_string();
        } else {
            self.view_mode = ViewMode::Menu;
            self.status_message = STATUS_MENU.to_string();
        }
    }

    pub fn enter_report_view(&mut self) {
        self.view_mode = ViewMode::Report;
        self.status_message = STATUS_REPORT.to_string();
    }

    pub fn enter_help_view(&mut self) {
        self.view_mode = ViewMode::Help;
        self.status_message = STATUS_HELP.to_string();
    }

    pub fn enter_settings_view(&mut self, groq_from_env: bool) {
        self.settings.return_mode = self.view_mode;
        self.view_mode = ViewMode::Settings;
        self.settings.focus = SettingsFocus::Groq;
        self.settings.groq_input.clear();
        self.settings.jev_input.clear();
        self.settings.groq_from_env = groq_from_env;
        self.status_message.clear();
    }

    pub fn return_from_settings(&mut self) {
        self.settings.groq_input.clear();
        self.settings.jev_input.clear();
        self.view_mode = self.settings.return_mode;
        self.status_message = if self.view_mode == ViewMode::Normal {
            STATUS_NORMAL
        } else {
            STATUS_MENU
        }
        .to_string();
    }

    pub fn begin_editing(&mut self) {
        self.view_mode = ViewMode::Editing;
        self.status_message = STATUS_EDITING.to_string();
    }

    pub fn stop_editing(&mut self) {
        self.view_mode = ViewMode::Normal;
        self.status_message = STATUS_NORMAL.to_string();
    }

    pub fn begin_training_generation(&mut self, is_next_training: bool) {
        self.original_text = None;
        self.generation_error = None;
        self.view_mode = ViewMode::Normal;
        self.status_message = if is_next_training {
            STATUS_NEXT_GENERATING
        } else {
            STATUS_GENERATING
        }
        .to_string();
    }

    pub fn apply_generated_text(&mut self, text: String) {
        self.original_text = Some(text);
        self.generation_error = None;
        self.status_message = STATUS_NORMAL.to_string();
    }

    pub fn apply_generation_error(&mut self, error: &dyn std::fmt::Display) {
        self.original_text = None;
        self.generation_error = Some(format!("{GENERATION_ERROR_PREFIX}{error}"));
        self.status_message = STATUS_GENERATION_FAILED.to_string();
    }

    pub fn begin_evaluation(&mut self) {
        self.status_message = STATUS_EVALUATING.to_string();
    }

    pub fn finish_evaluation(&mut self, text: String, outcome: Outcome) {
        self.evaluation_text = text;
        self.evaluation_outcome = Some(outcome);
        self.show_evaluation_overlay = true;
        self.evaluation_overlay_scroll = 0;
        self.status_message = STATUS_EVALUATED.to_string();
    }

    pub fn fail_evaluation_format(&mut self) {
        self.evaluation_text = STATUS_INVALID_EVALUATION.to_string();
        self.evaluation_outcome = None;
        self.show_evaluation_overlay = true;
        self.evaluation_overlay_scroll = 0;
        self.status_message = STATUS_INVALID_EVALUATION.to_string();
    }

    pub fn fail_evaluation_request(&mut self, error: &dyn std::fmt::Display) {
        self.evaluation_text = format!("エラー: {error}");
        self.evaluation_outcome = None;
        self.show_evaluation_overlay = true;
        self.evaluation_overlay_scroll = 0;
        self.status_message = STATUS_RUNTIME_ERROR.to_string();
    }

    pub fn prepare_next_training(&mut self) {
        self.show_evaluation_overlay = false;
        self.evaluation_text.clear();
        self.evaluation_outcome = None;
        let clipboard = self.summary_input.yank_text();
        self.summary_input = Self::new_summary_input();
        self.summary_input.set_yank_text(clipboard);
        self.original_text_scroll = 0;
        self.evaluation_overlay_scroll = 0;
        self.begin_training_generation(true);
    }

    pub fn update_terminal_size(&mut self, width: u16, height: u16) {
        self.terminal_width = width;
        self.terminal_height = height;
    }

    pub fn calculate_overlay_area(&self) -> Rect {
        Self::calculate_overlay_area_for_size(self.terminal_width, self.terminal_height)
    }

    pub fn calculate_overlay_area_for_size(width: u16, height: u16) -> Rect {
        let full_area = Rect::new(0, 0, width, height);
        let max_overlay_width = full_area
            .width
            .saturating_sub(OVERLAY_MARGIN.saturating_mul(2));
        let max_overlay_height = full_area
            .height
            .saturating_sub(OVERLAY_MARGIN.saturating_mul(2));

        let overlay_width = full_area
            .width
            .saturating_mul(OVERLAY_SIZE_PERCENT)
            .saturating_div(100)
            .max(MIN_OVERLAY_WIDTH)
            .min(max_overlay_width);
        let overlay_height = full_area
            .height
            .saturating_mul(OVERLAY_SIZE_PERCENT)
            .saturating_div(100)
            .max(MIN_OVERLAY_HEIGHT)
            .min(max_overlay_height);

        Rect {
            x: full_area.x + full_area.width.saturating_sub(overlay_width) / 2,
            y: full_area.y + full_area.height.saturating_sub(overlay_height) / 2,
            width: overlay_width,
            height: overlay_height,
        }
    }

    pub fn evaluation_viewport_size(&self) -> (u16, u16) {
        let overlay_area = self.calculate_overlay_area();
        (
            overlay_area.height.saturating_sub(BLOCK_BORDER_SIZE),
            overlay_area.width.saturating_sub(BLOCK_BORDER_SIZE),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returning_from_settings_clears_entered_api_keys() {
        let mut app = App::default();
        app.enter_settings_view(false);
        app.settings.groq_input = "groq-secret".to_string();
        app.settings.jev_input = "jev-secret".to_string();

        app.return_from_settings();

        assert_eq!(app.settings.groq_input, "");
        assert_eq!(app.settings.jev_input, "");
    }
}
