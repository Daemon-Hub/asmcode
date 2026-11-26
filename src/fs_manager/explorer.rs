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
    pub icon_name: &'static str,
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
        let mut results = Vec::with_capacity(64);
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
        let bytes = fs::read(path_ref)?;
        let has_bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
        let has_cr = contains_cr(if has_bom { &bytes[3..] } else { &bytes[..] });

        let content = if !has_bom && !has_cr {
            match String::from_utf8(bytes) {
                Ok(s) => s,
                Err(err) => String::from_utf8_lossy(err.as_bytes()).into_owned(),
            }
        } else {
            let slice = if has_bom { &bytes[3..] } else { &bytes[..] };
            match std::str::from_utf8(slice) {
                Ok(valid_str) => {
                    if has_cr {
                        normalize_line_endings_fast(valid_str)
                    } else {
                        valid_str.to_string()
                    }
                }
                Err(_) => {
                    let lossy = String::from_utf8_lossy(slice);
                    if contains_cr(lossy.as_bytes()) {
                        normalize_line_endings_fast(&lossy)
                    } else {
                        lossy.into_owned()
                    }
                }
            }
        };
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

        if let Some(ref active) = self.active_file_path
            && let Some(remapped) = remap_path_with_prefix(active, old, &target)
        {
            self.active_file_path = Some(remapped);
        }

        let remapped_expanded: HashSet<PathBuf> = self
            .expanded_folders
            .iter()
            .map(|p| remap_path_with_prefix(p, old, &target).unwrap_or_else(|| p.clone()))
            .collect();
        self.expanded_folders = remapped_expanded;

        Ok(target)
    }
}

pub fn remap_path_with_prefix(path: &Path, old_prefix: &Path, new_prefix: &Path) -> Option<PathBuf> {
    if path == old_prefix {
        return Some(new_prefix.to_path_buf());
    }
    path.strip_prefix(old_prefix)
        .ok()
        .map(|rel| new_prefix.join(rel))
}

#[inline]
fn contains_cr(bytes: &[u8]) -> bool {
    const CR_MASK: u64 = 0x0D0D_0D0D_0D0D_0D0D;
    const LO_BITS: u64 = 0x0101_0101_0101_0101;
    const HI_BITS: u64 = 0x8080_8080_8080_8080;

    let (chunks, rem) = bytes.as_chunks::<8>();
    for &eight in chunks {
        let word = u64::from_ne_bytes(eight);
        let x = word ^ CR_MASK;
        if (x.wrapping_sub(LO_BITS) & !x & HI_BITS) != 0 {
            return true;
        }
    }
    rem.contains(&b'\r')
}

fn normalize_cr_chunk(chunk: &[u8], skip_leading_lf: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(chunk.len());
    let mut i = if skip_leading_lf && chunk.first() == Some(&b'\n') {
        1
    } else {
        0
    };
    while i < chunk.len() {
        let slice = &chunk[i..];
        if let Some(pos) = slice.iter().position(|&b| b == b'\r') {
            out.extend_from_slice(&slice[..pos]);
            out.push(b'\n');
            let after_cr = i + pos + 1;
            if after_cr < chunk.len() && chunk[after_cr] == b'\n' {
                i = after_cr + 1;
            } else {
                i = after_cr;
            }
        } else {
            out.extend_from_slice(slice);
            break;
        }
    }
    out
}

fn normalize_line_endings_fast(input: &str) -> String {
    let bytes = input.as_bytes();
    const PAR_THRESHOLD: usize = 128 * 1024;
    const CHUNK_SIZE: usize = 128 * 1024;

    let out_bytes = if bytes.len() < PAR_THRESHOLD {
        normalize_cr_chunk(bytes, false)
    } else {
        let num_chunks = bytes.len().div_ceil(CHUNK_SIZE);
        let chunk_vecs: Vec<Vec<u8>> = (0..num_chunks)
            .into_par_iter()
            .map(|idx| {
                let start = idx * CHUNK_SIZE;
                let end = (start + CHUNK_SIZE).min(bytes.len());
                let skip_lf = idx > 0 && bytes[start - 1] == b'\r';
                normalize_cr_chunk(&bytes[start..end], skip_lf)
            })
            .collect();
        let total_len: usize = chunk_vecs.iter().map(Vec::len).sum();
        let mut combined = Vec::with_capacity(total_len);
        for v in chunk_vecs {
            combined.extend_from_slice(&v);
        }
        combined
    };

    // SAFETY: input is valid UTF-8 and we only removed ASCII '\r' bytes or replaced them with ASCII '\n'.
    unsafe { String::from_utf8_unchecked(out_bytes) }
}

#[inline]
fn compare_ascii_case_insensitive(a: &str, b: &str) -> std::cmp::Ordering {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let min_len = a_bytes.len().min(b_bytes.len());
    let mut i = 0;
    while i < min_len {
        let ca = a_bytes[i].to_ascii_lowercase();
        let cb = b_bytes[i].to_ascii_lowercase();
        if ca != cb {
            return ca.cmp(&cb);
        }
        i += 1;
    }
    a_bytes.len().cmp(&b_bytes.len())
}

fn collect_tree_entries(
    dir: &Path,
    depth: i32,
    expanded_folders: &HashSet<PathBuf>,
    results: &mut Vec<FsEntry>,
) -> io::Result<()> {
    let mut entries = Vec::with_capacity(32);
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

    // Sort: directories first, then alphabetical (case-insensitive without allocating temporary strings)
    if entries.len() > 64 {
        entries.par_sort_by(|a, b| match (a.2, b.2) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => compare_ascii_case_insensitive(&a.0, &b.0),
        });
    } else {
        entries.sort_by(|a, b| match (a.2, b.2) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => compare_ascii_case_insensitive(&a.0, &b.0),
        });
    }

    for (name, path, is_dir, byte_size) in entries {
        let is_expanded = is_dir && expanded_folders.contains(&path);

        let icon_name: &'static str = if is_dir {
            match name.to_ascii_lowercase().as_str() {
                "src" => "src",
                "ui" => "ui",
                "target" | "build" | "dist" => "target",
                "doc" | "docs" => "docs",
                "test" | "tests" => "test",
                "scripts" => "scripts",
                "tools" => "tools",
                "config" => "config",
                "debug" => "debug",
                "components" => "components",
                _ => "default",
            }
        } else {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            match ext.as_str() {
                "asm" | "inc" | "mac" | "s" => "asm",
                "rs" => "rust",
                "toml" => "toml",
                "md" => "markdown",
                "json" => "json",
                "lock" => "lock",
                "exe" | "com" | "bin" => "exe",
                "bat" | "cmd" | "ps1" => "powershell",
                "svg" | "png" | "jpg" | "jpeg" => "image",
                "c" | "h" => "c",
                "cpp" | "hpp" => "cpp",
                "cs" => "csharp",
                "xml" => "xml",
                "yaml" | "yml" => "yaml",
                "zip" | "tar" | "gz" | "7z" => "zip",
                "pdf" => "pdf",
                "log" => "log",
                "key" => "key",
                "hex" => "hex",
                _ => "document",
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

    #[test]
    fn test_parallel_crlf_normalization_cross_boundary() {
        let mut raw = vec![b'a'; 128 * 1024 - 1];
        raw.push(b'\r');
        raw.push(b'\n');
        raw.extend_from_slice(b"tail\rline\r\nend");
        let input = std::str::from_utf8(&raw).unwrap();
        let normalized = normalize_line_endings_fast(input);
        assert_eq!(normalized, input.replace("\r\n", "\n").replace('\r', "\n"));
    }
}
