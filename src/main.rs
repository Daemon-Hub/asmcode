#![cfg_attr(windows, windows_subsystem = "windows")]

mod autocomplete;
mod compiler;
mod editor;
mod fs_manager;
mod session;
mod syntax;
mod ui;

use slint::ComponentHandle;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use ui::{AppWindow, IdeController, setup_ui};

fn main() -> Result<(), slint::PlatformError> {
    let saved_session = session::load_session();
    let current_dir = if let Some(ref folder) = saved_session.last_open_folder {
        PathBuf::from(folder)
    } else {
        env::current_dir()
            .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
    };

    let controller = Arc::new(Mutex::new(IdeController::new(current_dir)));

    let app = AppWindow::new()?;
    setup_ui(&app, controller, saved_session);

    app.run()
}
