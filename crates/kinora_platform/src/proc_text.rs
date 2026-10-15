//! Parsing of Linux `/proc` text files (`/proc/meminfo`, `/proc/self/status`).

/// Reads a `Key:   12345 kB` line and returns the value in bytes.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))] // only called on Linux; tested everywhere
pub fn kib_field(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix(key)?.strip_prefix(':')?;
        let mut parts = rest.split_whitespace();
        let value: u64 = parts.next()?.parse().ok()?;
        match parts.next() {
            Some("kB") | None => value.checked_mul(1024),
            Some(_) => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMINFO: &str =
        "MemTotal:       16314256 kB\nMemFree:         1234567 kB\nHugePages_Total:       0\n";
    const STATUS: &str = "Name:\tkinora\nVmHWM:\t  204800 kB\nVmRSS:\t  102400 kB\n";

    #[test]
    fn reads_kib_values() {
        assert_eq!(kib_field(MEMINFO, "MemTotal"), Some(16_314_256 * 1024));
        assert_eq!(kib_field(STATUS, "VmRSS"), Some(102_400 * 1024));
        assert_eq!(kib_field(STATUS, "VmHWM"), Some(204_800 * 1024));
        assert_eq!(kib_field(MEMINFO, "HugePages_Total"), Some(0));
    }

    #[test]
    fn missing_or_garbage_is_none() {
        assert_eq!(kib_field(MEMINFO, "SwapTotal"), None);
        assert_eq!(kib_field("MemTotal: lots kB", "MemTotal"), None);
        assert_eq!(kib_field("MemTotal: 5 MB", "MemTotal"), None);
        assert_eq!(kib_field("MemTotalX: 5 kB", "MemTotal"), None);
    }
}
