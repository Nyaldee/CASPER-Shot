
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Foundation::HINSTANCE;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{HCURSOR, SW_SHOWNORMAL, WNDCLASSEXW, WNDPROC};

pub(crate) fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

pub(crate) fn shell_execute(file: impl AsRef<OsStr>, params: Option<&OsStr>) -> bool {
    let verb = wide("open");
    let file = wide(file);
    let params = params.map(wide);
    let params_ptr = params.as_ref().map_or(std::ptr::null(), |p| p.as_ptr());
    let result =
        unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), params_ptr, std::ptr::null(), SW_SHOWNORMAL) };
    result as isize > 32
}

pub(crate) fn shell_open(target: impl AsRef<OsStr>) -> bool {
    shell_execute(target, None)
}

pub(crate) fn simple_wndclass(class_name: *const u16, wndproc: WNDPROC, hinstance: HINSTANCE, hcursor: HCURSOR) -> WNDCLASSEXW {
    WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: wndproc,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: std::ptr::null_mut(),
        hCursor: hcursor,
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name,
        hIconSm: std::ptr::null_mut(),
    }
}
