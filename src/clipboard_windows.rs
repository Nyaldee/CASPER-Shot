
use crate::capture_windows::Capture;
use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::Graphics::Gdi::{BITMAPV5HEADER, BI_BITFIELDS};
use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::System::Ole::CF_DIBV5;

const LCS_SRGB: u32 = 0x7352_4742;

pub(crate) fn copy_bitmap(capture: &Capture) -> bool {
    let (width, height) = (capture.width, capture.height);
    if width <= 0 || height <= 0 {
        return false;
    }
    let stride = width as usize * 4;
    let image_size = capture.bgra.len();
    let header_size = std::mem::size_of::<BITMAPV5HEADER>();

    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return false;
        }
        EmptyClipboard();

        let handle = GlobalAlloc(GMEM_MOVEABLE, header_size + image_size);
        let ptr = if handle.is_null() { std::ptr::null_mut() } else { GlobalLock(handle) };
        if ptr.is_null() {
            if !handle.is_null() {
                GlobalFree(handle);
            }
            CloseClipboard();
            return false;
        }

        let mut header: BITMAPV5HEADER = std::mem::zeroed();
        header.bV5Size = header_size as u32;
        header.bV5Width = width;
        header.bV5Height = height;
        header.bV5Planes = 1;
        header.bV5BitCount = 32;
        header.bV5Compression = BI_BITFIELDS;
        header.bV5SizeImage = image_size as u32;
        header.bV5RedMask = 0x00FF_0000;
        header.bV5GreenMask = 0x0000_FF00;
        header.bV5BlueMask = 0x0000_00FF;
        header.bV5AlphaMask = 0;
        header.bV5CSType = LCS_SRGB;
        std::ptr::write(ptr as *mut BITMAPV5HEADER, header);

        let pixels = std::slice::from_raw_parts_mut((ptr as *mut u8).add(header_size), image_size);
        for (dst, src) in pixels.chunks_exact_mut(stride).zip(capture.bgra.chunks_exact(stride).rev()) {
            dst.copy_from_slice(src);
        }
        GlobalUnlock(handle);

        let ok = !SetClipboardData(CF_DIBV5.into(), handle).is_null();
        if !ok {
            GlobalFree(handle);
        }
        CloseClipboard();
        ok
    }
}
