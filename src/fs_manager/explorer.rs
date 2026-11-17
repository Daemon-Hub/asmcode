use rayon::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_asm: bool,
    pub is_expanded: bool,
    pub depth: i32,
    pub icon_name: String,
    pub size_formatted: String,
}

pub struct DeletedPathRecord {
    pub original_path: PathBuf,
    pub backup_path: PathBuf,
    pub is_dir: bool,
}

pub struct FileManager {
    pub current_dir: PathBuf,
    pub active_file_path: Option<PathBuf>,
    pub expanded_folders: HashSet<PathBuf>,
    pub deleted_history: Vec<DeletedPathRecord>,
}

impl FileManager {
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Self {
        let current_dir = root_dir.as_ref().to_path_buf();
        Self {
            current_dir,
            active_file_path: None,
            expanded_folders: HashSet::new(),
            deleted_history: Vec::new(),
        }
    }

    pub fn toggle_folder(&mut self, path: &Path) {
        if self.expanded_folders.contains(path) {
            self.expanded_folders.remove(path);
        } else {
            self.expanded_folders.insert(path.to_path_buf());
        }
    }

    pub fn list_directory(&self) -> io::Result<Vec<FsEntry>> {
        let mut results = Vec::new();
        collect_tree_entries(&self.current_dir, 0, &self.expanded_folders, &mut results)?;
        Ok(results)
    }

    pub fn navigate_parent(&mut self) -> bool {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.expanded_folders.clear();
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn navigate_into(&mut self, path: &Path) -> io::Result<()> {
        if path.is_dir() {
            self.current_dir = path.to_path_buf();
            self.expanded_folders.clear();
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Path is not a directory",
            ))
        }
    }

    pub fn read_file_content<P: AsRef<Path>>(&mut self, path: P) -> io::Result<String> {
        let path_ref = path.as_ref();
        let content = fs::read_to_string(path_ref)?;
        let content = content.replace("\r\n", "\n");
        self.active_file_path = Some(path_ref.to_path_buf());
        Ok(content)
    }

    pub fn write_file_content<P: AsRef<Path>>(&mut self, path: P, content: &str) -> io::Result<()> {
        let path_ref = path.as_ref();
        fs::write(path_ref, content)?;
        self.active_file_path = Some(path_ref.to_path_buf());
        Ok(())
    }

    pub fn create_new_file(&mut self, filename: &str) -> io::Result<(PathBuf, String)> {
        let mut clean_name = filename.trim().to_string();
        if clean_name.is_empty() {
            clean_name = "program.asm".to_string();
        }
        if !clean_name.contains('.') {
            clean_name.push_str(".asm");
        }

        let target_path = self.current_dir.join(&clean_name);
        if target_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("File '{}' already exists", clean_name),
            ));
        }

        let initial_template = format!(
            "; ==================================================================\n\
             ; File: {}\n\
             ; Target: Intel 8080 CPU\n\
             ; ==================================================================\n\
             \n\
                     org     0100h       ; CP/M or base TPA address\n\
             \n\
             start:  lxi     sp, 0ffffh  ; Initialize stack pointer\n\
                     mvi     a, 05h      ; Load value 5 into Accumulator\n\
                     mvi     b, 03h      ; Load value 3 into Register B\n\
                     add     b           ; A = A + B (A = 8)\n\
                     sta     result      ; Save result to memory\n\
             \n\
                     hlt                 ; Halt CPU\n\
             \n\
             ; Data segment\n\
             result: db      00h\n\
             \n\
                     end     start\n",
            clean_name
        );

        fs::write(&target_path, &initial_template)?;
        self.active_file_path = Some(target_path.clone());
        Ok((target_path, initial_template))
    }

    pub fn create_new_folder(&mut self, folder_name: &str) -> io::Result<PathBuf> {
        let clean = folder_name.trim();
        let name = if clean.is_empty() {
            "new_folder"
        } else {
            clean
        };
        let target = self.current_dir.join(name);
        fs::create_dir_all(&target)?;
        Ok(target)
    }

    pub fn delete_path<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let p = path.as_ref();
        if !p.exists() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Path not found"));
        }
        let is_dir = p.is_dir();
        let recycle_dir = std::env::temp_dir().join("asmcode_recycle_bin");
        let _ = fs::create_dir_all(&recycle_dir);

        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let file_stem = p.file_name().unwrap_or_default().to_string_lossy();
        let backup_name = format!("{}_{}", unique_id, file_stem);
        let backup_path = recycle_dir.join(backup_name);

        if fs::rename(p, &backup_path).is_err() {
            if is_dir {
                copy_dir_all(p, &backup_path)?;
                fs::remove_dir_all(p)?;
            } else {
                fs::copy(p, &backup_path)?;
                fs::remove_file(p)?;
            }
        }

        self.deleted_history.push(DeletedPathRecord {
            original_path: p.to_path_buf(),
            backup_path,
            is_dir,
        });

        if self
            .active_file_path
            .as_ref()
            .is_some_and(|active| active == p || active.starts_with(p))
        {
            self.active_file_path = None;
        }
        Ok(())
    }

    pub fn undo_delete(&mut self) -> io::Result<Option<PathBuf>> {
        let Some(record) = self.deleted_history.pop() else {
            return Ok(None);
        };
        if let Some(parent) = record.original_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::rename(&record.backup_path, &record.original_path).is_err() {
            if record.is_dir {
                copy_dir_all(&record.backup_path, &record.original_path)?;
                let _ = fs::remove_dir_all(&record.backup_path);
            } else {
                fs::copy(&record.backup_path, &record.original_path)?;
                let _ = fs::remove_file(&record.backup_path);
            }
        }
        Ok(Some(record.original_path))
    }

    pub fn expand_parents_of(&mut self, path: &Path) {
        let mut curr = path.parent();
        while let Some(p) = curr {
            if p.starts_with(&self.current_dir) {
                self.expanded_folders.insert(p.to_path_buf());
            }
            curr = p.parent();
        }
    }

    pub fn rename_path<P: AsRef<Path>>(
        &mut self,
        old_path: P,
        new_name: &str,
    ) -> io::Result<PathBuf> {
        let old = old_path.as_ref();
        let clean = new_name.trim();
        if clean.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Empty name"));
        }
        let parent = old.parent().unwrap_or(&self.current_dir);
        let target = parent.join(clean);
        fs::rename(old, &target)?;
        if self
            .active_file_path
            .as_ref()
            .is_some_and(|active| active == old)
        {
            self.active_file_path = Some(target.clone());
        }
        Ok(target)
    }
}

fn collect_tree_entries(
    dir: &Path,
    depth: i32,
    expanded_folders: &HashSet<PathBuf>,
    results: &mut Vec<FsEntry>,
) -> io::Result<()> {
    let mut entries = Vec::new();
    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(()),
    };

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let is_dir = metadata.is_dir();
        entries.push((name, path, is_dir, metadata.len()));
    }

    // Sort: directories first, then alphabetical (parallel acceleration via Rayon for large folders)
    if entries.len() > 64 {
        entries.par_sort_by(|a, b| match (a.2, b.2) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
        });
    } else {
        entries.sort_by(|a, b| match (a.2, b.2) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
        });
    }

    for (name, path, is_dir, byte_size) in entries {
        let is_expanded = is_dir && expanded_folders.contains(&path);

        let icon_name = if is_dir {
            match name.to_ascii_lowercase().as_str() {
                "src" => "src".to_string(),
                "ui" => "ui".to_string(),
                "target" | "build" | "dist" => "target".to_string(),
                "doc" | "docs" => "docs".to_string(),
                "test" | "tests" => "test".to_string(),
                "scripts" => "scripts".to_string(),
                "tools" => "tools".to_string(),
                "config" => "config".to_string(),
                "debug" => "debug".to_string(),
                "components" => "components".to_string(),
                _ => "default".to_string(),
            }
        } else {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            match ext.as_str() {
                "asm" | "inc" | "mac" | "s" => "asm".to_string(),
                "rs" => "rust".to_string(),
                "toml" => "toml".to_string(),
                "md" => "markdown".to_string(),
                "json" => "json".to_string(),
                "lock" => "lock".to_string(),
                "exe" | "com" | "bin" => "exe".to_string(),
                "bat" | "cmd" | "ps1" => "powershell".to_string(),
                "svg" | "png" | "jpg" | "jpeg" => "image".to_string(),
                "c" | "h" => "c".to_string(),
                "cpp" | "hpp" => "cpp".to_string(),
                "cs" => "csharp".to_string(),
                "xml" => "xml".to_string(),
                "yaml" | "yml" => "yaml".to_string(),
                "zip" | "tar" | "gz" | "7z" => "zip".to_string(),
                "pdf" => "pdf".to_string(),
                "log" => "log".to_string(),
                "key" => "key".to_string(),
                "hex" => "hex".to_string(),
                _ => "document".to_string(),
            }
        };

        let is_asm = icon_name == "asm";
        let size_formatted = if is_dir {
            "DIR".to_string()
        } else if byte_size < 1024 {
            format!("{} B", byte_size)
        } else if byte_size < 1024 * 1024 {
            format!("{:.1} KB", byte_size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", byte_size as f64 / (1024.0 * 1024.0))
        };

        results.push(FsEntry {
            name,
            path: path.clone(),
            is_dir,
            is_asm,
            is_expanded,
            depth,
            icon_name,
            size_formatted,
        });

        if is_expanded {
            let _ = collect_tree_entries(&path, depth + 1, expanded_folders, results);
        }
    }

    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_child = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_child)?;
        } else {
            fs::copy(entry.path(), dest_child)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_and_undo_delete() {
        let temp_dir = std::env::temp_dir().join("asmcode_test_undo_delete");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let file_path = temp_dir.join("test_file.asm");
        fs::write(&file_path, "mvi a, 10h\nret\n").unwrap();
        assert!(file_path.exists());

        let mut fm = FileManager::new(&temp_dir);
        fm.delete_path(&file_path).unwrap();
        assert!(!file_path.exists());
        assert_eq!(fm.deleted_history.len(), 1);

        let restored = fm.undo_delete().unwrap();
        assert_eq!(restored, Some(file_path.clone()));
        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "mvi a, 10h\nret\n");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
