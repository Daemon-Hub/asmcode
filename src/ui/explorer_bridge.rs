use crate::ui::controller::IdeController;
use crate::ui::{AppWindow, FileItem};
use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub fn refresh_files_ui(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
    let controller = controller_rc.lock().unwrap();
    app.set_current_dir_path(SharedString::from(
        controller
            .file_manager
            .current_dir
            .to_string_lossy()
            .to_string(),
    ));

    match controller.file_manager.list_directory() {
        Ok(entries) => {
            let items: Vec<FileItem> = entries
                .into_iter()
                .map(|e| FileItem {
                    name: SharedString::from(e.name),
                    path: SharedString::from(e.path.to_string_lossy().to_string()),
                    is_dir: e.is_dir,
                    is_asm: e.is_asm,
                    is_expanded: e.is_expanded,
                    depth: e.depth,
                    icon_name: SharedString::from(e.icon_name),
                    size_str: SharedString::from(e.size_formatted),
                })
                .collect();
            app.set_file_list(ModelRc::from(Rc::new(VecModel::from(items))));
        }
        Err(err) => {
            eprintln!("Failed to read directory: {}", err);
        }
    }
}
