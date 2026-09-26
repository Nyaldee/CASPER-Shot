
use crate::json::{quote, Json};
use std::path::{Path, PathBuf};
use windows_sys::core::GUID;
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::{
    SHGetKnownFolderPath, FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music, FOLDERID_Pictures,
    FOLDERID_Videos, KF_FLAG_DEFAULT,
};

pub(crate) struct State {
    pub(crate) quality: u8,
    pub(crate) hotkey_fullscreen: String,
    pub(crate) hotkey_region: String,
    pub(crate) save_path: String,
    pub(crate) preview_percent: u8,
    pub(crate) preview_duration_ms: u32,
}

impl Default for State {
    fn default() -> Self {
        State {
            quality: 90,
            hotkey_fullscreen: "PrintScreen".to_string(),
            hotkey_region: "Shift+PrintScreen".to_string(),
            save_path: r"%USERPROFILE%\Pictures\Screenshots".to_string(),
            preview_percent: 40,
            preview_duration_ms: 2500,
        }
    }
}

const KNOWN_FOLDERS: &[(&str, GUID)] = &[
    ("Desktop", FOLDERID_Desktop),
    ("Documents", FOLDERID_Documents),
    ("Downloads", FOLDERID_Downloads),
    ("Music", FOLDERID_Music),
    ("Pictures", FOLDERID_Pictures),
    ("Videos", FOLDERID_Videos),
];

fn known_folder(id: &GUID) -> Option<String> {
    unsafe {
        let mut pwstr: *mut u16 = std::ptr::null_mut();
        let hr = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT as u32, std::ptr::null_mut(), &mut pwstr);
        let path = if hr >= 0 && !pwstr.is_null() {
            let len = (0..).take_while(|&i| *pwstr.add(i) != 0).count();
            Some(String::from_utf16_lossy(std::slice::from_raw_parts(pwstr, len)))
        } else {
            None
        };
        CoTaskMemFree(pwstr as *const _);
        path
    }
}

fn known_folder_after_userprofile(rest: &str) -> Option<(String, usize)> {
    let after_sep = rest.strip_prefix(['\\', '/'])?;
    KNOWN_FOLDERS.iter().find_map(|(name, guid)| {
        let head = after_sep.get(..name.len())?;
        let whole_component = matches!(after_sep.as_bytes().get(name.len()), None | Some(b'\\' | b'/'));
        if head.eq_ignore_ascii_case(name) && whole_component {
            known_folder(guid).map(|folder| (folder, 1 + name.len()))
        } else {
            None
        }
    })
}

fn expand_env(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut rest = path;
    while let Some(start) = rest.find('%') {
        let Some(len) = rest[start + 1..].find('%') else { break };
        let name = &rest[start + 1..start + 1 + len];
        let after_var = &rest[start + len + 2..];
        out.push_str(&rest[..start]);
        rest = after_var;

        if name.eq_ignore_ascii_case("USERPROFILE") {
            if let Some((folder, consumed)) = known_folder_after_userprofile(after_var) {
                out.push_str(&folder);
                rest = &after_var[consumed..];
                continue;
            }
        }
        match std::env::var(name) {
            Ok(value) => out.push_str(&value),
            Err(_) => {
                out.push('%');
                out.push_str(name);
                out.push('%');
            }
        }
    }
    out.push_str(rest);
    out
}

pub(crate) fn load(path: &Path) -> State {
    let text = std::fs::read_to_string(path).ok();
    let data = text.as_deref().map(Json::parse);
    let state = match &data {
        Some(Ok(data)) => parse(data),
        _ => State::default(),
    };
    if !matches!(data, Some(Err(_))) {
        let _ = save(path, &state);
    }
    state
}

fn parse(data: &Json) -> State {
    let default = State::default();
    let number = |key: &str, max: f64| data.get(key).and_then(Json::as_f64).map(|n| n.clamp(0.0, max));
    let string = |key: &str| data.get(key).and_then(Json::as_str).map(str::to_string);
    State {
        quality: number("quality", 100.0).map_or(default.quality, |n| n as u8),
        hotkey_fullscreen: string("hotkey_fullscreen").unwrap_or(default.hotkey_fullscreen),
        hotkey_region: string("hotkey_region").unwrap_or(default.hotkey_region),
        save_path: string("save_path").unwrap_or(default.save_path),
        preview_percent: number("preview_percent", 100.0).map_or(default.preview_percent, |n| n as u8),
        preview_duration_ms: number("preview_duration_ms", u32::MAX as f64).map_or(default.preview_duration_ms, |n| n as u32),
    }
}

pub(crate) fn save(path: &Path, state: &State) -> std::io::Result<()> {
    let json = format!(
        "{{\n  \"quality\": {},\n  \"hotkey_fullscreen\": {},\n  \"hotkey_region\": {},\n  \"save_path\": {},\n  \"preview_percent\": {},\n  \"preview_duration_ms\": {}\n}}\n",
        state.quality,
        quote(&state.hotkey_fullscreen),
        quote(&state.hotkey_region),
        quote(&state.save_path),
        state.preview_percent,
        state.preview_duration_ms,
    );
    std::fs::write(path, json)
}

pub(crate) fn exe_dir_join(name: impl AsRef<Path>) -> PathBuf {
    match std::env::current_exe() {
        Ok(exe) => exe.parent().map_or_else(|| name.as_ref().to_path_buf(), |dir| dir.join(&name)),
        Err(_) => name.as_ref().to_path_buf(),
    }
}

pub(crate) fn state_path() -> PathBuf {
    exe_dir_join("state.json")
}

impl State {
    pub(crate) fn resolve_save_path(&self) -> PathBuf {
        let dir = exe_dir_join(expand_env(&self.save_path));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("casper_shot_state_test_{}_{}.json", std::process::id(), name));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn fichier_absent_donne_les_valeurs_par_defaut_et_le_cree() {
        let path = temp_path("missing");
        let state = load(&path);
        assert_eq!(state.quality, 90);
        assert_eq!(state.hotkey_fullscreen, "PrintScreen");
        assert_eq!(state.hotkey_region, "Shift+PrintScreen");
        assert_eq!(state.preview_percent, 40);
        assert_eq!(state.preview_duration_ms, 2500);
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn preview_percent_a_zero_est_accepte() {
        let path = temp_path("preview_zero");
        std::fs::write(&path, r#"{"preview_percent": 0}"#).unwrap();
        assert_eq!(load(&path).preview_percent, 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn round_trip_save_puis_load_preserve_toutes_les_valeurs() {
        let path = temp_path("round_trip");
        let state = State {
            quality: 75,
            hotkey_fullscreen: "Ctrl+Alt+S".to_string(),
            hotkey_region: "Ctrl+Alt+D".to_string(),
            save_path: r#"C:\Users\"Test""#.to_string(),
            preview_percent: 25,
            preview_duration_ms: 1500,
        };
        save(&path, &state).unwrap();

        let reloaded = load(&path);
        assert_eq!(reloaded.quality, 75);
        assert_eq!(reloaded.hotkey_fullscreen, "Ctrl+Alt+S");
        assert_eq!(reloaded.hotkey_region, "Ctrl+Alt+D");
        assert_eq!(reloaded.save_path, state.save_path);
        assert_eq!(reloaded.preview_percent, 25);
        assert_eq!(reloaded.preview_duration_ms, 1500);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn champ_manquant_ou_hors_limites_retombe_sur_le_defaut_ou_est_borne() {
        let path = temp_path("partial");
        std::fs::write(&path, r#"{"quality": 420, "hotkey_region": 3}"#).unwrap();
        let state = load(&path);
        assert_eq!(state.quality, 100);
        assert_eq!(state.hotkey_region, State::default().hotkey_region);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn fichier_invalide_n_est_pas_ecrase() {
        let path = temp_path("invalid");
        let text = r#"{"quality": 50,}"#;
        std::fs::write(&path, text).unwrap();
        assert_eq!(load(&path).quality, 90);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn expand_env_remplace_les_variables_connues() {
        std::env::set_var("CASPER_SHOT_TEST_VAR", r"C:\Users\Test");
        assert_eq!(expand_env(r"%CASPER_SHOT_TEST_VAR%\Pictures"), r"C:\Users\Test\Pictures");
    }

    #[test]
    fn expand_env_laisse_une_variable_inconnue_ou_non_fermee_telle_quelle() {
        assert_eq!(expand_env(r"%CASPER_SHOT_INCONNUE_XYZ%\Pictures"), r"%CASPER_SHOT_INCONNUE_XYZ%\Pictures");
        assert_eq!(expand_env(r"C:\100%\x"), r"C:\100%\x");
    }

    #[test]
    fn userprofile_pictures_suit_une_redirection() {
        let pictures = known_folder(&FOLDERID_Pictures).expect("SHGetKnownFolderPath(FOLDERID_Pictures) a échoué");
        assert_eq!(expand_env(r"%USERPROFILE%\Pictures\Screenshots"), format!("{pictures}\\Screenshots"));
    }

    #[test]
    fn userprofile_picturesfoo_nest_pas_confondu_avec_pictures() {
        let userprofile = std::env::var("USERPROFILE").unwrap();
        assert_eq!(expand_env(r"%USERPROFILE%\Picturesfoo\save"), format!("{userprofile}\\Picturesfoo\\save"));
    }
}
