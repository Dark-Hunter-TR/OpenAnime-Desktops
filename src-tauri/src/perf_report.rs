// === OpenAnime — Periyodik Performans Raporu ===
//
// Her dakika terminale/log dosyasına tek satırlık bir özet basar: uygulamanın
// (ana süreç + tüm WebView2 alt süreçleri: renderer/GPU/utility) toplam RAM
// kullanımı, CPU kullanım yüzdesi ve her pencerenin o anki arka plan durumu
// (normal / Media-uyku / DeepSleep). Sadece gözlem amaçlı — hiçbir kararı
// etkilemez.

#![cfg(target_os = "windows")]

use std::sync::{Arc, Mutex};
use std::time::Instant;

use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
};

use crate::{log, perf_mode, BgMode, PerfState};

fn filetime_to_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

/// Bir süreç için (working-set RAM baytı, kernel+user CPU zamanı 100ns birim).
fn sample_process(pid: u32) -> Option<(u64, u64)> {
    unsafe {
        let handle: HANDLE =
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, false, pid).ok()?;

        let mut mem = PROCESS_MEMORY_COUNTERS::default();
        let ram = if GetProcessMemoryInfo(
            handle,
            &mut mem,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .is_ok()
        {
            mem.WorkingSetSize as u64
        } else {
            0
        };

        let (mut creation, mut exit, mut kernel, mut user) =
            (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let cpu = if GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user).is_ok() {
            filetime_to_u64(kernel) + filetime_to_u64(user)
        } else {
            0
        };

        let _ = CloseHandle(handle);
        Some((ram, cpu))
    }
}

/// Bir önceki örneklemenin (an, toplam CPU zamanı) — yüzdeyi delta üzerinden
/// hesaplamak için. Anlık GetProcessTimes tek başına yüzde vermez, iki
/// örnekleme arasındaki FARK gerekir.
static LAST_SAMPLE: Mutex<Option<(Instant, u64)>> = Mutex::new(None);

/// Bir pencerenin `BgMode`'unu okunaklı Türkçeye çevirir.
fn mode_label(mode: BgMode) -> &'static str {
    match mode {
        BgMode::Foreground => "normal",
        BgMode::Media => "sleep (video oynuyor)",
        BgMode::DeepSleep => "sleep (donmuş)",
    }
}

/// RAM + CPU + pencere durumlarını tek satır log olarak basar.
pub fn report(app: &AppHandle) {
    // Ana süreç + her pencerenin WebView2 browser süreci ve onun tüm
    // çocukları (renderer/GPU/utility) — perf_mode.rs'teki yaklaşımla aynı.
    let mut pids = vec![std::process::id()];

    let browser_pids: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
    for (_, win) in app.webview_windows() {
        let bp = browser_pids.clone();
        let _ = win.with_webview(move |webview| unsafe {
            use windows_core::Interface;
            let controller = webview.controller();
            if Interface::as_raw(&controller).is_null() {
                return;
            }
            if let Ok(core) = controller.CoreWebView2() {
                let mut pid: u32 = 0;
                if core.BrowserProcessId(&mut pid).is_ok() && pid != 0 {
                    if let Ok(mut v) = bp.lock() {
                        v.push(pid);
                    }
                }
            }
        });
    }
    let browsers: Vec<u32> = browser_pids.lock().map(|v| v.clone()).unwrap_or_default();
    for bpid in &browsers {
        pids.extend(perf_mode::webview_process_tree(*bpid));
    }
    pids.sort_unstable();
    pids.dedup();

    let mut total_ram: u64 = 0;
    let mut total_cpu: u64 = 0;
    let mut sampled = 0;
    for pid in &pids {
        if let Some((ram, cpu)) = sample_process(*pid) {
            total_ram += ram;
            total_cpu += cpu;
            sampled += 1;
        }
    }

    let now = Instant::now();
    let cpu_pct = {
        let mut last = LAST_SAMPLE.lock().unwrap();
        let pct = match *last {
            Some((prev_t, prev_cpu)) if total_cpu >= prev_cpu => {
                let wall_100ns = now.duration_since(prev_t).as_nanos() as f64 / 100.0;
                let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
                if wall_100ns > 0.0 {
                    ((total_cpu - prev_cpu) as f64 / wall_100ns / cores * 100.0).clamp(0.0, 100.0 * cores)
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };
        *last = Some((now, total_cpu));
        pct
    };

    let ram_mb = total_ram as f64 / (1024.0 * 1024.0);

    let state_str = {
        let st = app.state::<PerfState>();
        let map = st.suspended.lock().unwrap();
        if map.is_empty() {
            "normal".to_string()
        } else {
            map.iter()
                .map(|(label, mode)| format!("{}={}", label, mode_label(*mode)))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };

    log!(
        "[Perf] RAM {:.0} MB · CPU %{:.1} · {} süreç · Durum: {}",
        ram_mb,
        cpu_pct,
        sampled,
        state_str
    );
}
