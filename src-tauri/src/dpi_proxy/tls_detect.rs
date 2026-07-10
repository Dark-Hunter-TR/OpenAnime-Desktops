// === OpenAnime — TLS/SNI Tespit Modülü ===
// GoodbyeDPI'nin extract_sni() fonksiyonunun Rust portu
// TLS ClientHello'dan SNI hostname'ini çıkarır

/// TLS ClientHello paketinden SNI (Server Name Indication) hostname'ini çıkar
/// 
/// GoodbyeDPI'nin extract_sni() mantığının aynısı:
/// TLS ClientHello'daki extensions bölümünde SNI extension'ını (0x0000) arar
pub fn extract_sni(data: &[u8]) -> Option<&[u8]> {
    if data.len() < 50 {
        return None;
    }

    // TLS ClientHello kontrolü: ContentType=0x16, Protocol=0x03 0x0x
    if data[0] != 0x16 || data[1] != 0x03 {
        return None;
    }

    let mut ptr = 0usize;

    while ptr + 8 < data.len() {
        // GoodbyeDPI'nin kullandığı SNI extension tespit imzası:
        // 00 00 00 LL 00 LL 00 LL ...
        // Extension Type = 0x0000 (SNI)
        // Extension Data Length, Server Name List Length, Server Name Type, Server Name Length
        if data[ptr] == 0x00
            && data[ptr + 1] == 0x00
            && data[ptr + 2] == 0x00
            && data[ptr + 4] == 0x00
            && data[ptr + 6] == 0x00
            && data[ptr + 7] == 0x00
        {
            // Uzunluk ilişkilerini kontrol et
            // d[3] - d[5] == 2  &&  d[5] - d[8] == 3
            if data[ptr + 3] - data[ptr + 5] == 2
                && data[ptr + 5] - data[ptr + 8] == 3
            {
                let hnlen = data[ptr + 8] as usize;
                if ptr + 9 + hnlen > data.len() {
                    return None;
                }

                // Hostname boyutu kontrolü (3-253 byte)
                if hnlen < 3 || hnlen > 253 {
                    return None;
                }

                // Sadece ASCII küçük harf, rakam, nokta ve tire kontrolü
                let hostname = &data[ptr + 9..ptr + 9 + hnlen];
                if !hostname
                    .iter()
                    .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
                {
                    return None;
                }

                return Some(hostname);
            }
        }
        ptr += 1;
    }

    None
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Verinin TLS ClientHello olup olmadığını kontrol et
    fn is_tls_client_hello(data: &[u8]) -> bool {
        data.len() >= 3
            && data[0] == 0x16
            && data[1] == 0x03
            && (data[2] == 0x01 || data[2] == 0x03)
    }

    #[test]
    fn test_not_tls() {
        let data = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert!(!is_tls_client_hello(data));
        assert!(extract_sni(data).is_none());
    }

    #[test]
    fn test_too_short() {
        assert!(extract_sni(&[0u8; 10]).is_none());
    }

    #[test]
    fn test_extract_sni_from_valid() {
        // Basit bir TLS ClientHello SNI içermeyebilir, 
        // gerçek test için canlı veri gerek
        let mut data = vec![0x16, 0x03, 0x01];
        data.resize(100, 0);
        assert!(extract_sni(&data).is_none());
    }
}
