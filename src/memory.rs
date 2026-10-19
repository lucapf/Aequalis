//! This process's memory, for About, `--json` and the interactive mode: the peak resident memory so
//! far and, where the system says, the memory now.

/// The peak resident memory of this process so far, in KiB (0 if unknown). On Linux `VmHWM` from
/// `/proc/self/status`: `getrusage`'s `ru_maxrss` survives `exec`, so a process started by a large
/// one (the Omarchy shell, ~360 MiB) would report its parent's peak.
pub fn peak_kb() -> u64 {
    if cfg!(target_os = "linux") {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            let kb = status.lines().find_map(|l| l.strip_prefix("VmHWM:")?.trim().strip_suffix("kB")?.trim().parse().ok());
            if let Some(kb) = kb {
                return kb;
            }
        }
    }
    maxrss_kb()
}

/// The resident memory of this process now, in KiB (Linux: `VmRSS`); `None` where it is not
/// known (then the peak is shown instead).
#[cfg(not(windows))]
pub fn current_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|l| l.strip_prefix("VmRSS:")?.trim().strip_suffix("kB")?.trim().parse().ok())
}

/// The working set now, in KiB.
#[cfg(windows)]
pub fn current_kb() -> Option<u64> {
    crate::windows::memory_kb().map(|(_, now)| now)
}

/// KiB for people: `900 KiB`, `3.4 MiB`.
pub fn size(kb: u64) -> String {
    match kb {
        0..=1023 => format!("{kb} KiB"),
        _ => format!("{:.1} MiB", kb as f64 / 1024.0),
    }
}

/// The peak working set, in KiB.
#[cfg(windows)]
fn maxrss_kb() -> u64 {
    crate::windows::memory_kb().map_or(0, |(peak, _)| peak)
}

/// `getrusage`'s peak resident memory, in KiB.
#[cfg(not(windows))]
fn maxrss_kb() -> u64 {
    // `struct rusage`: two `struct timeval` (16 bytes each on 64-bit Linux and macOS), then
    // `ru_maxrss` and 13 more longs.
    #[repr(C)]
    struct Rusage {
        times: [i64; 4],
        maxrss: i64,
        rest: [i64; 13],
    }
    const _: () = assert!(std::mem::size_of::<Rusage>() == 144);
    extern "C" {
        fn getrusage(who: i32, usage: *mut Rusage) -> i32;
    }
    const RUSAGE_SELF: i32 = 0;
    let mut usage = Rusage { times: [0; 4], maxrss: 0, rest: [0; 13] };
    // SAFETY: `usage` is a writable `struct rusage`.
    if unsafe { getrusage(RUSAGE_SELF, &mut usage) } != 0 {
        return 0;
    }
    let maxrss = u64::try_from(usage.maxrss).unwrap_or(0);
    // Linux reports KiB, macOS bytes.
    if cfg!(target_os = "macos") {
        maxrss / 1024
    } else {
        maxrss
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_now() {
        assert_eq!(size(900), "900 KiB");
        assert_eq!(size(3482), "3.4 MiB");
        if cfg!(target_os = "linux") {
            // Two counters read a moment apart, while other tests allocate: a little slack.
            let now = current_kb().expect("VmRSS on Linux");
            assert!(now > 0 && now <= peak_kb() + 1024, "{now} <= {} + 1024", peak_kb());
        }
    }
}
