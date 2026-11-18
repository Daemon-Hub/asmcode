use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn is_asm_associated() -> bool {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("reg");
        cmd.args(["query", "HKCU\\Software\\Classes\\.asm", "/ve"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = cmd.output() {
            return output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains("ASMCode.asm");
        }
        false
    }
    #[cfg(target_os = "linux")]
    {
        let mut cmd = Command::new("xdg-mime");
        cmd.args(["query", "default", "text/x-asm"]);
        if let Ok(output) = cmd.output() {
            return output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains("asmcode");
        }
        false
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        false
    }
}

pub fn set_asm_association(enable: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        if enable {
            let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let exe_path = current_exe.to_string_lossy().to_string();

            let mut cmd = Command::new("reg");
            cmd.args([
                "add",
                "HKCU\\Software\\Classes\\.asm",
                "/ve",
                "/d",
                "ASMCode.asm",
                "/f",
            ]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let status = cmd.status().map_err(|e| e.to_string())?;
            if !status.success() {
                return Err("Failed to register .asm extension in HKCU".to_string());
            }

            let mut cmd = Command::new("reg");
            cmd.args([
                "add",
                "HKCU\\Software\\Classes\\ASMCode.asm",
                "/ve",
                "/d",
                "ASM Code Intel 8080 Source File",
                "/f",
            ]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.status();

            let mut cmd = Command::new("reg");
            cmd.args([
                "add",
                "HKCU\\Software\\Classes\\Applications\\asmcode.exe",
                "/v",
                "FriendlyAppName",
                "/d",
                "ASM Code",
                "/f",
            ]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.status();

            let mut cmd = Command::new("reg");
            let icon_val = format!("\"{}\",0", exe_path);
            cmd.args([
                "add",
                "HKCU\\Software\\Classes\\ASMCode.asm\\DefaultIcon",
                "/ve",
                "/d",
                &icon_val,
                "/f",
            ]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.status();

            let mut cmd = Command::new("reg");
            let open_val = format!("\"{}\" \"%1\"", exe_path);
            cmd.args([
                "add",
                "HKCU\\Software\\Classes\\ASMCode.asm\\shell\\open\\command",
                "/ve",
                "/d",
                &open_val,
                "/f",
            ]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.status();

            let mut cmd = Command::new("reg");
            cmd.args([
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\.asm\\OpenWithProgids",
                "/v",
                "ASMCode.asm",
                "/t",
                "REG_SZ",
                "/d",
                "",
                "/f",
            ]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.status();

            notify_shell_change();
            Ok(())
        } else {
            let mut cmd = Command::new("reg");
            cmd.args(["delete", "HKCU\\Software\\Classes\\.asm", "/f"]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.status();

            let mut cmd = Command::new("reg");
            cmd.args(["delete", "HKCU\\Software\\Classes\\ASMCode.asm", "/f"]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.status();

            let mut cmd = Command::new("reg");
            cmd.args([
                "delete",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\.asm\\OpenWithProgids",
                "/v",
                "ASMCode.asm",
                "/f",
            ]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.status();

            notify_shell_change();
            Ok(())
        }
    }

    #[cfg(target_os = "linux")]
    {
        if enable {
            if let Some(apps_dir) = dirs::data_local_dir().map(|d| d.join("applications")) {
                let _ = std::fs::create_dir_all(&apps_dir);
                let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
                let desktop_entry = format!(
                    "[Desktop Entry]\nType=Application\nName=ASM Code\nExec=\"{}\" %f\nIcon=text-x-generic\nTerminal=false\nCategories=Development;IDE;\nMimeType=text/x-asm;\n",
                    current_exe.display()
                );
                let desktop_file = apps_dir.join("asmcode.desktop");
                std::fs::write(&desktop_file, desktop_entry).map_err(|e| e.to_string())?;

                let _ = Command::new("xdg-mime")
                    .args(["default", "asmcode.desktop", "text/x-asm"])
                    .status();
            }
            Ok(())
        } else {
            let _ = Command::new("xdg-mime")
                .args(["default", "", "text/x-asm"])
                .status();
            Ok(())
        }
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Ok(())
    }
}

#[cfg(windows)]
fn notify_shell_change() {
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "$c = '[DllImport(\"shell32.dll\")]public static extern void SHChangeNotify(int e, int f, System.IntPtr i1, System.IntPtr i2);'; Add-Type -TypeDefinition $c -Name S -Namespace W; [W.S]::SHChangeNotify(0x08000000, 0, [System.IntPtr]::Zero, [System.IntPtr]::Zero)",
    ]);
    cmd.creation_flags(CREATE_NO_WINDOW);
    let _ = cmd.status();
}
