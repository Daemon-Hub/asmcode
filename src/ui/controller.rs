use crate::autocomplete::engine::{AutocompleteEngine, Suggestion};
use crate::autocomplete::extractor::UserSymbol;
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
    pub cached_user_symbols: Arc<Vec<UserSymbol>>,
    pub cached_symbols_path: Option<PathBuf>,
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
            cached_user_symbols: Arc::new(Vec::new()),
            cached_symbols_path: None,
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

    /// Remaps all open-tab, buffer, dirty, pinned, and viewport paths when a file or folder is renamed.
    pub fn remap_path_prefix(&mut self, old_path: &Path, new_path: &Path) {
        if old_path == new_path {
            return;
        }

        let mut deduped = Vec::with_capacity(self.open_tabs.len());
        for tab in &self.open_tabs {
            let next = crate::fs_manager::explorer::remap_path_with_prefix(tab, old_path, new_path)
                .unwrap_or_else(|| tab.clone());
            if !deduped.contains(&next) {
                deduped.push(next);
            }
        }
        self.open_tabs = deduped;

        remap_path_set(&mut self.pinned_tabs, old_path, new_path);
        remap_path_set(&mut self.deleted_open_tabs, old_path, new_path);
        remap_path_set(&mut self.untitled_drafts, old_path, new_path);
        remap_path_set(&mut self.dirty_tabs, old_path, new_path);

        remap_path_map(&mut self.tab_buffers, old_path, new_path);
        remap_path_map(&mut self.tab_line_starts, old_path, new_path);

        if let Some(ref mut cache) = self.viewport_cache
            && let Some(ref cached_path) = cache.path
            && let Some(remapped) =
                crate::fs_manager::explorer::remap_path_with_prefix(cached_path, old_path, new_path)
        {
            cache.path = Some(remapped);
        }
        if let Some(ref sym_path) = self.cached_symbols_path
            && let Some(remapped) =
                crate::fs_manager::explorer::remap_path_with_prefix(sym_path, old_path, new_path)
        {
            self.cached_symbols_path = Some(remapped);
        }
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

fn remap_path_set(set: &mut HashSet<PathBuf>, old_path: &Path, new_path: &Path) {
    let updated: HashSet<PathBuf> = set
        .drain()
        .map(|p| {
            crate::fs_manager::explorer::remap_path_with_prefix(&p, old_path, new_path)
                .unwrap_or(p)
        })
        .collect();
    *set = updated;
}

fn remap_path_map<V>(map: &mut HashMap<PathBuf, V>, old_path: &Path, new_path: &Path) {
    let keys: Vec<PathBuf> = map
        .keys()
        .filter(|p| {
            crate::fs_manager::explorer::remap_path_with_prefix(p, old_path, new_path).is_some()
        })
        .cloned()
        .collect();
    for old_key in keys {
        if let Some(val) = map.remove(&old_key)
            && let Some(new_key) =
                crate::fs_manager::explorer::remap_path_with_prefix(&old_key, old_path, new_path)
        {
            map.insert(new_key, val);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remap_path_prefix_updates_file_and_nested_tabs() {
        let mut ctrl = IdeController::new("d:/tmp/proj");
        let old_file = PathBuf::from("d:/tmp/proj/old.asm");
        let new_file = PathBuf::from("d:/tmp/proj/new.asm");
        let other_file = PathBuf::from("d:/tmp/proj/sub/helper.asm");

        ctrl.open_tabs = vec![old_file.clone(), other_file.clone()];
        ctrl.pinned_tabs.insert(old_file.clone());
        ctrl.dirty_tabs.insert(old_file.clone());
        ctrl.set_tab_buffer(old_file.clone(), "mov a, b\n".to_string());

        ctrl.remap_path_prefix(&old_file, &new_file);

        assert_eq!(ctrl.open_tabs, vec![new_file.clone(), other_file.clone()]);
        assert!(!ctrl.pinned_tabs.contains(&old_file));
        assert!(ctrl.pinned_tabs.contains(&new_file));
        assert!(!ctrl.dirty_tabs.contains(&old_file));
        assert!(ctrl.dirty_tabs.contains(&new_file));
        assert!(!ctrl.tab_buffers.contains_key(&old_file));
        assert_eq!(
            ctrl.tab_buffers.get(&new_file).map(String::as_str),
            Some("mov a, b\n")
        );

        let old_dir = PathBuf::from("d:/tmp/proj/sub");
        let new_dir = PathBuf::from("d:/tmp/proj/inc");
        ctrl.remap_path_prefix(&old_dir, &new_dir);
        assert_eq!(
            ctrl.open_tabs,
            vec![new_file, PathBuf::from("d:/tmp/proj/inc/helper.asm")]
        );
    }
}
