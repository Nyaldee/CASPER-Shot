
use crate::capture_windows::Capture;
use crate::win_util::wide;
use std::ffi::c_void;
use std::path::Path;
use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
};

type EncodeBgraFn = unsafe extern "C" fn(*const u8, i32, i32, i32, f32, *mut *mut u8) -> usize;
type EncodeLosslessBgraFn = unsafe extern "C" fn(*const u8, i32, i32, i32, *mut *mut u8) -> usize;
type FreeFn = unsafe extern "C" fn(*mut c_void);

pub(crate) struct Webp {
    module: HMODULE,
    encode_bgra: EncodeBgraFn,
    encode_lossless_bgra: EncodeLosslessBgraFn,
    free: FreeFn,
}

impl Webp {
    pub(crate) fn load(path: &Path) -> Option<Webp> {
        let wide_path = wide(path);
        let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32;
        let module = unsafe { LoadLibraryExW(wide_path.as_ptr(), std::ptr::null_mut(), flags) };
        if module.is_null() {
            return None;
        }
        unsafe {
            let symbols = (
                GetProcAddress(module, c"WebPEncodeBGRA".as_ptr().cast()),
                GetProcAddress(module, c"WebPEncodeLosslessBGRA".as_ptr().cast()),
                GetProcAddress(module, c"WebPFree".as_ptr().cast()),
            );
            let (Some(encode_bgra), Some(encode_lossless_bgra), Some(free)) = symbols else {
                FreeLibrary(module);
                return None;
            };
            Some(Webp {
                module,
                encode_bgra: std::mem::transmute::<unsafe extern "system" fn() -> isize, EncodeBgraFn>(encode_bgra),
                encode_lossless_bgra: std::mem::transmute::<unsafe extern "system" fn() -> isize, EncodeLosslessBgraFn>(encode_lossless_bgra),
                free: std::mem::transmute::<unsafe extern "system" fn() -> isize, FreeFn>(free),
            })
        }
    }

    pub(crate) fn encode(&self, capture: &Capture, quality: u8) -> Option<Vec<u8>> {
        let (bgra, width, height, stride) = (capture.bgra.as_ptr(), capture.width, capture.height, capture.width * 4);
        let mut out: *mut u8 = std::ptr::null_mut();
        let len = unsafe {
            if quality >= 100 {
                (self.encode_lossless_bgra)(bgra, width, height, stride, &mut out)
            } else {
                (self.encode_bgra)(bgra, width, height, stride, quality as f32, &mut out)
            }
        };
        if out.is_null() {
            return None;
        }
        let result = (len > 0).then(|| unsafe { std::slice::from_raw_parts(out, len) }.to_vec());
        unsafe { (self.free)(out as *mut c_void) };
        result
    }
}

impl Drop for Webp {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.module) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dll_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("vendor-build").join("libwebp.dll")
    }

    #[test]
    fn encode_avec_et_sans_perte_produit_du_webp_valide() {
        let webp = Webp::load(&dll_path()).expect("libwebp.dll introuvable ou invalide");
        let pixel = [0u8, 0, 255, 255];
        let capture = Capture { bgra: pixel.iter().copied().cycle().take(4 * 4 * 4).collect(), width: 4, height: 4 };

        for quality in [90, 100] {
            let encoded = webp.encode(&capture, quality).expect("échec de l'encodage");
            assert_eq!(&encoded[0..4], b"RIFF");
            assert_eq!(&encoded[8..12], b"WEBP");
        }
    }
}
