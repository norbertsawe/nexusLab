#[derive(Default, Clone)]
pub struct Diagnostics {
    pub resident_bytes: Option<usize>,
    pub threads: Option<usize>,
}
#[cfg(windows)]
pub fn read() -> Diagnostics {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
                THREADENTRY32,
            },
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::{GetCurrentProcess, GetCurrentProcessId},
        },
    };
    // SAFETY: properly sized writable Win32 structures, a process pseudo-handle,
    // and an owned snapshot closed after enumeration. No pointer escapes.
    unsafe {
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let memory = if GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) != 0 {
            Some(counters.WorkingSetSize)
        } else {
            None
        };
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        let mut threads = None;
        if snapshot != INVALID_HANDLE_VALUE {
            let mut entry: THREADENTRY32 = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
            let pid = GetCurrentProcessId();
            let mut count = 0;
            let mut next = Thread32First(snapshot, &mut entry);
            while next != 0 {
                if entry.th32OwnerProcessID == pid {
                    count += 1;
                }
                next = Thread32Next(snapshot, &mut entry);
            }
            threads = Some(count);
            CloseHandle(snapshot);
        }
        Diagnostics {
            resident_bytes: memory,
            threads,
        }
    }
}
#[cfg(not(windows))]
pub fn read() -> Diagnostics {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let value = |prefix: &str| {
        status
            .lines()
            .find(|l| l.starts_with(prefix))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|n| n.parse::<usize>().ok())
    };
    Diagnostics {
        resident_bytes: value("VmRSS:").map(|v| v * 1024),
        threads: value("Threads:"),
    }
}
