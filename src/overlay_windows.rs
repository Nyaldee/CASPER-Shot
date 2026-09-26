
use crate::capture_windows::Capture;
use crate::win_util::{simple_wndclass, wide};
use windows_sys::core::BOOL;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateSolidBrush, DeleteDC, DeleteObject, EndPaint, FrameRect,
    GetDC, InvalidateRect, ReleaseDC, SelectObject, HBRUSH, HDC, PAINTSTRUCT, SRCCOPY,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumWindows, GetClassNameW, GetMessageW, GetSystemMetrics,
    GetWindowLongPtrW, GetWindowRect, IsIconic, IsWindowVisible, LoadCursorW, RegisterClassExW, SetForegroundWindow,
    SetWindowLongPtrW, ShowWindow, TranslateMessage, UnregisterClassW, GWLP_USERDATA, IDC_CROSS, MSG, SM_CXDRAG, SM_CYDRAG,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOW, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONUP,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const FRAME_COLOR: u32 = 0x0000FF00;

struct Overlay {
    bright_dc: HDC,
    dark_dc: HDC,
    mem_dc: HDC,
    frame_brush: HBRUSH,
    windows: Vec<RECT>,
    active_rect: RECT,
    press: Option<POINT>,
    dragging: bool,
    drag_threshold: (i32, i32),
    result: Option<RECT>,
    finished: bool,
}

impl Overlay {
    unsafe fn redraw(&self, dirty: &RECT) {
        blit(self.mem_dc, self.dark_dc, dirty);
        if blit(self.mem_dc, self.bright_dc, &self.active_rect) {
            FrameRect(self.mem_dc, &self.active_rect, self.frame_brush);
        }
    }

    fn finish(&mut self, result: Option<RECT>) {
        self.result = result;
        self.finished = true;
    }
}

unsafe fn blit(dst: HDC, src: HDC, r: &RECT) -> bool {
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    w > 0 && h > 0 && BitBlt(dst, r.left, r.top, w, h, src, r.left, r.top, SRCCOPY) != 0
}

fn union_rect(a: &RECT, b: &RECT) -> RECT {
    RECT { left: a.left.min(b.left), top: a.top.min(b.top), right: a.right.max(b.right), bottom: a.bottom.max(b.bottom) }
}

fn point_from_lparam(lparam: LPARAM) -> POINT {
    POINT { x: (lparam & 0xFFFF) as i16 as i32, y: ((lparam >> 16) & 0xFFFF) as i16 as i32 }
}

unsafe fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked: u32 = 0;
    DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, &mut cloaked as *mut u32 as *mut _, 4) >= 0 && cloaked != 0
}

unsafe fn is_desktop_class(hwnd: HWND) -> bool {
    let mut buf = [0u16; 16];
    let len = GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
    matches!(String::from_utf16_lossy(&buf[..len]).as_str(), "Progman" | "WorkerW")
}

unsafe fn visual_window_rect(hwnd: HWND) -> Option<RECT> {
    let mut rect: RECT = std::mem::zeroed();
    let size = std::mem::size_of::<RECT>() as u32;
    if DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS as u32, &mut rect as *mut RECT as *mut _, size) < 0
        && GetWindowRect(hwnd, &mut rect) == 0
    {
        return None;
    }
    (rect.right > rect.left && rect.bottom > rect.top).then_some(rect)
}

unsafe extern "system" fn collect_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam as *mut Vec<RECT>);
    if IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0 && !is_cloaked(hwnd) && !is_desktop_class(hwnd) {
        list.extend(visual_window_rect(hwnd));
    }
    1
}

unsafe fn snapshot_windows(origin: POINT) -> Vec<RECT> {
    let mut list: Vec<RECT> = Vec::new();
    EnumWindows(Some(collect_windows_proc), &mut list as *mut Vec<RECT> as LPARAM);
    for r in &mut list {
        *r = RECT { left: r.left - origin.x, top: r.top - origin.y, right: r.right - origin.x, bottom: r.bottom - origin.y };
    }
    list
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(state) = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Overlay).as_mut() else {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    };
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            blit(hdc, state.mem_dc, &ps.rcPaint);
            EndPaint(hwnd, &ps);
        }
        WM_MOUSEMOVE => {
            let point = point_from_lparam(lparam);
            if let Some(press) = state.press {
                let (tx, ty) = state.drag_threshold;
                state.dragging |= (point.x - press.x).abs() >= tx || (point.y - press.y).abs() >= ty;
            }
            let old_rect = state.active_rect;
            match state.press {
                Some(press) if state.dragging => {
                    state.active_rect = RECT {
                        left: press.x.min(point.x),
                        top: press.y.min(point.y),
                        right: press.x.max(point.x),
                        bottom: press.y.max(point.y),
                    };
                }
                Some(_) => {}
                None => {
                    let hit = state.windows.iter().find(|r| point.x >= r.left && point.x < r.right && point.y >= r.top && point.y < r.bottom);
                    if let Some(rect) = hit {
                        state.active_rect = *rect;
                    }
                }
            }
            let dirty = union_rect(&old_rect, &state.active_rect);
            state.redraw(&dirty);
            InvalidateRect(hwnd, &dirty, 0);
        }
        WM_LBUTTONDOWN => {
            state.press = Some(point_from_lparam(lparam));
            SetCapture(hwnd);
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            let rect = state.active_rect;
            let valid = rect.right > rect.left && rect.bottom > rect.top;
            state.finish(valid.then_some(rect));
        }
        WM_KEYDOWN if wparam == VK_ESCAPE as usize => state.finish(None),
        WM_RBUTTONUP => state.finish(None),
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    0
}

pub(crate) fn pick_region(capture: &Capture) -> Option<RECT> {
    unsafe {
        let origin = POINT { x: GetSystemMetrics(SM_XVIRTUALSCREEN), y: GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let windows = snapshot_windows(origin);

        let screen_dc = GetDC(std::ptr::null_mut());
        let (bright_dc, bright_bitmap) = capture.to_gdi_bitmap(screen_dc, false);
        let (dark_dc, dark_bitmap) = capture.to_gdi_bitmap(screen_dc, true);
        let mem_dc = CreateCompatibleDC(screen_dc);
        let mem_bitmap = CreateCompatibleBitmap(screen_dc, capture.width, capture.height);
        SelectObject(mem_dc, mem_bitmap);
        ReleaseDC(std::ptr::null_mut(), screen_dc);

        let mut overlay = Overlay {
            bright_dc,
            dark_dc,
            mem_dc,
            frame_brush: CreateSolidBrush(FRAME_COLOR),
            windows,
            active_rect: std::mem::zeroed(),
            press: None,
            dragging: false,
            drag_threshold: (GetSystemMetrics(SM_CXDRAG), GetSystemMetrics(SM_CYDRAG)),
            result: None,
            finished: false,
        };
        overlay.redraw(&RECT { left: 0, top: 0, right: capture.width, bottom: capture.height });

        let class_name = wide("CasperOverlay");
        let hinstance = GetModuleHandleW(std::ptr::null());
        let class = simple_wndclass(class_name.as_ptr(), Some(wnd_proc), hinstance, LoadCursorW(std::ptr::null_mut(), IDC_CROSS));
        RegisterClassExW(&class);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_POPUP,
            origin.x,
            origin.y,
            capture.width,
            capture.height,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );
        if !hwnd.is_null() {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut overlay as *mut Overlay as isize);
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            let mut msg: MSG = std::mem::zeroed();
            while !overlay.finished && GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            DestroyWindow(hwnd);
        }
        UnregisterClassW(class_name.as_ptr(), hinstance);

        DeleteObject(overlay.frame_brush);
        for (dc, bitmap) in [(bright_dc, bright_bitmap), (dark_dc, dark_bitmap), (mem_dc, mem_bitmap)] {
            DeleteDC(dc);
            DeleteObject(bitmap);
        }
        overlay.result
    }
}
