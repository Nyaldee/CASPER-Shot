
use crate::capture_windows::{monitor_rect_under_cursor, Capture};
use crate::win_util::{shell_open, simple_wndclass, wide};
use std::path::PathBuf;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, DeleteDC, DeleteObject, EndPaint, GetDC, ReleaseDC, SetStretchBltMode, StretchBlt, HALFTONE, HBITMAP, HDC,
    PAINTSTRUCT, SRCCOPY,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW, GetWindowLongPtrW, LoadCursorW,
    PostQuitMessage, RegisterClassExW, SetTimer, SetWindowDisplayAffinity, SetWindowLongPtrW, ShowWindow, TranslateMessage,
    UnregisterClassW, GWLP_USERDATA, IDC_ARROW, MSG, SW_SHOWNOACTIVATE, WDA_EXCLUDEFROMCAPTURE, WM_DESTROY, WM_LBUTTONUP, WM_PAINT,
    WM_RBUTTONUP, WM_TIMER, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const TIMER_ID: usize = 1;
const BOTTOM_MARGIN: i32 = 24;

struct Toast {
    dc: HDC,
    bitmap: HBITMAP,
    src_width: i32,
    src_height: i32,
    path: Option<PathBuf>,
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(toast) = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Toast).as_ref() else {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    };
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rect = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            SetStretchBltMode(hdc, HALFTONE);
            StretchBlt(hdc, 0, 0, rect.right, rect.bottom, toast.dc, 0, 0, toast.src_width, toast.src_height, SRCCOPY);
            EndPaint(hwnd, &ps);
        }
        WM_LBUTTONUP => {
            if let Some(path) = &toast.path {
                shell_open(path);
            }
        }
        WM_RBUTTONUP => {
            if let Some(dir) = toast.path.as_deref().and_then(|p| p.parent()) {
                shell_open(dir);
            }
        }
        WM_TIMER => {
            DestroyWindow(hwnd);
        }
        WM_DESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            let toast = Box::from_raw(toast as *const Toast as *mut Toast);
            DeleteDC(toast.dc);
            DeleteObject(toast.bitmap);
            PostQuitMessage(0);
        }
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    0
}

fn preview_size(capture_w: i32, capture_h: i32, monitor_w: i32, monitor_h: i32, percent: u8) -> (i32, i32) {
    let scale = f32::from(percent) / 100.0 * (monitor_w as f32 / capture_w as f32).min(monitor_h as f32 / capture_h as f32);
    ((capture_w as f32 * scale) as i32, (capture_h as f32 * scale) as i32)
}

pub(crate) fn show(capture: Capture, path: Option<PathBuf>, percent: u8, duration_ms: u32) {
    if percent == 0 || capture.width <= 0 || capture.height <= 0 {
        return;
    }
    std::thread::spawn(move || unsafe {
        let Some(monitor) = monitor_rect_under_cursor() else { return };
        let (monitor_w, monitor_h) = (monitor.right - monitor.left, monitor.bottom - monitor.top);
        let (width, height) = preview_size(capture.width, capture.height, monitor_w, monitor_h, percent);
        if width <= 0 || height <= 0 {
            return;
        }

        let screen_dc = GetDC(std::ptr::null_mut());
        let (dc, bitmap) = capture.to_gdi_bitmap(screen_dc, false);
        ReleaseDC(std::ptr::null_mut(), screen_dc);
        let toast = Box::new(Toast { dc, bitmap, src_width: capture.width, src_height: capture.height, path });
        drop(capture);

        let class_name = wide("CasperToast");
        let hinstance = GetModuleHandleW(std::ptr::null());
        let class = simple_wndclass(class_name.as_ptr(), Some(wnd_proc), hinstance, LoadCursorW(std::ptr::null_mut(), IDC_ARROW));
        RegisterClassExW(&class);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_POPUP,
            monitor.left + (monitor_w - width) / 2,
            monitor.bottom - height - BOTTOM_MARGIN,
            width,
            height,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            DeleteDC(toast.dc);
            DeleteObject(toast.bitmap);
            return;
        }
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(toast) as isize);
        SetTimer(hwnd, TIMER_ID, duration_ms, None);
        SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        UnregisterClassW(class_name.as_ptr(), hinstance);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_size_tient_dans_le_pourcentage_sans_deformer() {
        assert_eq!(preview_size(1920, 1080, 1920, 1080, 40), (768, 432));
        assert_eq!(preview_size(100, 2000, 1920, 1080, 50), (27, 540));
    }
}
