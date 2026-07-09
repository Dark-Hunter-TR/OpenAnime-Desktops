// === OpenAnime — DPI Atlatma Yöntemleri ===
// Her farklı DPI atlatma stratejisini tanımlar

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MethodStatus {
    Untested,
    Working,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DpiMethodRecord {
    pub id: u32,
    pub status: MethodStatus,
    pub success_count: u32,
    pub fail_count: u32,
    pub first_success: Option<String>,
    pub last_tested: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DpiMethod {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub http_host_case: bool,
    pub http_host_mixedcase: bool,
    pub http_host_removespace: bool,
    pub http_fragment_size: u32,
    pub https_fragment_size: u32,
    pub fragment_by_sni: bool,
    pub reverse_fragment: bool,
}

fn make_methods() -> Vec<DpiMethod> {
    vec![
        DpiMethod { id: 1, name: "Host Case Change".into(), description: "Host: → hoSt: (en hafif)".into(), http_host_case: true, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 0, https_fragment_size: 0, fragment_by_sni: false, reverse_fragment: false },
        DpiMethod { id: 2, name: "HTTP Fragment 2".into(), description: "Sadece HTTP'yi 2 parçaya böl".into(), http_host_case: false, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 2, https_fragment_size: 0, fragment_by_sni: false, reverse_fragment: false },
        DpiMethod { id: 3, name: "HTTPS Fragment 2".into(), description: "Sadece TLS'yi 2 parçaya böl".into(), http_host_case: false, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 0, https_fragment_size: 2, fragment_by_sni: false, reverse_fragment: false },
        DpiMethod { id: 4, name: "HTTP+HTTPS Fragment 2".into(), description: "İkisini de 2 parçaya böl".into(), http_host_case: false, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 2, https_fragment_size: 2, fragment_by_sni: false, reverse_fragment: false },
        DpiMethod { id: 5, name: "SNI Bazlı Fragment".into(), description: "TLS SNI'den önce parçala".into(), http_host_case: false, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 0, https_fragment_size: 1, fragment_by_sni: true, reverse_fragment: false },
        DpiMethod { id: 6, name: "Reverse Fragment".into(), description: "Önce küçük, sonra büyük parça".into(), http_host_case: false, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 2, https_fragment_size: 2, fragment_by_sni: false, reverse_fragment: true },
        DpiMethod { id: 7, name: "Mixed Case + Fragment".into(), description: "Case + HTTP/HTTPS fragment".into(), http_host_case: true, http_host_mixedcase: true, http_host_removespace: false, http_fragment_size: 2, https_fragment_size: 2, fragment_by_sni: false, reverse_fragment: false },
        DpiMethod { id: 8, name: "Full (en agresif)".into(), description: "Tüm teknikler bir arada".into(), http_host_case: true, http_host_mixedcase: true, http_host_removespace: true, http_fragment_size: 2, https_fragment_size: 2, fragment_by_sni: true, reverse_fragment: true },
    ]
}

pub static ALL_METHODS: LazyLock<Vec<DpiMethod>> = LazyLock::new(make_methods);

pub fn get_method_by_id(id: u32) -> Option<&'static DpiMethod> {
    ALL_METHODS.iter().find(|m| m.id == id)
}

pub fn default_method_order() -> Vec<u32> {
    ALL_METHODS.iter().map(|m| m.id).collect()
}
