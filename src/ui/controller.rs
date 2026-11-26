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
    pub total_lines: usize,
    pub theme_id: i32,
    pub is_large_mode: bool,
    pub path: Option<PathBuf>,
}

pub const LARGE_WINDOW_BYTES_THRESHOLD: usize = 2 * 1024 * 1024;
pub const LARGE_WINDOW_LINES_THRESHOLD: usize = 20_000;
pub const TEXT_WINDOW_BEFORE_LINES: usize = 150;
pub const TEXT_WINDOW_TARGET_LINES: usize = 450;
pub const TEXT_WINDOW_MARGIN_LINES: usize = 45;
pub const TEXT_WINDOW_MAX_BYTES: usize = 256 * 1024;
pub const TEXT_WINDOW_MAX_LINE_BYTES: usize = 8 * 1024;
pub const TEXT_WINDOW_TRUNCATED_LINE_BYTES: usize = 4096;

#[derive(Clone, Debug)]
pub struct LargeWindowState {
    pub path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub total_lines: usize,
    pub read_only: bool,
}

#[inline]
pub fn should_use_windowed_text(byte_len: usize, line_count: usize) -> bool {
    byte_len > LARGE_WINDOW_BYTES_THRESHOLD || line_count > LARGE_WINDOW_LINES_THRESHOLD
}

#[inline]
fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

pub fn build_window_slice_for_path(
    path: PathBuf,
    buf: &str,
    starts: &[usize],
    target_line: usize,
) -> (String, LargeWindowState, PathBuf) {
    let total_lines = starts.len().max(1);
    let clamped_target = target_line.min(total_lines.saturating_sub(1));
    let start_line = clamped_target.saturating_sub(TEXT_WINDOW_BEFORE_LINES);
    let max_end = (start_line + TEXT_WINDOW_TARGET_LINES).min(total_lines);
    let start_byte = starts.get(start_line).copied().unwrap_or(0).min(buf.len());

    let mut out = String::with_capacity(TEXT_WINDOW_MAX_BYTES.min(buf.len().saturating_sub(start_byte)));
    let mut end_line = start_line;
    let mut end_byte = start_byte;
    let mut read_only = false;

    for line_idx in start_line..max_end {
        let line_start = starts.get(line_idx).copied().unwrap_or(buf.len()).min(buf.len());
        let next_start = if line_idx + 1 < total_lines {
            starts[line_idx + 1].min(buf.len())
        } else {
            buf.len()
        };
        let slice_len = next_start.saturating_sub(line_start);

        if line_idx > clamped_target + 40 && out.len() + slice_len > TEXT_WINDOW_MAX_BYTES {
            break;
        }

        let raw_slice = &buf[line_start..next_start];
        let (line_body, has_nl) = match raw_slice.strip_suffix('\n') {
            Some(s) => (s, true),
            None => (raw_slice, false),
        };

        if line_body.len() > TEXT_WINDOW_MAX_LINE_BYTES {
            read_only = true;
            let cut = floor_char_boundary(line_body, TEXT_WINDOW_TRUNCATED_LINE_BYTES);
            out.push_str(&line_body[..cut]);
            out.push_str(" ... [truncated]");
            if has_nl {
                out.push('\n');
            }
        } else {
            out.push_str(raw_slice);
        }

        end_line = line_idx + 1;
        end_byte = next_start;
    }

    let state = LargeWindowState {
        path: path.clone(),
        start_line,
        end_line,
        start_byte,
        end_byte,
        total_lines,
        read_only,
    };
    (out, state, path)
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
    pub large_window: Option<LargeWindowState>,
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
            large_window: None,
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
        if self.large_window.as_ref().is_some_and(|w| w.path == path) {
            self.large_window = None;
        }
        self.viewport_cache = None;
    }

    pub fn active_document_text_cow(&self, ui: &AppWindow) -> std::borrow::Cow<'_, str> {
        if let Some(active_path) = &self.file_manager.active_file_path
            && let Some(buf) = self.tab_buffers.get(active_path)
            && (self.large_window.as_ref().is_some_and(|w| &w.path == active_path)
                || !ui.get_is_dirty())
        {
            return std::borrow::Cow::Borrowed(buf.as_str());
        }
        std::borrow::Cow::Owned(ui.get_code_text().to_string())
    }

    pub fn apply_window_mutation(&mut self, ui: &AppWindow, new_window_text: &str) -> bool {
        let Some(mut win) = self.large_window.clone() else {
            return false;
        };
        if win.read_only {
            return false;
        }
        let Some(buf) = self.tab_buffers.get_mut(&win.path) else {
            return false;
        };
        if win.start_byte > win.end_byte || win.end_byte > buf.len() {
            return false;
        }
        buf.replace_range(win.start_byte..win.end_byte, new_window_text);
        win.end_byte = win.start_byte + new_window_text.len();

        let starts = crate::syntax::intel8080::build_line_starts(buf);
        win.total_lines = starts.len().max(1);
        let end_line = match starts.binary_search(&win.end_byte) {
            Ok(idx) => idx,
            Err(idx) => idx,
        };
        win.end_line = end_line.max(win.start_line + 1).min(win.total_lines);

        ui.set_total_line_count(win.total_lines as i32);
        self.tab_line_starts.insert(win.path.clone(), starts);
        self.dirty_tabs.insert(win.path.clone());
        self.large_window = Some(win);
        self.viewport_cache = None;
        true
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
        large_file_optimizations: app.get_large_file_optimizations(),
        large_file_soft_limit_mb: app.get_large_file_soft_limit_mb().max(1) as u64,
        large_file_soft_limit_lines: ctrl.session.large_file_soft_limit_lines,
        large_file_hard_limit_mb: app.get_large_file_hard_limit_mb().max(1) as u64,
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

    #[test]
    fn test_window_slice_bounds_and_long_line_truncation() {
        let mut doc = String::new();
        for i in 0..1000 {
            doc.push_str(&format!("line_{:04}: nop\n", i));
        }
        let starts = crate::syntax::intel8080::build_line_starts(&doc);
        let path = PathBuf::from("d:/tmp/huge.asm");
        let (text, state, _) = build_window_slice_for_path(path.clone(), &doc, &starts, 600);
        assert_eq!(state.path, path);
        assert!(!state.read_only);
        assert_eq!(state.start_line, 600 - TEXT_WINDOW_BEFORE_LINES);
        assert_eq!(state.end_line, state.start_line + TEXT_WINDOW_TARGET_LINES);
        assert_eq!(state.total_lines, 1001);
        assert!(text.contains("line_0600: nop"));
        assert!(!text.contains("line_0001: nop"));

        let long_line = "X".repeat(TEXT_WINDOW_MAX_LINE_BYTES + 256);
        let long_starts = vec![0];
        let (trunc_text, trunc_state, _) =
            build_window_slice_for_path(path, &long_line, &long_starts, 0);
        assert!(trunc_state.read_only);
        assert!(trunc_text.ends_with(" ... [truncated]"));
    }
}
