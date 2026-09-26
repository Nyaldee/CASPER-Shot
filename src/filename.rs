
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetAncestor, GetCursorPos, GetWindowTextW, WindowFromPoint, GA_ROOT};

const FALLBACK_NAME: &str = "Desktop";
const MAX_TITLE_CHARS: usize = 60;

pub(crate) fn window_title_under_cursor() -> String {
    let title = unsafe {
        let mut cursor = POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor);
        let hwnd = WindowFromPoint(cursor);
        let root = GetAncestor(hwnd, GA_ROOT);
        let target = if root.is_null() { hwnd } else { root };
        let mut buf = [0u16; 256];
        let len = if target.is_null() { 0 } else { GetWindowTextW(target, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize };
        String::from_utf16_lossy(&buf[..len])
    };
    sanitize_component(&title)
}

fn sanitize_component(s: &str) -> String {
    let cleaned: String = s
        .trim()
        .chars()
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { '_' } else { c })
        .take(MAX_TITLE_CHARS)
        .collect();
    if cleaned.trim().is_empty() { FALLBACK_NAME.to_string() } else { cleaned }
}

pub(crate) fn unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let ext = path.extension().unwrap_or_default().to_string_lossy().into_owned();
    let parent = path.parent().unwrap_or(Path::new(""));
    (1u32..).map(|n| parent.join(format!("{stem} ({n}).{ext}"))).find(|candidate| !candidate.exists()).unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("casper_shot_filename_test_{}_{}", std::process::id(), name))
    }

    #[test]
    fn sanitize_remplace_les_caracteres_interdits() {
        assert_eq!(sanitize_component(r#"C:\Users\Test\file<>:"/\|?*.txt"#), "C__Users_Test_file_________.txt");
    }

    #[test]
    fn sanitize_tronque_le_titre() {
        assert_eq!(sanitize_component(&"é".repeat(200)).chars().count(), MAX_TITLE_CHARS);
    }

    #[test]
    fn sanitize_titre_vide_ou_blanc_donne_desktop() {
        assert_eq!(sanitize_component(""), FALLBACK_NAME);
        assert_eq!(sanitize_component("   "), FALLBACK_NAME);
    }

    #[test]
    fn unique_path_sans_conflit_renvoie_le_chemin_tel_quel() {
        let path = temp_path("no_conflict.webp");
        let _ = std::fs::remove_file(&path);
        assert_eq!(unique_path(path.clone()), path);
    }

    #[test]
    fn unique_path_incremente_sur_conflit() {
        let base = temp_path("conflict.webp");
        let first = temp_path("conflict (1).webp");
        let second = temp_path("conflict (2).webp");
        let _ = std::fs::remove_file(&first);
        let _ = std::fs::remove_file(&second);
        std::fs::write(&base, b"x").unwrap();
        assert_eq!(unique_path(base.clone()), first);

        std::fs::write(&first, b"x").unwrap();
        assert_eq!(unique_path(base.clone()), second);

        let _ = std::fs::remove_file(&base);
        let _ = std::fs::remove_file(&first);
    }
}
