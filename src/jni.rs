//! The Android app's bridge (JNI): the `external fun`s of `io.lucapf.aequalis.Core`, in Kotlin. Each is
//! a thin wrapper over the C interface (`ffi.rs`): UTF-8 bytes in, a JSON reply out, as bytes too
//! (Java's own string encoding in JNI is "modified UTF-8", which emoji and NUL break; bytes are
//! exact, and Kotlin decodes them in one call).
//!
//! No `jni` crate: the few JNI functions used are taken from the function table every `JNIEnv`
//! points to, by their place in it (fixed by the JNI specification, `jni.h`).

use std::ffi::c_void;

/// `JNIEnv *`: a pointer to the pointer to the function table.
type Env = *mut *const *const c_void;
/// `jclass`, `jbyteArray`: opaque references owned by the Java side.
type Class = *mut c_void;
type ByteArray = *mut c_void;

// Places in the JNI function table (`struct JNINativeInterface`).
const GET_ARRAY_LENGTH: usize = 171;
const NEW_BYTE_ARRAY: usize = 176;
const GET_BYTE_ARRAY_REGION: usize = 200;
const SET_BYTE_ARRAY_REGION: usize = 208;

/// The JNI function at `index` of `env`'s table, as the function type `F`.
///
/// # Safety
/// `env` is the `JNIEnv *` Java passed in, and `F` is that function's signature.
unsafe fn function<F: Copy>(env: Env, index: usize) -> F {
    unsafe {
        let table = *env;
        let pointer = *table.add(index);
        std::mem::transmute_copy(&pointer)
    }
}

/// The bytes of a Java `byte[]` (`None`: null).
///
/// # Safety
/// `env` and `array` come from Java, in this call.
unsafe fn bytes(env: Env, array: ByteArray) -> Option<Vec<u8>> {
    if array.is_null() {
        return None;
    }
    unsafe {
        let length: extern "system" fn(Env, ByteArray) -> i32 = function(env, GET_ARRAY_LENGTH);
        let region: extern "system" fn(Env, ByteArray, i32, i32, *mut u8) = function(env, GET_BYTE_ARRAY_REGION);
        let n = length(env, array);
        let mut out = vec![0u8; usize::try_from(n).ok()?];
        region(env, array, 0, n, out.as_mut_ptr());
        Some(out)
    }
}

/// A new Java `byte[]` holding `text` in UTF-8.
///
/// # Safety
/// `env` comes from Java, in this call.
unsafe fn to_java(env: Env, text: &str) -> ByteArray {
    let Ok(n) = i32::try_from(text.len()) else { return std::ptr::null_mut() };
    unsafe {
        let new: extern "system" fn(Env, i32) -> ByteArray = function(env, NEW_BYTE_ARRAY);
        let set: extern "system" fn(Env, ByteArray, i32, i32, *const u8) = function(env, SET_BYTE_ARRAY_REGION);
        let array = new(env, n);
        if !array.is_null() {
            set(env, array, 0, n, text.as_ptr());
        }
        array
    }
}

/// Whether a Java `byte[]` holds more bytes than the longest request can take (`ffi::MAX_BYTES`):
/// then it is refused without copying it.
///
/// # Safety
/// As `bytes`.
unsafe fn too_many_bytes(env: Env, array: ByteArray) -> bool {
    if array.is_null() {
        return false;
    }
    unsafe {
        let length: extern "system" fn(Env, ByteArray) -> i32 = function(env, GET_ARRAY_LENGTH);
        usize::try_from(length(env, array)).map_or(true, |n| n > crate::ffi::MAX_BYTES)
    }
}

/// The text of a Java `byte[]` of UTF-8 (`None`: null or not UTF-8).
///
/// # Safety
/// As `bytes`.
unsafe fn text(env: Env, array: ByteArray) -> Option<String> {
    String::from_utf8(unsafe { bytes(env, array) }?).ok()
}

fn error(message: &str) -> String {
    crate::json::Object::new().num("ok", false).str("error", message).finish()
}

/// `Core.init(config)`, once, first: what the app knows about the device, as `aequalis_init`'s JSON
/// (`{"dir": …, "zone": …, "zoneinfo": …, "decimal": …, …}`). Returns `{"ok":true,"max_input":512}` or the error.
#[no_mangle]
pub extern "system" fn Java_io_lucapf_aequalis_Core_init(env: Env, _class: Class, config: ByteArray) -> ByteArray {
    let reply = match unsafe { text(env, config) }.map(|t| crate::ffi::parse_init(&t)) {
        Some(Ok(init)) => {
            crate::ffi::start(&init);
            crate::ffi::started()
        }
        Some(Err(e)) => error(&e),
        None => error("init: no configuration"),
    };
    unsafe { to_java(env, &reply) }
}

/// `Core.eval(request, submitted)`: the reply as the app shows it (`json::view`): what to show, its
/// message, the value to copy, the choices of an ambiguous request. `submitted` is a `jboolean`.
#[no_mangle]
pub extern "system" fn Java_io_lucapf_aequalis_Core_eval(env: Env, _class: Class, request: ByteArray, submitted: u8) -> ByteArray {
    // Too long: refused before its bytes are even copied.
    if unsafe { too_many_bytes(env, request) } {
        return unsafe { to_java(env, &crate::ffi::too_long()) };
    }
    let reply = match unsafe { text(env, request) } {
        Some(request) => crate::json::view(&request, submitted != 0),
        None => error("no request"),
    };
    unsafe { to_java(env, &reply) }
}

/// `Core.suggest(before)`: suggestions for the word being typed at the end of `before` (the text up
/// to the cursor), from its third character (`ffi::suggest`).
#[no_mangle]
pub extern "system" fn Java_io_lucapf_aequalis_Core_suggest(env: Env, _class: Class, before: ByteArray) -> ByteArray {
    if unsafe { too_many_bytes(env, before) } {
        return unsafe { to_java(env, crate::ffi::NO_SUGGESTIONS) };
    }
    let reply = match unsafe { text(env, before) } {
        Some(before) => crate::ffi::suggest(&before, 3),
        None => error("no text"),
    };
    unsafe { to_java(env, &reply) }
}

/// `Core.lists(which)`: the lists for the keyboard's panels and Settings, as the C interface gives
/// them: `measures` (the unit classes and their units), `currencies`, `trivia`, `settings`; and
/// `about`, `memory` (what the core holds, `ffi::memory`).
#[no_mangle]
pub extern "system" fn Java_io_lucapf_aequalis_Core_lists(env: Env, _class: Class, which: ByteArray) -> ByteArray {
    let reply = match unsafe { text(env, which) }.as_deref() {
        Some("measures") => crate::ffi::measures(),
        Some("currencies") => crate::ffi::currencies(),
        Some("trivia") => crate::ffi::trivia(),
        Some("settings") => crate::ffi::settings(),
        Some("about") => crate::ffi::about(),
        Some("memory") => crate::ffi::memory(),
        _ => error("lists: measures, currencies, trivia, settings, about or memory"),
    };
    unsafe { to_java(env, &reply) }
}

/// `Core.set(key, value)`: changes a setting and saves it (an empty value: its default), as
/// `aequalis --set key=value`: `{"ok":true,"key":…,"value":…}` or the error.
#[no_mangle]
pub extern "system" fn Java_io_lucapf_aequalis_Core_set(env: Env, _class: Class, key: ByteArray, value: ByteArray) -> ByteArray {
    let reply = match (unsafe { text(env, key) }, unsafe { text(env, value) }) {
        (Some(key), Some(value)) => crate::ffi::set(&key, Some(value.as_str()).filter(|v| !v.is_empty())),
        _ => error("set: a key and a value"),
    };
    unsafe { to_java(env, &reply) }
}

/// `Core.seed(rates, cities)`: the data shipped with the app (`ffi::seed`): ExchangeRate-API's
/// answer (empty: none) and the path of the city archive's JSON file (empty: none). Off the main
/// thread: installing the archive takes a second or two.
#[no_mangle]
pub extern "system" fn Java_io_lucapf_aequalis_Core_seed(env: Env, _class: Class, rates: ByteArray, cities: ByteArray) -> ByteArray {
    let rates = unsafe { text(env, rates) }.filter(|r| !r.is_empty());
    let cities = unsafe { text(env, cities) }.filter(|c| !c.is_empty());
    let reply = crate::ffi::seed(rates.as_deref(), cities.as_deref().map(std::path::Path::new));
    unsafe { to_java(env, &reply) }
}
