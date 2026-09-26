
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture_windows;
mod clipboard_windows;
mod filename;
mod hotkey_windows;
mod json;
mod overlay_windows;
mod popup_menu;
mod state;
mod toast_windows;
mod tray_windows;
mod webp;
mod win_util;

use capture_windows::Capture;
use state::State;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use tray_windows::{TrayEvent, TrayIcon};
use webp::Webp;
use win_util::{shell_execute, shell_open, wide};
use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, SYSTEMTIME};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MessageBoxW, TranslateMessage, MB_ICONERROR, MB_OK, MSG, WM_HOTKEY,
};

const GITHUB_URL: &str = "https://github.com/Nyaldee/CASPER-Shot";

fn acquire_single_instance_lock() -> bool {
    let name = wide("CasperShotSingleInstance");
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    !handle.is_null() && unsafe { GetLastError() } != ERROR_ALREADY_EXISTS
}

fn output_path(state: &State) -> PathBuf {
    let mut now: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut now) };
    let month_dir = state.resolve_save_path().join(format!("{}-{:02}", now.wYear, now.wMonth));
    let _ = std::fs::create_dir_all(&month_dir);
    let name = format!("{}_{:02}-{:02}-{}_q{}.webp", filename::window_title_under_cursor(), now.wMonth, now.wDay, now.wYear, state.quality);
    filename::unique_path(month_dir.join(name))
}

fn save(capture: Capture, webp: &Webp, state: &State) {
    clipboard_windows::copy_bitmap(&capture);
    let path = output_path(state);
    let saved = webp.encode(&capture, state.quality).and_then(|encoded| std::fs::write(&path, encoded).ok()).map(|()| path);
    toast_windows::show(capture, saved, state.preview_percent, state.preview_duration_ms);
}

fn capture_fullscreen(webp: &Webp, state: &State) {
    if let Some(capture) = capture_windows::capture_screen_under_cursor() {
        save(capture, webp, state);
    }
}

fn capture_region(webp: &Webp, state: &State) {
    let Some(capture) = capture_windows::capture_virtual_screen() else { return };
    let Some(r) = overlay_windows::pick_region(&capture) else { return };
    save(capture.crop(r.left, r.top, r.right - r.left, r.bottom - r.top), webp, state);
}

fn open_settings() {
    let path = state::state_path();
    if !shell_open(&path) {
        let mut quoted = std::ffi::OsString::from("\"");
        quoted.push(&path);
        quoted.push("\"");
        shell_execute("notepad.exe", Some(&quoted));
    }
}

fn show_error(message: &str) {
    let text = wide(message);
    let title = wide("CASPER Shot");
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR) };
}

fn register_hotkeys(tray: &mut TrayIcon, state: &State) {
    hotkey_windows::unregister();
    let registered = hotkey_windows::register(&state.hotkey_fullscreen, &state.hotkey_region);
    tray.set_hotkeys(&state.hotkey_fullscreen, &state.hotkey_region, registered);
}

fn main() {
    if !acquire_single_instance_lock() {
        return;
    }

    let Some(webp) = Webp::load(&state::exe_dir_join("libwebp.dll")) else {
        show_error("libwebp.dll or libsharpyuv.dll is missing next to casper_shot.exe.\nExtract the whole archive again.");
        return;
    };
    let webp = Rc::new(webp);
    let state = Rc::new(RefCell::new(state::load(&state::state_path())));

    let mut tray = {
        let s = state.borrow();
        TrayIcon::create(&s.hotkey_fullscreen, &s.hotkey_region)
    };
    register_hotkeys(&mut tray, &state.borrow());
    {
        let (webp, state) = (Rc::clone(&webp), Rc::clone(&state));
        tray.set_capture_region_hook(Box::new(move || capture_region(&webp, &state.borrow())));
    }

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
        if msg.hwnd.is_null() && msg.message == WM_HOTKEY {
            match msg.wParam as i32 {
                hotkey_windows::ID_FULLSCREEN => capture_fullscreen(&webp, &state.borrow()),
                hotkey_windows::ID_REGION => capture_region(&webp, &state.borrow()),
                _ => {}
            }
        } else {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        match tray.take_event() {
            Some(TrayEvent::ToggleHotkeys) => {
                if tray.hotkeys_enabled() {
                    register_hotkeys(&mut tray, &state.borrow());
                } else {
                    hotkey_windows::unregister();
                }
            }
            Some(TrayEvent::OpenScreenshots) => {
                shell_open(state.borrow().resolve_save_path());
            }
            Some(TrayEvent::OpenSettings) => open_settings(),
            Some(TrayEvent::OpenGitHub) => {
                shell_open(GITHUB_URL);
            }
            Some(TrayEvent::Reload) => {
                *state.borrow_mut() = state::load(&state::state_path());
                let s = state.borrow();
                if tray.hotkeys_enabled() {
                    register_hotkeys(&mut tray, &s);
                } else {
                    tray.set_hotkeys(&s.hotkey_fullscreen, &s.hotkey_region, false);
                }
            }
            Some(TrayEvent::Quit) => break,
            None => {}
        }
    }

    hotkey_windows::unregister();
    drop(tray);
}
