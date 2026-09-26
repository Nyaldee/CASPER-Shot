
use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetDIBits, GetMonitorInfoW,
    MonitorFromPoint, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, SRCCOPY,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

pub(crate) struct Capture {
    pub(crate) bgra: Vec<u8>,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

fn top_down_header(width: i32, height: i32) -> BITMAPINFO {
    let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        ..unsafe { std::mem::zeroed() }
    };
    info
}

fn buffer_len(width: i32, height: i32) -> usize {
    width.max(0) as usize * height.max(0) as usize * 4
}

impl Capture {
    pub(crate) fn crop(&self, x: i32, y: i32, width: i32, height: i32) -> Capture {
        let x = x.clamp(0, self.width);
        let y = y.clamp(0, self.height);
        let width = width.clamp(0, self.width - x);
        let height = height.clamp(0, self.height - y);
        let row_len = width as usize * 4;
        let mut bgra = Vec::with_capacity(buffer_len(width, height));
        for row in y..y + height {
            let start = (row as usize * self.width as usize + x as usize) * 4;
            bgra.extend_from_slice(&self.bgra[start..start + row_len]);
        }
        Capture { bgra, width, height }
    }

    pub(crate) unsafe fn to_gdi_bitmap(&self, compat_dc: HDC, darken: bool) -> (HDC, HBITMAP) {
        let mem_dc = CreateCompatibleDC(compat_dc);
        let header = top_down_header(self.width, self.height);
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(mem_dc, &header, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if !bitmap.is_null() && !bits.is_null() {
            let dst = std::slice::from_raw_parts_mut(bits as *mut u8, self.bgra.len());
            if darken {
                for (d, s) in dst.iter_mut().zip(&self.bgra) {
                    *d = s >> 1;
                }
            } else {
                dst.copy_from_slice(&self.bgra);
            }
        }
        SelectObject(mem_dc, bitmap);
        (mem_dc, bitmap)
    }
}

pub(crate) fn capture_virtual_screen() -> Option<Capture> {
    unsafe {
        capture_region(
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

pub(crate) fn monitor_rect_under_cursor() -> Option<RECT> {
    unsafe {
        let mut cursor = POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor);
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        Some(info.rcMonitor)
    }
}

pub(crate) fn capture_screen_under_cursor() -> Option<Capture> {
    let Some(r) = monitor_rect_under_cursor() else { return capture_virtual_screen() };
    unsafe { capture_region(r.left, r.top, r.right - r.left, r.bottom - r.top) }
}

unsafe fn capture_region(x: i32, y: i32, width: i32, height: i32) -> Option<Capture> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let screen_dc = GetDC(std::ptr::null_mut());
    if screen_dc.is_null() {
        return None;
    }
    let mem_dc = CreateCompatibleDC(screen_dc);
    let bitmap = if mem_dc.is_null() { std::ptr::null_mut() } else { CreateCompatibleBitmap(screen_dc, width, height) };

    let mut bgra = vec![0u8; buffer_len(width, height)];
    let mut ok = false;
    if !bitmap.is_null() {
        let previous = SelectObject(mem_dc, bitmap);
        let mut header = top_down_header(width, height);
        ok = BitBlt(mem_dc, 0, 0, width, height, screen_dc, x, y, SRCCOPY) != 0
            && GetDIBits(mem_dc, bitmap, 0, height as u32, bgra.as_mut_ptr() as *mut _, &mut header, DIB_RGB_COLORS) != 0;
        SelectObject(mem_dc, previous);
        DeleteObject(bitmap);
    }
    if !mem_dc.is_null() {
        DeleteDC(mem_dc);
    }
    ReleaseDC(std::ptr::null_mut(), screen_dc);

    if !ok {
        return None;
    }
    for alpha in bgra.iter_mut().skip(3).step_by(4) {
        *alpha = 0xFF;
    }
    Some(Capture { bgra, width, height })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_borne_le_rectangle_a_la_capture() {
        let capture = Capture { bgra: (0..4 * 4 * 4).map(|i| i as u8).collect(), width: 4, height: 4 };
        let cropped = capture.crop(2, 3, 10, 10);
        assert_eq!((cropped.width, cropped.height), (2, 1));
        assert_eq!(cropped.bgra, capture.bgra[(3 * 4 + 2) * 4..(3 * 4 + 4) * 4]);
    }
}
