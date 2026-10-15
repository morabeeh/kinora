//! Windows API calls, each wrapped in a safe function.

use kinora_core::hardware::Gpu;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::LibraryLoader::LoadLibraryW;
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetProcessAffinityMask,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};

use crate::memory::ProcessMemory;

/// Installed physical RAM in bytes.
pub fn total_ram_bytes() -> Option<u64> {
    let mut status = MEMORYSTATUSEX {
        dwLength: u32::try_from(size_of::<MEMORYSTATUSEX>()).ok()?,
        dwMemoryLoad: 0,
        ullTotalPhys: 0,
        ullAvailPhys: 0,
        ullTotalPageFile: 0,
        ullAvailPageFile: 0,
        ullTotalVirtual: 0,
        ullAvailVirtual: 0,
        ullAvailExtendedVirtual: 0,
    };
    // SAFETY: `status` is a valid, writable MEMORYSTATUSEX with `dwLength` set as the API
    // requires; the call only writes into it.
    let ok = unsafe { GlobalMemoryStatusEx(&raw mut status) };
    (ok != 0).then_some(status.ullTotalPhys)
}

/// Working set (≈ RSS) and its peak for this process.
pub fn process_memory() -> Option<ProcessMemory> {
    let size = u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS>()).ok()?;
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: size,
        PageFaultCount: 0,
        PeakWorkingSetSize: 0,
        WorkingSetSize: 0,
        QuotaPeakPagedPoolUsage: 0,
        QuotaPagedPoolUsage: 0,
        QuotaPeakNonPagedPoolUsage: 0,
        QuotaNonPagedPoolUsage: 0,
        PagefileUsage: 0,
        PeakPagefileUsage: 0,
    };
    // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no closing; `counters`
    // is a valid, writable struct whose size is passed in `cb`.
    let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &raw mut counters, size) };
    (ok != 0).then(|| ProcessMemory {
        resident_bytes: u64::try_from(counters.WorkingSetSize).unwrap_or(u64::MAX),
        peak_resident_bytes: u64::try_from(counters.PeakWorkingSetSize).unwrap_or(u64::MAX),
    })
}

/// Number of logical CPUs this process may run on. Unlike
/// `std::thread::available_parallelism`, this respects the process affinity mask, so
/// `start /affinity 3 kinora.exe` (the Lite-tier simulation) reports 2 cores.
pub fn affinity_cores() -> Option<usize> {
    let mut process_mask: usize = 0;
    let mut system_mask: usize = 0;
    let process: HANDLE = {
        // SAFETY: returns a constant pseudo-handle; no preconditions.
        unsafe { GetCurrentProcess() }
    };
    // SAFETY: both out-pointers point to valid, writable `usize` locals.
    let ok =
        unsafe { GetProcessAffinityMask(process, &raw mut process_mask, &raw mut system_mask) };
    if ok == 0 || process_mask == 0 {
        return None;
    }
    usize::try_from(process_mask.count_ones()).ok()
}

/// The Remote Desktop / fast-user-switching session this process runs in.
pub fn session_id() -> Option<u32> {
    let mut session = 0u32;
    // SAFETY: GetCurrentProcessId has no preconditions; `session` is a valid out-pointer.
    let ok = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &raw mut session) };
    (ok != 0).then_some(session)
}

/// Lets whichever process we message next bring its window to the front. Windows only
/// allows the foreground app to hand this permission on; a just-launched instance is the
/// foreground app, so it grants it before forwarding a file to the running instance.
pub fn allow_foreground_activation() {
    // SAFETY: plain call without pointers; failure only means the other window may flash
    // in the taskbar instead of coming to the front.
    let ok = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
    if ok == 0 {
        tracing::debug!("AllowSetForegroundWindow failed (not the foreground process)");
    }
}

/// Loads a DLL by name (normal Windows search order) and keeps it loaded. Used before the
/// first call into a delay-loaded DLL such as `heif.dll`: if this fails, Kinora reports
/// "not available" instead of crashing on the first call.
pub fn load_library(name: &str) -> bool {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call. Loading runs
    // the DLL's initialisation code; we only load libraries Kinora itself links against.
    let module = unsafe { LoadLibraryW(wide.as_ptr()) };
    // The module stays loaded for the rest of the process on purpose (no FreeLibrary).
    !module.is_null()
}

/// PCI vendor and device id of WARP ("Microsoft Basic Render Driver").
const WARP_IDS: (u32, u32) = (0x1414, 0x008C);

/// Finds out whether a hardware GPU exists by listing DXGI adapters. A PC with only the
/// "Microsoft Basic Display Adapter" driver (no GPU driver installed, many VMs) exposes only
/// WARP, Microsoft's software rasteriser.
pub fn gpu() -> Gpu {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1,
    };
    // SAFETY: CreateDXGIFactory1 has no preconditions; the COM object it returns is
    // reference-counted by the `windows` crate and released when dropped.
    let factory = match unsafe { CreateDXGIFactory1::<IDXGIFactory1>() } {
        Ok(factory) => factory,
        Err(err) => {
            tracing::warn!(%err, "could not create a DXGI factory; GPU unknown");
            return Gpu::Unknown;
        }
    };
    let software_flag = u32::try_from(DXGI_ADAPTER_FLAG_SOFTWARE.0).unwrap_or(2);
    let mut software: Option<String> = None;
    for index in 0u32..64 {
        // SAFETY: any index is valid; past the last adapter it returns DXGI_ERROR_NOT_FOUND.
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(index) }) else {
            break;
        };
        // SAFETY: GetDesc1 only fills a struct owned by the returned value.
        let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
            continue;
        };
        let name_len = desc
            .Description
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(desc.Description.len());
        let name = String::from_utf16_lossy(desc.Description.get(..name_len).unwrap_or_default());
        let is_software = desc.Flags & software_flag != 0
            || (desc.VendorId, desc.DeviceId) == WARP_IDS
            || kinora_core::hardware::is_software_renderer_name(&name);
        if !is_software {
            return Gpu::Hardware(Some(name));
        }
        software.get_or_insert(name);
    }
    match software {
        Some(name) => Gpu::SoftwareOnly(Some(name)),
        None => Gpu::Unknown,
    }
}
