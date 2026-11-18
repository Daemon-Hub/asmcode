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
            .as_ref(),
    ));

    match controller.file_manager.list_directory() {
        Ok(entries) => {
            let mut items = Vec::with_capacity(entries.len());
            for e in entries {
                items.push(FileItem {
                    name: SharedString::from(e.name),
                    path: SharedString::from(e.path.to_string_lossy().as_ref()),
                    is_dir: e.is_dir,
                    is_asm: e.is_asm,
                    is_expanded: e.is_expanded,
                    depth: e.depth,
                    icon_name: SharedString::from(e.icon_name),
                    size_str: SharedString::from(e.size_formatted),
                });
            }
            app.set_file_list(ModelRc::from(Rc::new(VecModel::from(items))));
        }
        Err(err) => {
            eprintln!("Failed to read directory: {}", err);
        }
    }
}
