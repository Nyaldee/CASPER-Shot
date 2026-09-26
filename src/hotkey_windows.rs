
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, VK_F1, VK_SNAPSHOT,
};

pub(crate) const ID_FULLSCREEN: i32 = 1;
pub(crate) const ID_REGION: i32 = 2;

fn parse(spec: &str) -> Option<(u32, u32)> {
    let mut modifiers = 0;
    let mut key = None;
    for part in spec.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_uppercase().as_str() {
            "CTRL" | "CONTROL" => modifiers |= MOD_CONTROL,
            "ALT" => modifiers |= MOD_ALT,
            "SHIFT" => modifiers |= MOD_SHIFT,
            "WIN" | "WINDOWS" => modifiers |= MOD_WIN,
            other if key.is_none() => key = Some(key_code(other)?),
            _ => return None,
        }
    }
    Some((modifiers, key?))
}

fn key_code(name: &str) -> Option<u32> {
    if matches!(name, "PRINTSCREEN" | "PRTSC" | "PRTSCN" | "IMPR" | "IMPRECR") {
        return Some(VK_SNAPSHOT as u32);
    }
    if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
        return (1..=12).contains(&n).then(|| VK_F1 as u32 + n - 1);
    }
    match name.as_bytes() {
        &[c] if c.is_ascii_alphanumeric() => Some(c as u32),
        _ => None,
    }
}

pub(crate) fn register(fullscreen: &str, region: &str) -> bool {
    let (Some((fs_mods, fs_key)), Some((rg_mods, rg_key))) = (parse(fullscreen), parse(region)) else { return false };
    unsafe {
        if RegisterHotKey(std::ptr::null_mut(), ID_FULLSCREEN, fs_mods | MOD_NOREPEAT, fs_key) == 0 {
            return false;
        }
        if RegisterHotKey(std::ptr::null_mut(), ID_REGION, rg_mods | MOD_NOREPEAT, rg_key) == 0 {
            UnregisterHotKey(std::ptr::null_mut(), ID_FULLSCREEN);
            return false;
        }
    }
    true
}

pub(crate) fn unregister() {
    unsafe {
        UnregisterHotKey(std::ptr::null_mut(), ID_FULLSCREEN);
        UnregisterHotKey(std::ptr::null_mut(), ID_REGION);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reconnait_modificateurs_et_touche() {
        assert_eq!(parse("Ctrl+Alt+S"), Some((MOD_CONTROL | MOD_ALT, b'S' as u32)));
        assert_eq!(parse("win+shift+F5"), Some((MOD_WIN | MOD_SHIFT, 0x74)));
        assert_eq!(parse("D"), Some((0, b'D' as u32)));
        assert_eq!(parse("PrintScreen"), Some((0, 0x2C)));
        assert_eq!(parse("Shift+PrintScreen"), Some((MOD_SHIFT, 0x2C)));
    }

    #[test]
    fn parse_refuse_le_texte_invalide() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("Ctrl+Alt"), None);
        assert_eq!(parse("Ctrl+F13"), None);
        assert_eq!(parse("Ctrl+AB"), None);
        assert_eq!(parse("Ctrl+A+B"), None);
        assert_eq!(parse("FF5"), None);
    }
}
