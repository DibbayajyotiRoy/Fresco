//! Minimal libmpv FFI loaded at runtime via dlopen.
//!
//! We dlopen `libmpv.so.2` then fall back to `libmpv.so.1`, and bind only
//! symbols whose signatures are identical across mpv ABI 1.x and 2.x. This
//! lets a single .deb run on Ubuntu 22.04 / Pop!_OS 22.04 (libmpv1) as well as
//! Debian 12 / Ubuntu 24.04 (libmpv2). We never touch `mpv_render_*` — video is
//! embedded into an existing X11 window via the `wid` option instead.

use std::ffi::{c_char, c_int, c_ulong, c_void, CStr, CString};
use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use libloading::Library;

pub type MpvHandle = *mut c_void;

#[allow(clippy::type_complexity)]
pub struct MpvFns {
    _lib: Library,
    pub soname: &'static str,
    create: unsafe extern "C" fn() -> MpvHandle,
    initialize: unsafe extern "C" fn(MpvHandle) -> c_int,
    terminate_destroy: unsafe extern "C" fn(MpvHandle),
    set_option_string: unsafe extern "C" fn(MpvHandle, *const c_char, *const c_char) -> c_int,
    set_property_string: unsafe extern "C" fn(MpvHandle, *const c_char, *const c_char) -> c_int,
    get_property_string: unsafe extern "C" fn(MpvHandle, *const c_char) -> *mut c_char,
    command: unsafe extern "C" fn(MpvHandle, *const *const c_char) -> c_int,
    free: unsafe extern "C" fn(*mut c_void),
    client_api_version: unsafe extern "C" fn() -> c_ulong,
}

// SAFETY: libmpv's client API is thread-safe; the function pointers are plain
// code addresses and the Library stays loaded for the process lifetime.
unsafe impl Send for MpvFns {}
unsafe impl Sync for MpvFns {}

static FNS: OnceLock<MpvFns> = OnceLock::new();

/// Load libmpv once. Returns an error describing which sonames were tried.
pub fn fns() -> Result<&'static MpvFns> {
    if let Some(f) = FNS.get() {
        return Ok(f);
    }
    let loaded = load()?;
    Ok(FNS.get_or_init(|| loaded))
}

fn load() -> Result<MpvFns> {
    let candidates = ["libmpv.so.2", "libmpv.so.1", "libmpv.so"];
    let mut last_err = String::new();
    for soname in candidates {
        match unsafe { Library::new(soname) } {
            Ok(lib) => return unsafe { bind(lib, soname) },
            Err(e) => last_err = format!("{soname}: {e}"),
        }
    }
    Err(anyhow!(
        "could not load libmpv (install libmpv2 or libmpv1). Last error: {last_err}"
    ))
}

unsafe fn bind(lib: Library, soname: &'static str) -> Result<MpvFns> {
    macro_rules! sym {
        ($name:literal, $ty:ty) => {{
            let s: libloading::Symbol<$ty> = lib
                .get(concat!($name, "\0").as_bytes())
                .map_err(|e| anyhow!(concat!("missing symbol ", $name, ": {}"), e))?;
            *s
        }};
    }
    // The soname string is needed for the static soname field below.
    let fns = MpvFns {
        soname,
        create: sym!("mpv_create", unsafe extern "C" fn() -> MpvHandle),
        initialize: sym!("mpv_initialize", unsafe extern "C" fn(MpvHandle) -> c_int),
        terminate_destroy: sym!("mpv_terminate_destroy", unsafe extern "C" fn(MpvHandle)),
        set_option_string: sym!(
            "mpv_set_option_string",
            unsafe extern "C" fn(MpvHandle, *const c_char, *const c_char) -> c_int
        ),
        set_property_string: sym!(
            "mpv_set_property_string",
            unsafe extern "C" fn(MpvHandle, *const c_char, *const c_char) -> c_int
        ),
        get_property_string: sym!(
            "mpv_get_property_string",
            unsafe extern "C" fn(MpvHandle, *const c_char) -> *mut c_char
        ),
        command: sym!(
            "mpv_command",
            unsafe extern "C" fn(MpvHandle, *const *const c_char) -> c_int
        ),
        free: sym!("mpv_free", unsafe extern "C" fn(*mut c_void)),
        client_api_version: sym!("mpv_client_api_version", unsafe extern "C" fn() -> c_ulong),
        _lib: lib,
    };
    Ok(fns)
}

impl MpvFns {
    pub fn create(&self) -> MpvHandle {
        unsafe { (self.create)() }
    }

    /// # Safety
    /// `h` must be a live handle from [`MpvFns::create`] that has not been
    /// destroyed. The same applies to every method below that takes a handle.
    pub unsafe fn initialize(&self, h: MpvHandle) -> c_int {
        unsafe { (self.initialize)(h) }
    }

    /// # Safety
    /// `h` must be a live handle from [`MpvFns::create`].
    pub unsafe fn terminate_destroy(&self, h: MpvHandle) {
        unsafe { (self.terminate_destroy)(h) }
    }

    /// # Safety
    /// `h` must be a live handle from [`MpvFns::create`].
    pub unsafe fn set_option(&self, h: MpvHandle, name: &str, val: &str) -> c_int {
        let n = CString::new(name).unwrap_or_default();
        let v = CString::new(val).unwrap_or_default();
        unsafe { (self.set_option_string)(h, n.as_ptr(), v.as_ptr()) }
    }

    /// # Safety
    /// `h` must be a live handle from [`MpvFns::create`].
    pub unsafe fn set_property(&self, h: MpvHandle, name: &str, val: &str) -> c_int {
        let n = CString::new(name).unwrap_or_default();
        let v = CString::new(val).unwrap_or_default();
        unsafe { (self.set_property_string)(h, n.as_ptr(), v.as_ptr()) }
    }

    /// Read a string property; None if unset/error.
    ///
    /// # Safety
    /// `h` must be a live handle from [`MpvFns::create`].
    pub unsafe fn get_property(&self, h: MpvHandle, name: &str) -> Option<String> {
        let n = CString::new(name).ok()?;
        unsafe {
            let ptr = (self.get_property_string)(h, n.as_ptr());
            if ptr.is_null() {
                return None;
            }
            let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
            (self.free)(ptr as *mut c_void);
            Some(s)
        }
    }

    /// Run an mpv command given as a list of string arguments.
    ///
    /// # Safety
    /// `h` must be a live handle from [`MpvFns::create`].
    pub unsafe fn command(&self, h: MpvHandle, args: &[&str]) -> c_int {
        let cstrings: Vec<CString> = args
            .iter()
            .map(|a| CString::new(*a).unwrap_or_default())
            .collect();
        let mut ptrs: Vec<*const c_char> = cstrings.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        unsafe { (self.command)(h, ptrs.as_ptr()) }
    }

    pub fn client_api_version(&self) -> c_ulong {
        unsafe { (self.client_api_version)() }
    }
}
