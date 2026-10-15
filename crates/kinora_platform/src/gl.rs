//! Reads the OpenGL renderer name ("llvmpipe", "Mesa Intel(R) UHD Graphics 620", …) from a
//! live OpenGL context, to find out whether drawing really happens on a GPU (brief §7.6).
//!
//! Slint hands its renderer's `get_proc_address` function to the app's rendering notifier
//! (on the thread that owns the current GL context). With it we look up `glGetString` and
//! call it once. That is a C function call, hence the `unsafe` below.

use std::ffi::{CStr, c_void};

/// `GL_RENDERER` from the OpenGL headers.
const GL_RENDERER: u32 = 0x1F01;

/// `const GLubyte *glGetString(GLenum name)` with the platform's GL calling convention.
type GlGetString = unsafe extern "system" fn(u32) -> *const u8;

/// The renderer string of the OpenGL context that is current on this thread.
///
/// Must be called on the thread with a current OpenGL context, e.g. from Slint's rendering
/// notifier. Returns `None` if the function can't be found or returns nothing.
#[must_use]
pub fn renderer_name(get_proc_address: &dyn Fn(&CStr) -> *const c_void) -> Option<String> {
    let address = get_proc_address(c"glGetString");
    if address.is_null() {
        return None;
    }
    // SAFETY: `address` is non-null and was returned by the OpenGL loader for the name
    // `glGetString`, whose C signature is exactly `GlGetString`. Function and data pointers
    // have the same size on every platform Kinora supports.
    let gl_get_string = unsafe { std::mem::transmute::<*const c_void, GlGetString>(address) };
    // SAFETY: the caller guarantees a current GL context on this thread (documented above);
    // GL_RENDERER is a valid argument, so the call returns a static string or null.
    let text = unsafe { gl_get_string(GL_RENDERER) };
    if text.is_null() {
        return None;
    }
    // SAFETY: glGetString returns a NUL-terminated string owned by the driver that stays
    // valid for the context's lifetime; we copy it out immediately.
    let name = unsafe { CStr::from_ptr(text.cast()) };
    Some(name.to_string_lossy().trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_function_gives_none() {
        let loader = |_: &CStr| std::ptr::null::<c_void>();
        assert_eq!(renderer_name(&loader), None);
    }
}
