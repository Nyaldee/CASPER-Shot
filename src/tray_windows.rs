
use crate::popup_menu;
use crate::win_util::{simple_wndclass, wide};
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, GetWindowLongPtrW, LoadIconW,
    PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SetWindowLongPtrW, CREATESTRUCTW, CW_USEDEFAULT, GWLP_USERDATA,
    HICON, IDI_APPLICATION, MSGFLT_ALLOW, WM_APP, WM_DESTROY, WM_LBUTTONUP, WM_NCCREATE, WM_RBUTTONUP,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_OVERLAPPED,
};

const WM_TRAYICON: u32 = WM_APP + 1;

#[derive(Clone, Copy)]
pub(crate) enum TrayEvent {
    ToggleHotkeys,
    OpenScreenshots,
    OpenSettings,
    Reload,
    OpenGitHub,
    Quit,
}

struct TrayState {
    notify: NOTIFYICONDATAW,
    taskbar_created: u32,
    hotkeys_enabled: bool,
    hotkeys_registered: bool,
    event: Option<TrayEvent>,
    fullscreen_hotkey: String,
    region_hotkey: String,
    on_capture_region: Option<Box<dyn FnMut()>>,
}

impl TrayState {
    fn tooltip(&self) -> String {
        if !self.hotkeys_enabled {
            "CASPER Shot (hotkeys disabled)".to_string()
        } else if !self.hotkeys_registered {
            "CASPER Shot (hotkeys unavailable)".to_string()
        } else {
            format!("CASPER Shot ({}, {})", self.fullscreen_hotkey, self.region_hotkey)
        }
    }

    unsafe fn update_tooltip(&mut self) {
        let tip: Vec<u16> = self.tooltip().encode_utf16().collect();
        let len = tip.len().min(self.notify.szTip.len() - 1);
        self.notify.szTip[..len].copy_from_slice(&tip[..len]);
        self.notify.szTip[len] = 0;
        Shell_NotifyIconW(NIM_MODIFY, &self.notify);
    }

    unsafe fn open_menu(&mut self, hwnd: HWND) {
        let toggle_label = if self.hotkeys_enabled { "Disable hotkeys" } else { "Enable hotkeys" };
        let items = [
            (TrayEvent::ToggleHotkeys, toggle_label),
            (TrayEvent::OpenScreenshots, "Open Screenshots"),
            (TrayEvent::OpenSettings, "Settings"),
            (TrayEvent::Reload, "Reload"),
            (TrayEvent::OpenGitHub, "GitHub"),
            (TrayEvent::Quit, "Exit CASPER Shot"),
        ];
        let Some(event) = popup_menu::show(hwnd, &items) else { return };
        if let TrayEvent::ToggleHotkeys = event {
            self.hotkeys_enabled = !self.hotkeys_enabled;
            self.update_tooltip();
        }
        self.event = Some(event);
    }
}

unsafe fn get_state<'a>(hwnd: HWND) -> Option<&'a mut TrayState> {
    (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState).as_mut()
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let createstruct = &*(lparam as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, createstruct.lpCreateParams as isize);
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let Some(state) = get_state(hwnd) else { return DefWindowProcW(hwnd, msg, wparam, lparam) };

    match msg {
        WM_TRAYICON => match lparam as u32 {
            WM_LBUTTONUP => {
                if let Some(hook) = &mut state.on_capture_region {
                    hook();
                }
            }
            WM_RBUTTONUP => state.open_menu(hwnd),
            _ => {}
        },
        WM_DESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            Shell_NotifyIconW(NIM_DELETE, &state.notify);
            DestroyIcon(state.notify.hIcon);
            drop(Box::from_raw(state as *mut TrayState));
            PostQuitMessage(0);
        }
        _ if msg == state.taskbar_created => {
            Shell_NotifyIconW(NIM_ADD, &state.notify);
        }
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    0
}

unsafe fn load_icon(hinstance: HINSTANCE) -> HICON {
    let handle = LoadIconW(hinstance, std::ptr::without_provenance(1));
    if handle.is_null() { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) } else { handle }
}

pub(crate) struct TrayIcon {
    hwnd: HWND,
}

impl TrayIcon {
    pub(crate) fn create(fullscreen_hotkey: &str, region_hotkey: &str) -> TrayIcon {
        unsafe {
            let class_name = wide("CasperTray");
            let hinstance = GetModuleHandleW(std::ptr::null());
            let wc = simple_wndclass(class_name.as_ptr(), Some(wnd_proc), hinstance, std::ptr::null_mut());
            RegisterClassExW(&wc);

            let taskbar_created = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
            let mut state = Box::new(TrayState {
                notify: std::mem::zeroed(),
                taskbar_created,
                hotkeys_enabled: true,
                hotkeys_registered: true,
                event: None,
                fullscreen_hotkey: fullscreen_hotkey.to_string(),
                region_hotkey: region_hotkey.to_string(),
                on_capture_region: None,
            });
            state.notify.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            state.notify.uID = 1;
            state.notify.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            state.notify.uCallbackMessage = WM_TRAYICON;
            state.notify.hIcon = load_icon(hinstance);

            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                std::ptr::null(),
                WS_OVERLAPPED,
                CW_USEDEFAULT,
                0,
                CW_USEDEFAULT,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                Box::into_raw(state) as _,
            );
            ChangeWindowMessageFilterEx(hwnd, WM_TRAYICON, MSGFLT_ALLOW, std::ptr::null_mut());
            ChangeWindowMessageFilterEx(hwnd, taskbar_created, MSGFLT_ALLOW, std::ptr::null_mut());

            if let Some(state) = get_state(hwnd) {
                state.notify.hWnd = hwnd;
                Shell_NotifyIconW(NIM_ADD, &state.notify);
                state.update_tooltip();
            }
            TrayIcon { hwnd }
        }
    }

    fn state(&mut self) -> Option<&mut TrayState> {
        unsafe { get_state(self.hwnd) }
    }

    pub(crate) fn take_event(&mut self) -> Option<TrayEvent> {
        self.state().and_then(|s| s.event.take())
    }

    pub(crate) fn set_capture_region_hook(&mut self, hook: Box<dyn FnMut()>) {
        if let Some(state) = self.state() {
            state.on_capture_region = Some(hook);
        }
    }

    pub(crate) fn hotkeys_enabled(&mut self) -> bool {
        self.state().is_none_or(|s| s.hotkeys_enabled)
    }

    pub(crate) fn set_hotkeys(&mut self, fullscreen: &str, region: &str, registered: bool) {
        if let Some(state) = self.state() {
            state.fullscreen_hotkey = fullscreen.to_string();
            state.region_hotkey = region.to_string();
            state.hotkeys_registered = registered;
            unsafe { state.update_tooltip() };
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd) };
    }
}
