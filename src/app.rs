use crate::api_client::ApiClient;
use crate::stats::TrainingStats;
use rand::RngExt;
use rat_text::text_area::{TextAreaState, TextWrap};
use ratatui::layout::Rect;

#[derive(PartialEq, Clone, Copy)]
pub enum ViewMode {
    Menu,
    Normal,
    Report,
    Help,
}

pub const MENU_OPTIONS: [u16; 4] = [400, 720, 1440, 2880];
pub const OVERLAY_SIZE_PERCENT: u16 = 75;
pub const TEXT_WRAP_MARGIN: u16 = 2;
pub const OVERLAY_MARGIN: u16 = 2;
pub const MIN_OVERLAY_WIDTH: u16 = 40;
pub const MIN_OVERLAY_HEIGHT: u16 = 10;
const HEADER_HEIGHT: u16 = 1;
const STATUS_HEIGHT: u16 = 3;
const BLOCK_BORDER_SIZE: u16 = 2;

pub const INITIAL_ORIGINAL_TEXT: &str = "認証しています...";
pub const GENERATION_ERROR_PREFIX: &str = "文章生成に失敗しました: ";
pub const STATUS_MENU: &str = "文字数を選び、開始してください。";
pub const STATUS_NORMAL: &str = "通常モードです。'i' で入力します。";
pub const STATUS_EDITING: &str = "入力モードです。Esc で戻ります。";
pub const STATUS_REPORT: &str = "レポート表示中です。'r' で閉じます。";
pub const STATUS_HELP: &str = "ヘルプ表示中です。'h' で閉じます。";
pub const STATUS_GENERATING: &str = "文章を生成しています...";
pub const STATUS_NEXT_GENERATING: &str = "次の文章を生成しています...";
pub const STATUS_EVALUATING: &str = "要約を評価しています...";
pub const STATUS_EVALUATED: &str = "評価が完了しました。'e' で切替、'n' で次へ進みます。";
pub const STATUS_INVALID_EVALUATION: &str = "評価結果の形式が不正です。";
pub const STATUS_RUNTIME_ERROR: &str = "エラーが発生しました。";

pub struct App {
    pub api_client: Option<ApiClient>,
    pub original_text: String,
    pub original_text_scroll: u16,
    pub evaluation_text: String,
    pub status_message: String,
    pub text_area_state: TextAreaState,
    pub evaluation_overlay_scroll: u16,
    pub view_mode: ViewMode,
    pub stats: TrainingStats,
    pub character_count: u16,
    pub selected_menu_item: usize,
    pub help_scroll: u16,
    pub should_quit: bool,
    pub evaluation_passed: bool,
    pub show_evaluation_overlay: bool,
    pub terminal_width: u16,
    pub terminal_height: u16,
}

impl Default for App {
    fn default() -> Self {
        let stats = TrainingStats::load().unwrap_or_default();

        let text_area_state = Self::new_text_area_state();

        Self {
            api_client: None,
            original_text: INITIAL_ORIGINAL_TEXT.to_string(),
            original_text_scroll: 0,
            evaluation_text: String::new(),
            status_message: STATUS_MENU.to_string(),
            text_area_state,
            evaluation_overlay_scroll: 0,
            view_mode: ViewMode::Menu,
            stats,
            character_count: 400,
            selected_menu_item: 0,
            help_scroll: 0,
            should_quit: false,
            evaluation_passed: false,
            show_evaluation_overlay: false,
            terminal_width: 100,
            terminal_height: 30,
        }
    }
}

impl App {
    pub fn new_text_area_state() -> TextAreaState {
        let mut state = TextAreaState::default();
        state.set_text_wrap(TextWrap::Word(TEXT_WRAP_MARGIN));
        state
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

    pub fn has_training_started(&self) -> bool {
        self.original_text != INITIAL_ORIGINAL_TEXT
            && !self.original_text.starts_with(GENERATION_ERROR_PREFIX)
    }

    pub fn return_from_aux_view(&mut self) {
        if self.has_training_started() {
            self.view_mode = ViewMode::Normal;
            self.status_message = STATUS_NORMAL.to_string();
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

    pub fn begin_editing(&mut self) {
        self.text_area_state.focus.set(true);
        self.text_area_state.scroll_cursor_to_visible();
        self.status_message = STATUS_EDITING.to_string();
    }

    pub fn stop_editing(&mut self) {
        self.text_area_state.focus.set(false);
        self.status_message = STATUS_NORMAL.to_string();
    }

    pub fn begin_training_generation(&mut self, is_next_training: bool) {
        self.view_mode = ViewMode::Normal;
        self.status_message = if is_next_training {
            STATUS_NEXT_GENERATING
        } else {
            STATUS_GENERATING
        }
        .to_string();
    }

    pub fn apply_generated_text(&mut self, text: String) {
        self.original_text = text;
        self.status_message = STATUS_NORMAL.to_string();
    }

    pub fn apply_generation_error(&mut self, error: &impl std::fmt::Display) {
        self.original_text = format!("{GENERATION_ERROR_PREFIX}{error}");
        self.status_message = STATUS_RUNTIME_ERROR.to_string();
    }

    pub fn begin_evaluation(&mut self) {
        self.status_message = STATUS_EVALUATING.to_string();
    }

    pub fn finish_evaluation(&mut self, text: String, passed: bool) {
        self.evaluation_text = text;
        self.evaluation_passed = passed;
        self.show_evaluation_overlay = true;
        self.evaluation_overlay_scroll = 0;
        self.status_message = STATUS_EVALUATED.to_string();
    }

    pub fn fail_evaluation_format(&mut self) {
        self.evaluation_text = STATUS_INVALID_EVALUATION.to_string();
        self.evaluation_passed = false;
        self.show_evaluation_overlay = true;
        self.evaluation_overlay_scroll = 0;
        self.status_message = STATUS_INVALID_EVALUATION.to_string();
    }

    pub fn fail_evaluation_request(&mut self, error: &impl std::fmt::Display) {
        self.evaluation_text = format!("エラー: {error}");
        self.evaluation_passed = false;
        self.show_evaluation_overlay = true;
        self.evaluation_overlay_scroll = 0;
        self.status_message = STATUS_RUNTIME_ERROR.to_string();
    }

    pub fn prepare_next_training(&mut self) {
        self.show_evaluation_overlay = false;
        self.evaluation_text.clear();
        self.evaluation_passed = false;
        self.text_area_state = Self::new_text_area_state();
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

    pub fn original_text_viewport_size(&self) -> (u16, u16) {
        let content_height = self
            .terminal_height
            .saturating_sub(HEADER_HEIGHT + STATUS_HEIGHT);
        let pane_width = self.terminal_width / 2;
        (
            content_height.saturating_sub(BLOCK_BORDER_SIZE),
            pane_width.saturating_sub(BLOCK_BORDER_SIZE),
        )
    }

    pub fn evaluation_viewport_size(&self) -> (u16, u16) {
        let overlay_area = self.calculate_overlay_area();
        (
            overlay_area.height.saturating_sub(BLOCK_BORDER_SIZE),
            overlay_area.width.saturating_sub(BLOCK_BORDER_SIZE),
        )
    }
}
