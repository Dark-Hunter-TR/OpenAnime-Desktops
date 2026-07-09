// === OpenAnime — Session Logger ===
// Oturum boyunca log'ları dosyaya yazar, tekrar eden satırları deduplicate eder.
// Konsol yokken (release build) hatayı görmek için kullanılır.
// Global static sayesinde her yerden erişilebilir.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;
use tauri::Manager;

// ===== Global Static Logger =====
// Her yerden `log!("mesaj")` ile erişilebilir

static LOGGER: std::sync::LazyLock<Mutex<SessionLoggerInner>> =
    std::sync::LazyLock::new(|| Mutex::new(SessionLoggerInner::new()));

/// Global makro — hem console'a yazar hem de log dosyasına
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        println!("{}", msg);
        $crate::logger::write_to_file(&msg);
    }};
}

/// Dosyaya yaz (makro içinden çağrılır)
pub fn write_to_file(line: &str) {
    if let Ok(mut logger) = LOGGER.lock() {
        logger.write(line);
    }
}

/// Log dosyasının içeriğini döndür (frontend için)
pub fn read_log() -> Result<String, String> {
    if let Ok(logger) = LOGGER.lock() {
        logger.read_content()
    } else {
        Err("Logger kitli".to_string())
    }
}

/// Logger'ı başlat (setup'ta çağrılır)
pub fn init(app: &tauri::AppHandle) {
    if let Ok(mut logger) = LOGGER.lock() {
        logger.init(app);
    }
}

// ===== Internal Logger =====

struct SessionLoggerInner {
    file: Option<File>,
    path: Option<PathBuf>,
    /// Son yazılan log satırı (dedup için)
    last_line: String,
    /// Son satırın kaç kere tekrarlandığı
    repeat_count: u32,
}

impl SessionLoggerInner {
    fn new() -> Self {
        Self {
            file: None,
            path: None,
            last_line: String::new(),
            repeat_count: 0,
        }
    }

    fn init(&mut self, app: &tauri::AppHandle) {
        let local_data = app
            .path()
            .app_local_data_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let log_dir = local_data.join("logs");
        let _ = std::fs::create_dir_all(&log_dir);

        let now = unix_secs();
        let timestamp = format_timestamp_compact(now);
        let log_path = log_dir.join(format!("session-{}.log", timestamp));

        match OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
        {
            Ok(file) => {
                self.file = Some(file);
                self.path = Some(log_path.clone());
                self.last_line.clear();
                self.repeat_count = 0;

                let header = format!(
                    "===== OpenAnime Session Log =====\n\
                     Başlangıç: {}\n\
                     Build: {}\n\
                     Platform: {}\n\
                     =================================\n",
                    format_timestamp(now),
                    if cfg!(debug_assertions) { "DEBUG" } else { "RELEASE" },
                    std::env::consts::OS
                );
                let _ = self.raw_write(&header);
                println!("[Logger] ✅ Log dosyası: {}", log_path.display());
            }
            Err(e) => {
                eprintln!("[Logger] ❌ Log dosyası açılamadı ({}): {}", log_path.display(), e);
            }
        }
    }

    /// Log yaz — dedup ile
    fn write(&mut self, line: &str) {
        let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');

        if self.last_line == trimmed {
            self.repeat_count += 1;
            // Grup bildirimi: sadece belirli aralıklarda
            match self.repeat_count {
                2 => self.raw_write(&format!("  └── (x2) {}", trimmed)),
                5 => self.raw_write(&format!("  └── (x{}) {}", self.repeat_count, trimmed)),
                10 => self.raw_write(&format!("  └── (x{}) {}", self.repeat_count, trimmed)),
                20 => self.raw_write(&format!("  └── (x{}) {}", self.repeat_count, trimmed)),
                50 => self.raw_write(&format!("  └── (x{}) {}", self.repeat_count, trimmed)),
                n if n > 0 && n % 100 == 0 => {
                    self.raw_write(&format!("  └── (x{}) {}", n, trimmed))
                }
                _ => {}
            }
            return;
        }

        // Önceki tekrar varsa kapat
        if self.repeat_count > 1 {
            self.raw_write(&format!("  └── (toplam x{} kez)", self.repeat_count));
        }

        // Yeni satır
        self.last_line = trimmed.to_string();
        self.repeat_count = 0;
        self.raw_write(trimmed);
    }

    /// Dosyaya direkt yaz (zaman damgalı)
    fn raw_write(&mut self, line: &str) {
        let ts = format_timestamp_ms(unix_secs(), unix_millis());
        if let Some(ref mut file) = self.file {
            let _ = writeln!(file, "[{}] {}", ts, line);
            let _ = file.flush();
        }
    }

    /// Log içeriğini oku
    fn read_content(&self) -> Result<String, String> {
        match &self.path {
            Some(p) => std::fs::read_to_string(p).map_err(|e| format!("Log okunamadı: {}", e)),
            None => Err("Log dosyası henüz oluşturulmadı".to_string()),
        }
    }
}

// ===== Zaman Damgası Yardımcıları =====

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn unix_millis() -> u32 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_millis()
}

/// Yerel zaman (UTC+3) dönüşümü
fn to_local(secs: u64) -> (i64, u64) {
    let local = secs as i64 + 3 * 3600;
    if local < 0 {
        (0, 0)
    } else {
        (local / 86400, (local % 86400) as u64)
    }
}

fn format_timestamp(unix_secs: u64) -> String {
    let (days, time) = to_local(unix_secs);
    let hours = time / 3600;
    let minutes = (time % 3600) / 60;
    let seconds = time % 60;
    let (y, m, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, hours, minutes, seconds)
}

fn format_timestamp_compact(unix_secs: u64) -> String {
    let (days, time) = to_local(unix_secs);
    let hours = time / 3600;
    let minutes = (time % 3600) / 60;
    let seconds = time % 60;
    let (y, m, d) = days_to_ymd(days);
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", y, m, d, hours, minutes, seconds)
}

fn format_timestamp_ms(unix_secs: u64, millis: u32) -> String {
    let (_, time) = to_local(unix_secs);
    let hours = time / 3600;
    let minutes = (time % 3600) / 60;
    let seconds = time % 60;
    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
}

fn days_to_ymd(days: i64) -> (i64, u32, u32) {
    let mut y = 1970i64;
    let mut remaining = days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let month_days = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md {
            m = (i + 1) as u32;
            break;
        }
        remaining -= md;
    }
    (y, m, (remaining + 1) as u32)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// ===== Tauri Komutu =====

#[tauri::command]
pub async fn get_session_log() -> Result<String, String> {
    read_log()
}
