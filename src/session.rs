use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct AppSession {
    pub last_open_folder: Option<String>,
    pub last_open_file: Option<String>,
    pub open_tabs: Vec<String>,
    pub editor_font_size: f32,
    pub editor_font_italic: bool,
    pub current_language: String,
    pub labels_expanded: bool,
    pub terminal_expanded: bool,
    pub sidebar_width: f32,
    pub terminal_height: f32,
    pub active_theme_id: i32,
    pub cursor_blink: bool,
    pub tab_size: i32,
    pub compiler_path: String,
    pub emulator_path: String,
    pub toolchain_type: i32, // 0 = Default (intel-8080-kit built-in), 1 = External binary (asmx20 + cpmemu)
    pub large_file_optimizations: bool,
    pub large_file_soft_limit_mb: u64,
    pub large_file_soft_limit_lines: usize,
    pub large_file_hard_limit_mb: u64,
}

impl Default for AppSession {
    fn default() -> Self {
        Self {
            last_open_folder: None,
            last_open_file: None,
            open_tabs: Vec::new(),
            editor_font_size: 18.0,
            editor_font_italic: false,
            current_language: "ru".to_string(),
            labels_expanded: true,
            terminal_expanded: false,
            sidebar_width: 320.0,
            terminal_height: 180.0,
            active_theme_id: 1, // Cyberpunk 2077 Red by default
            cursor_blink: true,
            tab_size: 4,
            compiler_path: "C:\\bin\\asmx20.exe".to_string(),
            emulator_path: "C:\\bin\\cpmemu.exe".to_string(),
            toolchain_type: 0,
            large_file_optimizations: true,
            large_file_soft_limit_mb: 20,
            large_file_soft_limit_lines: 300_000,
            large_file_hard_limit_mb: 500,
        }
    }
}

pub fn get_cache_dir() -> PathBuf {
    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        PathBuf::from(local_app_data).join("ASMCode")
    } else if let Ok(user_profile) = env::var("USERPROFILE") {
        PathBuf::from(user_profile)
            .join("AppData")
            .join("Local")
            .join("ASMCode")
    } else {
        PathBuf::from(".").join(".asmcode_cache")
    }
}

pub fn load_session() -> AppSession {
    let file_path = get_cache_dir().join("session.json");
    if !file_path.exists() {
        return AppSession::default();
    }

    let content = match fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(_) => return AppSession::default(),
    };

    let mut session = AppSession::default();

    for line in content.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().trim_matches('"');
            let val = v.trim();

            match key {
                "last_open_folder" => {
                    let s = val.trim_matches('"');
                    if !s.is_empty() && s != "null" && Path::new(s).exists() {
                        session.last_open_folder = Some(s.to_string());
                    }
                }
                "last_open_file" => {
                    let s = val.trim_matches('"');
                    if !s.is_empty() && s != "null" && Path::new(s).exists() {
                        session.last_open_file = Some(s.to_string());
                    }
                }
                "editor_font_size" => {
                    if let Some(sz) = val
                        .parse::<f32>()
                        .ok()
                        .filter(|sz| (10.0..=40.0).contains(sz))
                    {
                        session.editor_font_size = sz;
                    }
                }
                "editor_font_italic" => {
                    session.editor_font_italic = val == "true";
                }
                "current_language" => {
                    let s = val.trim_matches('"');
                    if s == "ru" || s == "en" {
                        session.current_language = s.to_string();
                    }
                }
                "labels_expanded" => {
                    session.labels_expanded = val == "true";
                }
                "terminal_expanded" => {
                    session.terminal_expanded = val == "true";
                }
                "sidebar_width" => {
                    if let Some(w) = val
                        .parse::<f32>()
                        .ok()
                        .filter(|w| (180.0..=700.0).contains(w))
                    {
                        session.sidebar_width = w;
                    }
                }
                "terminal_height" => {
                    if let Some(h) = val
                        .parse::<f32>()
                        .ok()
                        .filter(|h| (70.0..=600.0).contains(h))
                    {
                        session.terminal_height = h;
                    }
                }
                "active_theme_id" => {
                    if let Ok(tid) = val.parse::<i32>() {
                        session.active_theme_id = tid;
                    }
                }
                "large_file_optimizations" => {
                    session.large_file_optimizations = val == "true";
                }
                "large_file_soft_limit_mb" => {
                    if let Ok(v) = val.parse::<u64>() {
                        session.large_file_soft_limit_mb = v;
                    }
                }
                "large_file_soft_limit_lines" => {
                    if let Ok(v) = val.parse::<usize>() {
                        session.large_file_soft_limit_lines = v;
                    }
                }
                "open_tabs" => {
                    let cleaned = val.trim_start_matches('[').trim_end_matches(']');
                    let mut tabs = Vec::new();
                    for item in cleaned.split(',') {
                        let path_str = item.trim().trim_matches('"');
                        if !path_str.is_empty() {
                            tabs.push(path_str.to_string());
                        }
                    }
                    session.open_tabs = tabs;
                }
                "cursor_blink" => {
                    session.cursor_blink = val != "false";
                }
                "tab_size" => {
                    if let Ok(v) = val.parse::<i32>() {
                        session.tab_size = v;
                    }
                }
                "compiler_path" => {
                    let s = val.trim_matches('"');
                    if !s.is_empty() {
                        session.compiler_path = s.to_string();
                    }
                }
                "emulator_path" => {
                    let s = val.trim_matches('"');
                    if !s.is_empty() {
                        session.emulator_path = s.to_string();
                    }
                }
                "toolchain_type" => {
                    if let Ok(v) = val.parse::<i32>() {
                        session.toolchain_type = v;
                    }
                }
                "large_file_hard_limit_mb" => {
                    if let Ok(v) = val.parse::<u64>() {
                        session.large_file_hard_limit_mb = v;
                    }
                }
                _ => {}
            }
        }
    }

    session
}

pub fn save_session(session: &AppSession) {
    let dir = get_cache_dir();
    let _ = fs::create_dir_all(&dir);

    let folder_str = match &session.last_open_folder {
        Some(f) => format!("\"{}\"", f.replace('\\', "/")),
        None => "null".to_string(),
    };

    let file_str = match &session.last_open_file {
        Some(f) => format!("\"{}\"", f.replace('\\', "/")),
        None => "null".to_string(),
    };

    let tabs_str = session
        .open_tabs
        .iter()
        .map(|t| format!("\"{}\"", t.replace('\\', "/")))
        .collect::<Vec<String>>()
        .join(", ");

    let json_text = format!(
        "{{\n  \"last_open_folder\": {},\n  \"last_open_file\": {},\n  \"open_tabs\": [{}],\n  \"editor_font_size\": {},\n  \"editor_font_italic\": {},\n  \"current_language\": \"{}\",\n  \"labels_expanded\": {},\n  \"terminal_expanded\": {},\n  \"sidebar_width\": {},\n  \"terminal_height\": {},\n  \"active_theme_id\": {},\n  \"cursor_blink\": {},\n  \"tab_size\": {},\n  \"compiler_path\": \"{}\",\n  \"emulator_path\": \"{}\",\n  \"toolchain_type\": {},\n  \"large_file_optimizations\": {},\n  \"large_file_soft_limit_mb\": {},\n  \"large_file_soft_limit_lines\": {},\n  \"large_file_hard_limit_mb\": {}\n}}\n",
        folder_str,
        file_str,
        tabs_str,
        session.editor_font_size,
        session.editor_font_italic,
        session.current_language,
        session.labels_expanded,
        session.terminal_expanded,
        session.sidebar_width,
        session.terminal_height,
        session.active_theme_id,
        session.cursor_blink,
        session.tab_size,
        session.compiler_path.replace('\\', "/"),
        session.emulator_path.replace('\\', "/"),
        session.toolchain_type,
        session.large_file_optimizations,
        session.large_file_soft_limit_mb,
        session.large_file_soft_limit_lines,
        session.large_file_hard_limit_mb
    );

    let file_path = dir.join("session.json");
    let _ = fs::write(file_path, json_text);
}
