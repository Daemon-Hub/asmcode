use crate::autocomplete::engine::{AutocompleteEngine, Suggestion};
use crate::editor::undo::UndoManager;
use crate::fs_manager::explorer::FileManager;
use crate::session::{self, AppSession};
use crate::ui::AppWindow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ChildStdin;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct RunningProcess {
    pub pid: u32,
    pub stdin: Arc<Mutex<ChildStdin>>,
}

#[derive(Default, Clone)]
pub struct FindState {
    pub query: String,
    pub match_case: bool,
    pub whole_word: bool,
    pub matches: Vec<(usize, usize)>,
    pub current_idx: usize,
}

pub struct IdeController {
    pub file_manager: FileManager,
    pub autocomplete: AutocompleteEngine,
    pub active_suggestions: Vec<Suggestion>,
    pub logs: Vec<String>,
    pub undo_manager: UndoManager,
    pub open_tabs: Vec<PathBuf>,
    pub pinned_tabs: HashSet<PathBuf>,
    pub deleted_open_tabs: HashSet<PathBuf>,
    pub untitled_drafts: HashSet<PathBuf>,
    pub tab_buffers: HashMap<PathBuf, String>,
    pub dirty_tabs: HashSet<PathBuf>,
    pub find_state: FindState,
    pub session: AppSession,
    pub is_large_file_mode: bool,
    pub force_full_features: bool,
    pub running_process: Option<RunningProcess>,
}

impl IdeController {
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Self {
        Self {
            file_manager: FileManager::new(root_dir),
            autocomplete: AutocompleteEngine::new(),
            active_suggestions: Vec::new(),
            logs: vec![
                "[SYSTEM] ASM Code Intel 8080 IDE ready.".to_string(),
                "[TOOLCHAIN] Ready (pure-Rust intel-8080-kit built-in).".to_string(),
            ],
            undo_manager: UndoManager::new(String::new(), 0),
            open_tabs: Vec::new(),
            pinned_tabs: HashSet::new(),
            deleted_open_tabs: HashSet::new(),
            untitled_drafts: HashSet::new(),
            tab_buffers: HashMap::new(),
            dirty_tabs: HashSet::new(),
            find_state: FindState::default(),
            session: AppSession::default(),
            is_large_file_mode: false,
            force_full_features: false,
            running_process: None,
        }
    }

    pub fn append_log(&mut self, message: &str) {
        self.logs.push(message.to_string());
    }

    pub fn get_combined_logs(&self) -> String {
        self.logs.join("\n")
    }
}

pub fn persist_current_session(app: &AppWindow, ctrl: &IdeController) {
    let open_tabs: Vec<String> = ctrl
        .open_tabs
        .iter()
        .filter(|p| !ctrl.untitled_drafts.contains(*p))
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let session = AppSession {
        last_open_folder: Some(ctrl.file_manager.current_dir.to_string_lossy().to_string()),
        last_open_file: ctrl
            .file_manager
            .active_file_path
            .as_ref()
            .filter(|p| !ctrl.untitled_drafts.contains(*p))
            .map(|p| p.to_string_lossy().to_string()),
        open_tabs,
        editor_font_size: app.get_editor_font_size(),
        editor_font_italic: app.get_editor_font_italic(),
        current_language: app.get_current_language().to_string(),
        labels_expanded: app.get_labels_expanded(),
        terminal_expanded: app.get_terminal_expanded(),
        sidebar_width: app.get_sidebar_width(),
        terminal_height: app.get_terminal_height(),
        active_theme_id: app.get_active_theme_id(),
        cursor_blink: app.get_cursor_blink(),
        tab_size: app.get_tab_size(),
        compiler_path: app.get_compiler_path().to_string(),
        emulator_path: app.get_emulator_path().to_string(),
        toolchain_type: app.get_toolchain_type(),
        large_file_optimizations: ctrl.session.large_file_optimizations,
        large_file_soft_limit_mb: ctrl.session.large_file_soft_limit_mb,
        large_file_soft_limit_lines: ctrl.session.large_file_soft_limit_lines,
        large_file_hard_limit_mb: ctrl.session.large_file_hard_limit_mb,
        associate_asm_files: app.get_associate_asm_files(),
    };
    session::save_session(&session);
}
