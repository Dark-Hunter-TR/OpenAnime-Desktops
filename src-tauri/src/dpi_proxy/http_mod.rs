// === OpenAnime — HTTP Header Manipülasyon Modülü ===
// GoodbyeDPI'nin Host: → hoSt: dönüşümü ve header manipülasyonu

/// Host header'ını bul ve case değişimi yap (Host: → hoSt:)
/// GoodbyeDPI'nin `-r` flag'inin Rust portu
pub fn replace_host_with_host(data: &mut [u8]) -> bool {
    if let Some(pos) = find_host_header(data) {
        // "Host: " (6 byte) → "hoSt: " (6 byte)
        // İlk 2 harfi değiştir: "Ho" → "ho", üçüncü "s" → "S"
        if pos + 6 <= data.len() {
            data[pos] = b'h';     // H → h
            data[pos + 1] = b'o';  // o → o (aynı)
            data[pos + 2] = b'S';  // s → S
            data[pos + 3] = b't';  // t → t (aynı)
            // ": " aynı kalır
            return true;
        }
    }
    false
}

/// Host header'ının değerini mixed case yap (tEsT.cOm)
/// GoodbyeDPI'nin `-m` flag'inin Rust portu
pub fn mix_host_case(data: &mut [u8]) -> bool {
    // "Host: " veya "hoSt: " sonrasındaki değeri bul
    let patterns = [b"\r\nHost: " as &[u8], b"\r\nhoSt: " as &[u8]];

    for pattern in &patterns {
        if let Some(pos) = find_pattern(data, pattern) {
            let value_start = pos + pattern.len();
            // Değerin sonu \r\n'ye kadar
            if let Some(value_end) = find_pattern(&data[value_start..], b"\r\n") {
                let end = value_start + value_end;
                for i in (value_start..end).step_by(2) {
                    if i < end {
                        data[i] = data[i].to_ascii_uppercase();
                    }
                }
                return true;
            }
        }
    }
    false
}

/// "Host:" ve "User-Agent" arasında space taşıma
/// GoodbyeDPI'nin `-s` flag'inin Rust portu (basitleştirilmiş)
pub fn remove_host_space(data: &mut [u8]) -> bool {
    // Host header'ını bul
    if let Some(host_pos) = find_host_header_or_host(data) {
        // "Host:" veya "hoSt:" pattern'ini bul
        let colon_pos = if let Some(p) = find_pattern(&data[host_pos..], b":") {
            host_pos + p
        } else {
            return false;
        };

        // İki noktadan sonraki ilk byte boşluksa
        let space_pos = colon_pos + 1;
        if space_pos < data.len() && data[space_pos] == b' ' {
            // User-Agent header'ını bul
            if let Some(ua_pos) = find_pattern(data, b"\r\nUser-Agent: ") {
                if ua_pos > space_pos {
                    // User-Agent'in sonuna boşluk ekle
                    let ua_value_end = ua_pos + 14; // "\r\nUser-Agent: " sonrası
                    if ua_value_end < data.len() {
                        // Host: değerini bir byte sola kaydır
                        data.copy_within(space_pos + 1.., space_pos);
                        // User-Agent değerinin sonuna boşluk ekle
                        if ua_value_end < data.len() {
                            data[ua_value_end] = b' ';
                        }
                        return true;
                    }
                }
            }
        }
    }
    false
}

// === Yardımcı Fonksiyonlar ===

/// "\r\nHost: " veya "\r\nhoSt: " pattern'ini ara
fn find_host_header(data: &[u8]) -> Option<usize> {
    find_pattern(data, b"\r\nHost: ")
        .or_else(|| find_pattern(data, b"\r\nhoSt: "))
}

/// "\r\nHost" veya "\r\nhoSt" pattern'ini ara (iki nokta olmadan)
fn find_host_header_or_host(data: &[u8]) -> Option<usize> {
    find_pattern(data, b"\r\nHost")
        .or_else(|| find_pattern(data, b"\r\nhoSt"))
}

/// Basit pattern arama (memmem)
fn find_pattern(data: &[u8], pattern: &[u8]) -> Option<usize> {
    if pattern.is_empty() || pattern.len() > data.len() {
        return None;
    }
    data.windows(pattern.len())
        .position(|window| window == pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_host() {
        let mut data = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec();
        assert!(replace_host_with_host(&mut data));
        assert_eq!(
            String::from_utf8_lossy(&data),
            "GET / HTTP/1.1\r\nhoSt: example.com\r\n\r\n"
        );
    }

    #[test]
    fn test_no_host_header() {
        let mut data = b"GET / HTTP/1.1\r\n\r\n".to_vec();
        assert!(!replace_host_with_host(&mut data));
    }

    #[test]
    fn test_mix_host_case() {
        let mut data = b"GET / HTTP/1.1\r\nHost: test.com\r\n\r\n".to_vec();
        assert!(mix_host_case(&mut data));
        // tEsT.cOm pattern'ini kontrol et
        let result = String::from_utf8_lossy(&data);
        assert!(result.contains("TeSt.cOm") || result.contains("tEsT.cOm"));
    }

    #[test]
    fn test_find_pattern() {
        let data = b"abc\r\nHost: value\r\nxyz";
        assert_eq!(find_pattern(data, b"\r\nHost: "), Some(3));
        assert_eq!(find_pattern(data, b"\r\nHOST: "), None);
        assert_eq!(find_pattern(data, b"xyz"), Some(18));
    }
}
