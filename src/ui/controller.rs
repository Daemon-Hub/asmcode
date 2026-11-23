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

#[derive(Clone, Debug)]
pub struct ViewportCache {
    pub buffer_start: usize,
    pub buffer_end: usize,
    pub path: Option<PathBuf>,
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
    pub tab_line_starts: HashMap<PathBuf, Vec<usize>>,
    pub viewport_cache: Option<ViewportCache>,
    pub dirty_tabs: HashSet<PathBuf>,
    pub find_state: FindState,
    pub session: AppSession,
    pub is_large_file_mode: bool,
    pub force_full_features: bool,
    pub running_process: Option<RunningProcess>,
    pub terminal_partial_line: bool,
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
            tab_line_starts: HashMap::new(),
            viewport_cache: None,
            dirty_tabs: HashSet::new(),
            find_state: FindState::default(),
            session: AppSession::default(),
            is_large_file_mode: false,
            force_full_features: false,
            running_process: None,
            terminal_partial_line: false,
        }
    }

    pub fn set_tab_buffer(&mut self, path: PathBuf, content: String) {
        let starts = crate::syntax::intel8080::build_line_starts(&content);
        self.tab_line_starts.insert(path.clone(), starts);
        self.tab_buffers.insert(path, content);
        self.viewport_cache = None;
    }

    pub fn remove_tab_buffer(&mut self, path: &Path) {
        self.tab_line_starts.remove(path);
        self.tab_buffers.remove(path);
        self.viewport_cache = None;
    }

    pub fn invalidate_viewport_cache(&mut self) {
        self.viewport_cache = None;
    }

    pub fn append_log(&mut self, message: &str) {
        self.terminal_partial_line = false;
        self.logs.push(message.to_string());
    }

    #[allow(dead_code)]
    pub fn append_terminal_stream(&mut self, text: &str) {
        for line in text.split_inclusive('\n') {
            let has_newline = line.ends_with('\n');
            let content = line.trim_end_matches(['\r', '\n']);
            if content.is_empty() && !has_newline {
                continue;
            }

            if self.terminal_partial_line {
                if let Some(last) = self.logs.last_mut() {
                    last.push_str(content);
                } else {
                    self.logs.push(format!("[CPM] {}", content));
                }
            } else if !content.is_empty() {
                self.logs.push(format!("[CPM] {}", content));
            }

            self.terminal_partial_line = !has_newline;
        }
    }

    #[allow(dead_code)]
    pub fn append_terminal_err_stream(&mut self, text: &str) {
        for line in text.split_inclusive('\n') {
            let has_newline = line.ends_with('\n');
            let content = line.trim_end_matches(['\r', '\n']);
            if content.is_empty() && !has_newline {
                continue;
            }

            if self.terminal_partial_line {
                if let Some(last) = self.logs.last_mut() {
                    last.push_str(content);
                } else {
                    self.logs.push(format!("[CPM ERR] {}", content));
                }
            } else if !content.is_empty() {
                self.logs.push(format!("[CPM ERR] {}", content));
            }

            self.terminal_partial_line = !has_newline;
        }
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
        compiler_args: app.get_compiler_args().to_string(),
        emulator_path: app.get_emulator_path().to_string(),
        emulator_args: app.get_emulator_args().to_string(),
        toolchain_type: app.get_toolchain_type(),
        large_file_optimizations: ctrl.session.large_file_optimizations,
        large_file_soft_limit_mb: ctrl.session.large_file_soft_limit_mb,
        large_file_soft_limit_lines: ctrl.session.large_file_soft_limit_lines,
        large_file_hard_limit_mb: ctrl.session.large_file_hard_limit_mb,
        associate_asm_files: app.get_associate_asm_files(),
    };
    session::save_session(&session);
}
