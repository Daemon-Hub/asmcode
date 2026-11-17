use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct AppSession {
    pub last_open_folder: Option<String>,
    pub last_open_file: Option<String>,
    pub editor_font_size: f32,
    pub editor_font_italic: bool,
    pub current_language: String,
    pub labels_expanded: bool,
    pub terminal_expanded: bool,
    pub sidebar_width: f32,
    pub terminal_height: f32,
}

impl Default for AppSession {
    fn default() -> Self {
        Self {
            last_open_folder: None,
            last_open_file: None,
            editor_font_size: 18.0,
            editor_font_italic: true,
            current_language: "ru".to_string(),
            labels_expanded: true,
            terminal_expanded: false,
            sidebar_width: 320.0,
            terminal_height: 180.0,
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
                    if let Ok(sz) = val.parse::<f32>() {
                        if (10.0..=40.0).contains(&sz) {
                            session.editor_font_size = sz;
                        }
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
                    if let Ok(w) = val.parse::<f32>() {
                        if (180.0..=700.0).contains(&w) {
                            session.sidebar_width = w;
                        }
                    }
                }
                "terminal_height" => {
                    if let Ok(h) = val.parse::<f32>() {
                        if (70.0..=600.0).contains(&h) {
                            session.terminal_height = h;
                        }
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

    let json_text = format!(
        "{{\n  \"last_open_folder\": {},\n  \"last_open_file\": {},\n  \"editor_font_size\": {},\n  \"editor_font_italic\": {},\n  \"current_language\": \"{}\",\n  \"labels_expanded\": {},\n  \"terminal_expanded\": {},\n  \"sidebar_width\": {},\n  \"terminal_height\": {}\n}}\n",
        folder_str,
        file_str,
        session.editor_font_size,
        session.editor_font_italic,
        session.current_language,
        session.labels_expanded,
        session.terminal_expanded,
        session.sidebar_width,
        session.terminal_height
    );

    let file_path = dir.join("session.json");
    let _ = fs::write(file_path, json_text);
}
