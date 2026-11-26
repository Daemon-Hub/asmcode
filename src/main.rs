#![cfg_attr(windows, windows_subsystem = "windows")]

mod autocomplete;
mod compiler;
mod docs;
mod editor;
mod fs_manager;
mod session;
mod syntax;
mod terminal;
mod ui;

use slint::ComponentHandle;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use ui::{AppWindow, IdeController, WinitEventHub, setup_ui};

fn maximize_window(app: &AppWindow) {
    #[cfg(windows)]
    {
        use slint::winit_030::WinitWindowAccessor;
        use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        app.window().with_winit_window(|winit_win| {
            if let Ok(handle) = winit_win.window_handle()
                && let RawWindowHandle::Win32(win32) = handle.as_raw()
            {
                let hwnd = win32.hwnd.get();
                unsafe extern "system" {
                    fn ShowWindow(hWnd: isize, nCmdShow: i32) -> i32;
                }
                const SW_MAXIMIZE: i32 = 3;
                unsafe {
                    ShowWindow(hwnd, SW_MAXIMIZE);
                }
            }
            winit_win.set_maximized(true);
        });
    }
    #[cfg(not(windows))]
    {
        use slint::winit_030::WinitWindowAccessor;
        app.window().with_winit_window(|winit_win| {
            winit_win.set_maximized(true);
        });
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let mut args = env::args().skip(1);
    let cli_file_arg = args.next().map(PathBuf::from);

    let saved_session = session::load_session();
    let current_dir = if let Some(ref file_path) = cli_file_arg {
        if let Some(parent) = file_path.parent() {
            if parent.exists() && parent.is_dir() {
                parent.to_path_buf()
            } else {
                file_path.clone()
            }
        } else {
            PathBuf::from(".")
        }
    } else if let Some(ref folder) = saved_session.last_open_folder {
        PathBuf::from(folder)
    } else {
        env::current_dir()
            .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
    };

    let controller = Arc::new(Mutex::new(IdeController::new(current_dir.clone())));

    let app = AppWindow::new()?;
    let winit_hub = WinitEventHub::default();
    setup_ui(&app, Arc::clone(&controller), saved_session, &winit_hub);
    let _terminal_bridge = terminal::TerminalUiBridge::setup(&app, current_dir, &winit_hub);
    winit_hub.install(app.window());

    maximize_window(&app);

    let app_weak1 = app.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(50), move || {
        if let Some(ui) = app_weak1.upgrade() {
            maximize_window(&ui);
        }
    });

    let app_weak2 = app.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(150), move || {
        if let Some(ui) = app_weak2.upgrade() {
            maximize_window(&ui);
        }
    });

    if let Some(file_path) = cli_file_arg.filter(|p| p.is_file()) {
        ui::handlers::open_file_internal(&app, &controller, &file_path, false);
    }

    let ctrl_close = Arc::clone(&controller);
    let app_weak_close = app.as_weak();
    app.window().on_close_requested(move || {
        if let Some(ui) = app_weak_close.upgrade() {
            let ctrl = ctrl_close.lock().unwrap();
            ui::controller::persist_current_session(&ui, &ctrl);
        }
        slint::CloseRequestResponse::HideWindow
    });

    app.run()
}


