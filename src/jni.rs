//! The Android app's bridge (JNI): the `external fun`s of `io.lucapf.aequalis.Core`, in Kotlin. Each is
//! a thin wrapper over the C interface (`ffi.rs`): UTF-8 bytes in, a JSON reply out, as bytes too
//! (Java's own string encoding in JNI is "modified UTF-8", which emoji and NUL break; bytes are
//! exact, and Kotlin decodes them in one call).
//!
//! No `jni` crate: the few JNI functions used are taken from the function table every `JNIEnv`
//! points to, by their place in it (fixed by the JNI specification, `jni.h`).

use std::cell::Cell;
use std::ffi::{c_char, c_void, CStr};

/// `JNIEnv *`: a pointer to the pointer to the function table.
type Env = *mut *const *const c_void;
/// `jclass`, `jbyteArray`: opaque references owned by the Java side.
type Class = *mut c_void;
type ByteArray = *mut c_void;

// Places in the JNI function table (`struct JNINativeInterface`).
const EXCEPTION_CLEAR: usize = 17;
const DELETE_LOCAL_REF: usize = 23;
const GET_OBJECT_CLASS: usize = 31;
const GET_METHOD_ID: usize = 33;
const CALL_BOOLEAN_METHOD_A: usize = 39;
const GET_ARRAY_LENGTH: usize = 171;
const NEW_BYTE_ARRAY: usize = 176;
const GET_BYTE_ARRAY_REGION: usize = 200;
const SET_BYTE_ARRAY_REGION: usize = 208;
const EXCEPTION_CHECK: usize = 228;

/// `jvalue`: an argument of a call into Java (8 bytes, as its `jlong`).
#[repr(C)]
union Value {
    object: *mut c_void,
    int: i32,
    boolean: u8,
    long: i64,
}

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
///
/// # Safety
/// Called by the JVM only: `env` is its `JNIEnv *`, the arrays its references, valid in this call.
#[no_mangle]
pub unsafe extern "system" fn Java_io_lucapf_aequalis_Core_init(env: Env, _class: Class, config: ByteArray) -> ByteArray {
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

/// `Core.eval(request, submitted)`: the reply as the app shows it (`reply::view`): what to show, its
/// message, the value to copy, the choices of an ambiguous request. `submitted` is a `jboolean`.
///
/// # Safety
/// Called by the JVM only: `env` is its `JNIEnv *`, the arrays its references, valid in this call.
#[no_mangle]
pub unsafe extern "system" fn Java_io_lucapf_aequalis_Core_eval(env: Env, _class: Class, request: ByteArray, submitted: u8) -> ByteArray {
    // Too long: refused before its bytes are even copied.
    if unsafe { too_many_bytes(env, request) } {
        return unsafe { to_java(env, &crate::ffi::too_long()) };
    }
    let reply = match unsafe { text(env, request) } {
        Some(request) => crate::profile::timed(crate::profile::Kind::Answer, &request, || crate::reply::view(&request, submitted != 0)),
        None => error("no request"),
    };
    unsafe { to_java(env, &reply) }
}

/// `Core.suggest(before)`: suggestions for the word being typed at the end of `before` (the text up
/// to the cursor), from its third character (`ffi::suggest`).
///
/// # Safety
/// Called by the JVM only: `env` is its `JNIEnv *`, the arrays its references, valid in this call.
#[no_mangle]
pub unsafe extern "system" fn Java_io_lucapf_aequalis_Core_suggest(env: Env, _class: Class, before: ByteArray) -> ByteArray {
    if unsafe { too_many_bytes(env, before) } {
        return unsafe { to_java(env, crate::ffi::NO_SUGGESTIONS) };
    }
    let reply = match unsafe { text(env, before) } {
        Some(before) => crate::profile::timed(crate::profile::Kind::Suggestions, &before, || crate::ffi::suggest(&before, 3)),
        None => error("no text"),
    };
    unsafe { to_java(env, &reply) }
}

/// `Core.profile(kind, request, roundTripUs)`: the app's round trip of its call of `kind` (`answer`,
/// `suggestions`) for `request`, in µs (`profile::app`). `round_trip_us` is a `jlong`.
///
/// # Safety
/// Called by the JVM only: `env` is its `JNIEnv *`, `kind` and `request` its references, valid in
/// this call.
#[no_mangle]
pub unsafe extern "system" fn Java_io_lucapf_aequalis_Core_profile(env: Env, _class: Class, kind: ByteArray, request: ByteArray, round_trip_us: i64) {
    let (kind, request) = unsafe { (text(env, kind), text(env, request)) };
    if let (Some(kind), Some(request)) = (kind.as_deref().and_then(crate::profile::Kind::of), request) {
        crate::profile::app(kind, &request, u64::try_from(round_trip_us).unwrap_or(0));
    }
}

/// `Core.profileReset()`: everything profiling recorded forgotten (About's Reset).
///
/// # Safety
/// Called by the JVM only.
#[no_mangle]
pub unsafe extern "system" fn Java_io_lucapf_aequalis_Core_profileReset(_env: Env, _class: Class) {
    crate::profile::reset();
}

/// `Core.sizeKey(line, key)`: the request after a key of the Sizes strip or panel, `shoes` added when it
/// has none (`ffi::size_key`).
///
/// # Safety
/// Called by the JVM with valid `env`, `line` and `key`.
#[no_mangle]
pub unsafe extern "system" fn Java_io_lucapf_aequalis_Core_sizeKey(env: Env, _class: Class, line: ByteArray, key: ByteArray) -> ByteArray {
    if unsafe { too_many_bytes(env, line) } {
        return unsafe { to_java(env, &crate::ffi::too_long()) };
    }
    let reply = match unsafe { (text(env, line), text(env, key)) } {
        (Some(line), Some(key)) => crate::ffi::size_key(&line, &key),
        _ => error("sizeKey: a line and a key"),
    };
    unsafe { to_java(env, &reply) }
}

/// `Core.lists(which)`: the lists for the keyboard's panels and Settings, as the C interface gives
/// them: `measures` (the unit classes and their units), `currencies`, `trivia`, `settings`; and
/// `about`, `memory` (what the core holds, `ffi::memory`), `online` (online or offline, and the age
/// of the last sync, `ffi::online`).
///
/// # Safety
/// Called by the JVM only: `env` is its `JNIEnv *`, the arrays its references, valid in this call.
#[no_mangle]
pub unsafe extern "system" fn Java_io_lucapf_aequalis_Core_lists(env: Env, _class: Class, which: ByteArray) -> ByteArray {
    let reply = match unsafe { text(env, which) }.as_deref() {
        Some("measures") => crate::ffi::measures(),
        Some("currencies") => crate::ffi::currencies(),
        Some("trivia") => crate::ffi::trivia(),
        Some("settings") => crate::ffi::settings(),
        Some("about") => crate::ffi::about(),
        Some("memory") => crate::ffi::memory(),
        Some("online") => crate::ffi::online(),
        _ => error("lists: measures, currencies, trivia, settings, about, memory or online"),
    };
    unsafe { to_java(env, &reply) }
}

/// `Core.set(key, value)`: changes a setting and saves it (an empty value: its default), as
/// `aequalis --set key=value`: `{"ok":true,"key":…,"value":…}` or the error.
///
/// # Safety
/// Called by the JVM only: `env` is its `JNIEnv *`, the arrays its references, valid in this call.
#[no_mangle]
pub unsafe extern "system" fn Java_io_lucapf_aequalis_Core_set(env: Env, _class: Class, key: ByteArray, value: ByteArray) -> ByteArray {
    let reply = match (unsafe { text(env, key) }, unsafe { text(env, value) }) {
        (Some(key), Some(value)) => crate::ffi::set(&key, Some(value.as_str()).filter(|v| !v.is_empty())),
        _ => error("set: a key and a value"),
    };
    unsafe { to_java(env, &reply) }
}

/// `Core.seed(rates, cities)`: the data shipped with the app (`ffi::seed`): ExchangeRate-API's
/// answer (empty: none) and the path of the city archive's JSON file (empty: none). Off the main
/// thread: installing the archive takes a second or two.
///
/// # Safety
/// Called by the JVM only: `env` is its `JNIEnv *`, the arrays its references, valid in this call.
#[no_mangle]
pub unsafe extern "system" fn Java_io_lucapf_aequalis_Core_seed(env: Env, _class: Class, rates: ByteArray, cities: ByteArray) -> ByteArray {
    let rates = unsafe { text(env, rates) }.filter(|r| !r.is_empty());
    let cities = unsafe { text(env, cities) }.filter(|c| !c.is_empty());
    let reply = crate::ffi::seed(rates.as_deref(), cities.as_deref().map(std::path::Path::new));
    unsafe { to_java(env, &reply) }
}

/// `Core.reset()`: every setting back to its default (`ffi::reset`).
///
/// # Safety
/// Called by the JVM only: `env` is its `JNIEnv *`, valid in this call.
#[no_mangle]
pub unsafe extern "system" fn Java_io_lucapf_aequalis_Core_reset(env: Env, _class: Class) -> ByteArray {
    unsafe { to_java(env, &crate::ffi::reset()) }
}

thread_local! {
    /// The Java side of the sync running on this thread (`Core.sync`): its `JNIEnv *` and the
    /// `Core` object, whose `fetch` downloads.
    static JAVA: Cell<Option<(Env, Class)>> = const { Cell::new(None) };
}

/// `Core.sync(auto)`: downloads the online data (`ffi::sync`), in this thread: everything, or with
/// `auto` what the refresh settings say. Each download is `Core.fetch`'s, in Kotlin (a phone has no
/// `curl`).
///
/// # Safety
/// Called by the JVM only: `env` is its `JNIEnv *`, `core` the `Core` object, valid in this call.
#[no_mangle]
pub unsafe extern "system" fn Java_io_lucapf_aequalis_Core_sync(env: Env, core: Class, auto: u8) -> ByteArray {
    crate::ffi::aequalis_set_fetcher(fetch);
    JAVA.set(Some((env, core)));
    let reply = crate::ffi::sync(auto != 0);
    JAVA.set(None);
    unsafe { to_java(env, &reply) }
}

/// The app's downloader (`ffi::Fetch`): `Core.fetch(url, to, seconds, strict)`, through the Java
/// side of the sync running on this thread; `false` outside one, or when Java threw.
extern "C" fn fetch(url: *const c_char, to: *const c_char, seconds: u32, strict: bool) -> bool {
    let Some((env, core)) = JAVA.get() else { return false };
    // Safety: `url` and `to` are `AppFetcher`'s NUL-terminated strings; `env` and `core` are this
    // thread's, from `Core.sync`, which is still running.
    unsafe {
        let (Ok(url), Ok(to)) = (CStr::from_ptr(url).to_str(), CStr::from_ptr(to).to_str()) else { return false };
        let class_of: extern "system" fn(Env, Class) -> Class = function(env, GET_OBJECT_CLASS);
        let method_of: extern "system" fn(Env, Class, *const c_char, *const c_char) -> *mut c_void = function(env, GET_METHOD_ID);
        let call: extern "system" fn(Env, Class, *mut c_void, *const Value) -> u8 = function(env, CALL_BOOLEAN_METHOD_A);
        let delete: extern "system" fn(Env, *mut c_void) = function(env, DELETE_LOCAL_REF);
        let class = class_of(env, core);
        let method = method_of(env, class, c"fetch".as_ptr(), c"([B[BIZ)Z".as_ptr());
        delete(env, class);
        if method.is_null() {
            thrown(env);
            return false;
        }
        let (url, to) = (to_java(env, url), to_java(env, to));
        let seconds = i32::try_from(seconds).unwrap_or(i32::MAX);
        let args = [Value { object: url }, Value { object: to }, Value { int: seconds }, Value { boolean: u8::from(strict) }];
        let saved = call(env, core, method, args.as_ptr()) != 0;
        delete(env, url);
        delete(env, to);
        saved && !thrown(env)
    }
}

/// Whether Java threw (cleared: the call just failed).
///
/// # Safety
/// `env` comes from Java, in this thread.
unsafe fn thrown(env: Env) -> bool {
    unsafe {
        let check: extern "system" fn(Env) -> u8 = function(env, EXCEPTION_CHECK);
        let clear: extern "system" fn(Env) = function(env, EXCEPTION_CLEAR);
        let thrown = check(env) != 0;
        if thrown {
            clear(env);
        }
        thrown
    }
}
