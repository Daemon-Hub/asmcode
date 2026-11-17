use crate::autocomplete::engine::{AutocompleteEngine, Suggestion};
use crate::editor::undo::UndoManager;
use crate::fs_manager::explorer::FileManager;
use crate::session::{self, AppSession};
use crate::ui::AppWindow;
use std::path::Path;

pub struct IdeController {
    pub file_manager: FileManager,
    pub autocomplete: AutocompleteEngine,
    pub active_suggestions: Vec<Suggestion>,
    pub logs: Vec<String>,
    pub undo_manager: UndoManager,
}

impl IdeController {
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Self {
        Self {
            file_manager: FileManager::new(root_dir),
            autocomplete: AutocompleteEngine::new(),
            active_suggestions: Vec::new(),
            logs: vec![
                "[SYSTEM] ASMCode Intel 8080 IDE ready.".to_string(),
                "[TOOLCHAIN] Compiler: C:\\bin\\asmx20.exe".to_string(),
                "[TOOLCHAIN] Emulator: C:\\bin\\cpmemu.exe".to_string(),
            ],
            undo_manager: UndoManager::new(String::new(), 0),
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
    let session = AppSession {
        last_open_folder: Some(ctrl.file_manager.current_dir.to_string_lossy().to_string()),
        last_open_file: ctrl
            .file_manager
            .active_file_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        editor_font_size: app.get_editor_font_size(),
        editor_font_italic: app.get_editor_font_italic(),
        current_language: app.get_current_language().to_string(),
        labels_expanded: app.get_labels_expanded(),
        terminal_expanded: app.get_terminal_expanded(),
        sidebar_width: app.get_sidebar_width(),
        terminal_height: app.get_terminal_height(),
    };
    session::save_session(&session);
}
