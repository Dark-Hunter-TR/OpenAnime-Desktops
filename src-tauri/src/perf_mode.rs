// === OpenAnime — Performans / Verimlilik Modu ===
//
// İki AYRI mekanizmayı birlikte yönetir. Karıştırılmamaları önemli:
//
//  1) SetMemoryUsageTargetLevel (WebView2 API'si) → BELLEĞİ etkiler.
//     Chromium'a "cache'lerini agresif at, az bellek hedefle" der.
//
//  2) EcoQoS / PROCESS_POWER_THROTTLING (Windows API'si) → CPU ve GÜCÜ etkiler.
//     Task Manager'da yeşil yaprak = "verimlilik modu". BELLEĞİ AZALTMAZ.
//     Sadece CPU önceliğini/frekansını düşürür, pil ömrünü uzatır.
//
// Kural:
//   Oynatıcıda video oynuyor  → NORMAL bellek + throttling YOK  (tam performans)
//   Diğer her durum           → LOW bellek + EcoQoS             (verimlilik)
//
// WebView2 alt süreçlerini bulma notu:
//   msedgewebview2.exe süreçleri openanime.exe'nin ÇOCUĞU DEĞİLDİR — süreç
//   ağacından gidilemez (ölçümle doğrulandı: openanime.exe'nin 0 çocuğu var).
//   Ama WebView2'nin *browser* süreci, renderer/GPU/utility süreçlerinin
//   ebeveynidir. Bu yüzden: WebView2'den BrowserProcessId'yi al, sonra
//   Toolhelp32 ile ppid == browser_pid olanları topla.

#![cfg(target_os = "windows")]

use std::sync::Mutex;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, SetPriorityClass, SetProcessInformation, IDLE_PRIORITY_CLASS,
    NORMAL_PRIORITY_CLASS, PROCESS_INFORMATION_CLASS, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
    PROCESS_SET_INFORMATION,
};

use crate::log;

/// ProcessPowerThrottling — windows crate'te sabit olarak yok, ham değer 4.
const PROCESS_POWER_THROTTLING: PROCESS_INFORMATION_CLASS = PROCESS_INFORMATION_CLASS(4);

/// Son uygulanan mod — gereksiz tekrar çağrıları elemek için.
/// (Her video timeupdate'inde API çağırmak istemiyoruz.)
static CURRENT_MODE: Mutex<Option<bool>> = Mutex::new(None);

/// Verilen süreç için EcoQoS'u aç/kapat.
///
/// enable=true  → EXECUTION_SPEED throttling AÇIK  + IDLE önceliği (verimlilik)
/// enable=false → throttling KAPALI (sistem karar versin) + NORMAL öncelik
fn set_eco_qos(pid: u32, enable: bool) -> bool {
    unsafe {
        let handle: HANDLE = match OpenProcess(PROCESS_SET_INFORMATION, false, pid) {
            Ok(h) => h,
            // Erişim reddi normal olabilir (yükseltilmiş süreç vb.) — sessiz geç.
            Err(_) => return false,
        };

        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            // ControlMask: hangi politikayı yönettiğimiz
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
            // StateMask: 0 = throttling KAPALI, EXECUTION_SPEED = AÇIK
            StateMask: if enable {
                PROCESS_POWER_THROTTLING_EXECUTION_SPEED
            } else {
                0
            },
        };

        let ok = SetProcessInformation(
            handle,
            PROCESS_POWER_THROTTLING,
            &state as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        )
        .is_ok();

        // Task Manager'ın "verimlilik modu" göstergesi EcoQoS + düşük önceliğin
        // BİRLİKTE olmasını bekler; sadece biri yeşil yaprağı göstermez.
        let _ = SetPriorityClass(
            handle,
            if enable {
                IDLE_PRIORITY_CLASS
            } else {
                NORMAL_PRIORITY_CLASS
            },
        );

        let _ = CloseHandle(handle);
        ok
    }
}

/// browser_pid'in kendisi + tüm çocukları (renderer, GPU, utility...).
fn webview_process_tree(browser_pid: u32) -> Vec<u32> {
    let mut out = vec![browser_pid];
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(s) => s,
            Err(_) => return out,
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32ParentProcessID == browser_pid {
                    out.push(entry.th32ProcessID);
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    out
}

/// Tüm WebView2 süreç ağacına verimlilik modunu uygula.
/// `low` = true → verimlilik (EcoQoS açık), false → tam performans.
///
/// HER ÇAĞRIDA tüm süreçlere yeniden uygulanır — "mod değişmedi, atla" KISAYOLU
/// YAPILMAZ. Sebep ölçümle bulundu: WebView2 sonradan yeni süreç doğuruyor
/// (ör. Cloudflare Turnstile iframe'i kendi renderer'ını açıyor). Erken dönseydik
/// bu süreçler EcoQoS'suz kalırdı — nitekim ilk sürümde tam bu oldu:
/// 8 süreçten 2'si kapsam dışı kaldı. Çağrı ucuz (birkaç OpenProcess), idempotent.
pub fn apply_eco_mode(browser_pid: u32, low: bool) {
    let pids = webview_process_tree(browser_pid);
    let mut ok = 0;
    for pid in &pids {
        if set_eco_qos(*pid, low) {
            ok += 1;
        }
    }

    // Log'u yalnızca mod GERÇEKTEN değişince bas — periyodik yenileme
    // her 10 sn'de bir satır üretmesin.
    let mut cur = CURRENT_MODE.lock().unwrap();
    if *cur != Some(low) {
        *cur = Some(low);
        log!(
            "[PerfMode] EcoQoS {} → {}/{} süreç (browser pid {})",
            if low { "AÇIK (verimlilik)" } else { "KAPALI (tam performans)" },
            ok,
            pids.len(),
            browser_pid
        );
    }
}
