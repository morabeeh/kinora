//! Installed RAM and this process's memory use.

/// Memory use of the Kinora process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessMemory {
    /// Resident set size (Windows: working set), in bytes.
    pub resident_bytes: u64,
    /// Peak resident set size so far, in bytes (0 if unknown).
    pub peak_resident_bytes: u64,
}

/// Installed physical RAM in bytes, if the OS tells us.
#[must_use]
pub fn total_ram_bytes() -> Option<u64> {
    #[cfg(windows)]
    {
        crate::sys_windows::total_ram_bytes()
    }
    #[cfg(target_os = "linux")]
    {
        crate::sys_linux::total_ram_bytes()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None // macOS arrives in v1.1; the tier rules assume 4 GiB when unknown
    }
}

/// This process's current and peak memory use.
#[must_use]
pub fn process_memory() -> Option<ProcessMemory> {
    #[cfg(windows)]
    {
        crate::sys_windows::process_memory()
    }
    #[cfg(target_os = "linux")]
    {
        crate::sys_linux::process_memory()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(windows, target_os = "linux"))]
    fn ram_and_process_memory_are_reported() {
        let ram = total_ram_bytes().unwrap();
        assert!(ram > 256 * 1024 * 1024, "{ram}");
        let mine = process_memory().unwrap();
        assert!(mine.resident_bytes > 0);
        assert!(mine.resident_bytes < ram);
    }
}
