
use crate::capture_windows::monitor_rect_under_cursor;
use crate::win_util::{simple_wndclass, wide};
use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontIndirectW, CreateSolidBrush, DeleteDC, DeleteObject,
    DrawTextW, EndPaint, FillRect, GetDC, GetTextExtentPoint32W, GetTextMetricsW, InvalidateRect, ReleaseDC, SelectObject,
    SetBkMode, SetTextColor, CLEARTYPE_QUALITY, DEFAULT_CHARSET, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FW_NORMAL,
    HBITMAP, HBRUSH, HDC, HFONT, LOGFONTW, PAINTSTRUCT, SRCCOPY, TEXTMETRICW, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, GetWindowLongPtrW, IsWindow,
    LoadCursorW, RegisterClassExW, SetForegroundWindow, SetWindowLongPtrW, ShowWindow, TranslateMessage, GWLP_USERDATA, IDC_ARROW,
    MSG, SW_SHOW, WA_INACTIVE, WM_ACTIVATE, WM_DESTROY, WM_ERASEBKGND, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};

const COLOR_BACKGROUND: COLORREF = 0x004a3c38;
const COLOR_TEXT: COLORREF = 0x00e3dad3;
const COLOR_SELECTED_BACKGROUND: COLORREF = 0x00e29452;
const COLOR_SELECTED_TEXT: COLORREF = 0x00ffffff;
const COLOR_BORDER: COLORREF = 0x0062514b;

const FONT_FAMILY: &str = "Segoe UI";
const BORDER_WIDTH: i32 = 1;
const FONT_SIZE: f64 = 11.0;
const PADDING_X: f64 = 14.0;
const PADDING_Y: f64 = 6.0;

struct MenuState {
    labels: Vec<Vec<u16>>,
    item_rects: Vec<RECT>,
    hover: Option<usize>,
    selected: Option<usize>,
    tracking_leave: bool,
    font: HFONT,
    background_brush: HBRUSH,
    selected_brush: HBRUSH,
    border_brush: HBRUSH,
    mem_dc: HDC,
    mem_bitmap: HBITMAP,
    size: (i32, i32),
}

impl MenuState {
    unsafe fn redraw(&self) {
        let hdc = self.mem_dc;
        FillRect(hdc, &RECT { left: 0, top: 0, right: self.size.0, bottom: self.size.1 }, self.border_brush);
        SetBkMode(hdc, TRANSPARENT as i32);
        let old_font = SelectObject(hdc, self.font);
        for (i, (label, r)) in self.labels.iter().zip(&self.item_rects).enumerate() {
            let hovered = self.hover == Some(i);
            FillRect(hdc, r, if hovered { self.selected_brush } else { self.background_brush });
            SetTextColor(hdc, if hovered { COLOR_SELECTED_TEXT } else { COLOR_TEXT });
            let pad = (r.bottom - r.top) / 3;
            let mut text_rect = RECT { left: r.left + pad, right: r.right - pad, ..*r };
            DrawTextW(hdc, label.as_ptr(), -1, &mut text_rect, DT_SINGLELINE | DT_VCENTER | DT_LEFT | DT_NOPREFIX);
        }
        SelectObject(hdc, old_font);
    }

    unsafe fn set_hover(&mut self, hwnd: HWND, hover: Option<usize>) {
        if hover != self.hover {
            self.hover = hover;
            self.redraw();
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
    }

    unsafe fn release(&self) {
        DeleteDC(self.mem_dc);
        for object in [self.mem_bitmap, self.font, self.background_brush, self.selected_brush, self.border_brush] {
            DeleteObject(object);
        }
    }
}

unsafe extern "system" fn menu_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(state) = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MenuState).as_mut() else {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    };
    match msg {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            BitBlt(hdc, 0, 0, state.size.0, state.size.1, state.mem_dc, 0, 0, SRCCOPY);
            EndPaint(hwnd, &ps);
        }
        WM_MOUSEMOVE => {
            if !state.tracking_leave {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut tme);
                state.tracking_leave = true;
            }
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
            let hover = state.item_rects.iter().position(|r| y >= r.top && y < r.bottom);
            state.set_hover(hwnd, hover);
        }
        WM_MOUSELEAVE => {
            state.tracking_leave = false;
            state.set_hover(hwnd, None);
        }
        WM_LBUTTONUP => {
            state.selected = state.hover;
            DestroyWindow(hwnd);
        }
        WM_ACTIVATE if (wparam & 0xFFFF) as u32 == WA_INACTIVE => {
            DestroyWindow(hwnd);
        }
        WM_DESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            state.release();
        }
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    0
}

unsafe fn make_font(pixel_height: i32) -> HFONT {
    let mut lf: LOGFONTW = std::mem::zeroed();
    lf.lfHeight = -pixel_height.max(1);
    lf.lfWeight = FW_NORMAL as i32;
    lf.lfCharSet = DEFAULT_CHARSET;
    lf.lfQuality = CLEARTYPE_QUALITY;
    let face = wide(FONT_FAMILY);
    let len = face.len().min(lf.lfFaceName.len());
    lf.lfFaceName[..len].copy_from_slice(&face[..len]);
    CreateFontIndirectW(&lf)
}

pub(crate) unsafe fn show<T: Copy>(parent: HWND, items: &[(T, &str)]) -> Option<T> {
    let class_name = wide("CasperMenu");
    let hinstance = GetModuleHandleW(std::ptr::null());
    let wc = simple_wndclass(class_name.as_ptr(), Some(menu_wndproc), hinstance, LoadCursorW(std::ptr::null_mut(), IDC_ARROW));
    RegisterClassExW(&wc);

    let scale = GetDpiForWindow(parent).max(1) as f64 / 96.0;
    let px = |v: f64| (v * scale).round() as i32;
    let font = make_font(px(FONT_SIZE));
    let labels: Vec<Vec<u16>> = items.iter().map(|(_, label)| wide(label)).collect();

    let screen_dc = GetDC(std::ptr::null_mut());
    let old_font = SelectObject(screen_dc, font);
    let mut tm: TEXTMETRICW = std::mem::zeroed();
    GetTextMetricsW(screen_dc, &mut tm);
    let row_h = tm.tmHeight + 2 * px(PADDING_Y);
    let text_w = labels
        .iter()
        .map(|label| {
            let mut size: SIZE = std::mem::zeroed();
            GetTextExtentPoint32W(screen_dc, label.as_ptr(), label.len() as i32 - 1, &mut size);
            size.cx
        })
        .max()
        .unwrap_or(0);
    SelectObject(screen_dc, old_font);
    let item_w = text_w + 2 * px(PADDING_X);

    let item_rects: Vec<RECT> = (0..labels.len() as i32)
        .map(|i| {
            let top = BORDER_WIDTH + i * row_h;
            RECT { left: BORDER_WIDTH, top, right: BORDER_WIDTH + item_w, bottom: top + row_h }
        })
        .collect();
    let size = (item_w + 2 * BORDER_WIDTH, labels.len() as i32 * row_h + 2 * BORDER_WIDTH);

    let mem_dc = CreateCompatibleDC(screen_dc);
    let mem_bitmap = CreateCompatibleBitmap(screen_dc, size.0.max(1), size.1.max(1));
    SelectObject(mem_dc, mem_bitmap);
    ReleaseDC(std::ptr::null_mut(), screen_dc);

    let mut state = Box::new(MenuState {
        labels,
        item_rects,
        hover: None,
        selected: None,
        tracking_leave: false,
        font,
        background_brush: CreateSolidBrush(COLOR_BACKGROUND),
        selected_brush: CreateSolidBrush(COLOR_SELECTED_BACKGROUND),
        border_brush: CreateSolidBrush(COLOR_BORDER),
        mem_dc,
        mem_bitmap,
        size,
    });
    state.redraw();

    let mut cursor = POINT { x: 0, y: 0 };
    GetCursorPos(&mut cursor);
    let bounds = monitor_rect_under_cursor().unwrap_or(RECT { left: 0, top: 0, right: size.0, bottom: size.1 });
    let x = (cursor.x - size.0).min(bounds.right - size.0).max(bounds.left);
    let y = (cursor.y - size.1).min(bounds.bottom - size.1).max(bounds.top);

    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        class_name.as_ptr(),
        std::ptr::null(),
        WS_POPUP,
        x,
        y,
        size.0,
        size.1,
        parent,
        std::ptr::null_mut(),
        hinstance,
        std::ptr::null(),
    );
    if hwnd.is_null() {
        state.release();
        return None;
    }
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut *state as *mut MenuState as isize);
    ShowWindow(hwnd, SW_SHOW);
    SetForegroundWindow(hwnd);
    SetFocus(hwnd);

    let mut msg: MSG = std::mem::zeroed();
    while IsWindow(hwnd) != 0 && GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    state.selected.map(|i| items[i].0)
}
