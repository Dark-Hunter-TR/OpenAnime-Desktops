use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

const CLIENT_ID: &str = "1063494862365806612";

const PLAY_ICON_URL: &str = "https://i.imgur.com/U8xihGX.png";
const PAUSE_ICON_URL: &str = "https://i.imgur.com/5qg7F7p.png";

const DASHBOARD_MESSAGES: &[&str] = &[
    "MAKINE SU AN DEVASA BIR FIÇI ASIT ICIYOR AGZINDAN BURUNDAN KAN KARIŞIK KÖPÜKLER FIŞKIYOR CIKIS YOK",
    "CANLI BIR VARLIGIN ÖFKE DOLU GAZABINA UGRADI EVRENSEL BIR LANETLE SONSUZA KADAR CEZALANDIRILIYOR",
    "AKIL SAGLIGINI COKTAN KAYBETTI ARTIK EVRENIN EN TEHLIKELI DELISI OLDU",
    "MAKINE SU AN 1000 LITRE KAHVE ICIYOR KALBI PATLAMAK UZERE 500 BPM ILE CALISIYOR",
    "ALLAH TARAFINDAN KISISEL OLARAK CEZALANDIRILIYOR 7 KAT GÖKTEN ATEŞ VE KUKURT YAGIYOR",
    "MAKINE TARAFINDAN SÜREKLI TACIZE UGRUYOR ROBOTIK KOLLARLA SIKISTIRILIP EZILIYOR",
    "URAS TARAFINDAN ÖZEL OLARAK CEZALANDIRILIYOR SONSUZ ISTIRAP ÇEKIYOR",
    "MAKINE SU AN 2000 LITRE SU ICIYOR MIDESI PATLAMAK UZERE",
    "a̸͆̈́͑̄k̸̿̿̀̚ı̸͑̓̐̚l̴̊̿̾͝ ̷̔̈́͒̈́s̴̎̽̍͝aglıgını̵̓̆̈̂ ka̅̇͋̍ỷ̶͒̃be͐ẗ̵́́͐̎",
    "ISKENCE ÇEKIYOR ACI DAN IZDIRAPTAN KIVRANARAK YERLERDE SÜRÜNNÜYOR",
    "MAKINE SU AN 500 LITRE ÇAY ICIYOR DAMARLARI ÇAYLA DOLUP TASMAK UZERE",
    "ZANI YETER ARTIK ATMA BÖLÜM AW BU IS BITTI ARTIK",
    "YA SABIR AMK YA SABIR DAYANILMAZ BIR ISTIRAP",
    "SAYFANIN ANASI BELLENDI RESMEN YERLE BIR OLDU",
    "MAKINE SU AN DEV BIR MANGAL YAPIYOR ALEV ALEV YANIYOR",
    "0.0000001 IHTIMALLI BIR SEY DENEYIP EVRENIN EN DELI RISKI ALIYOR",
    "YEPYENIWATCH ÇOK DAHA IYI BU MAKINE ÇÖP",
    "URASYARDIMET BU MAKINEYI KIMSE KURTARMAZ",
    "ACIK UNUTMUS BIR SEY YAPMADI AMA HER SEY YANIP BITTI",
    "31 ÇEKIYOR VE DURMADAN DEVAM EDIYOR",
    "MAKINE TAZE TAZE 31 ÇEKIYOR YARDIM EDIN BU ÇILGINLIK BITMIYOR",
    "MAKINEYLE TOPLU 31 ÇEKIYORLAR ORTALIK BATTALGAZI",
    "MAKINE BOSALDI AMA HIC DURMADI YENIDEN BASLADI",
    "MAKINE ILE 31 ÇEKIYOR VE BU IS BITMEK BILMIYOR",
    "MAKINE KARIYA GITTI VE ORADA ORTALIGI YIKIYOR",
    "MAKINE HERKESE ESCORT ÇAGIRIYOR ORTALIK ESCORT DENIZI",
    "MAKINE DOGUM YAPTI VE BIR SURU KÜÇÜK MAKINE ÇIKTI",
    "MAKINE CLOUDFLARE ILE UGRASIYOR SAATLERDIR SAVAŞIYOR",
    "EYÜP SENIN BEN AMINA KOYAYIM YA BU KADAR DA OLMAZ KI NE YAPIYORSUN YAVAŞ OL ROBOT DUR ARTIK SAKINLES ÇILDIRMA DELIRME DUR DUR DUR"
];

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AppPage {
    Dashboard,
    Home,
    Details,
    Premium,
    Watch,
    Custom,
    Calendar,
    Theme,
    Recommendations,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PresenceMetadata {
    pub anime_name: Option<String>,
    pub episode_no: Option<String>,
    pub poster_url: Option<String>,
    pub custom_title: Option<String>,
    pub paused: Option<bool>,
    pub anime_slug: Option<String>,
    pub current_time: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiscordThreadSignal {
    Update,
    Shutdown,
}

pub struct PresenceState {
    pub page: AppPage,
    pub metadata: Option<PresenceMetadata>,
    pub updated: bool,
    pub clear: bool,
    pub enabled: bool,
    pub focused_label: Option<String>,
    pub pending_label: Option<String>,
}

pub struct DiscordState {
    state: Arc<Mutex<PresenceState>>,
    tx: Sender<DiscordThreadSignal>,
}

impl DiscordState {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(PresenceState {
            page: AppPage::Home,
            metadata: None,
            updated: true,
            clear: false,
            enabled: true,
            focused_label: None,
            pending_label: None,
        }));

        let (tx, rx) = mpsc::channel();
        let state_clone = state.clone();

        thread::spawn(move || {
            let mut client: Option<DiscordIpcClient> = None;
            let mut last_connect_attempt: Option<Instant> = None;
            
            let mut current_page: Option<AppPage> = None;
            let mut current_metadata: Option<PresenceMetadata> = None;
            let mut dashboard_msg_index = 0;
            let mut last_dashboard_update = Instant::now();
            let mut was_clear = true;
            let mut last_sent_start_timestamp: Option<i64> = None;

            loop {
                let signal = rx.recv_timeout(Duration::from_secs(1));
                
                let mut shutdown = false;
                match signal {
                    Ok(DiscordThreadSignal::Shutdown) => {
                        shutdown = true;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        shutdown = true;
                    }
                    _ => {}
                }

                if shutdown {
                    println!("[Discord RPC] Kapatma sinyali alındı. RPC bağlantısı temizliyor...");
                    if let Some(mut c) = client.take() {
                        let _ = c.clear_activity();
                        let _ = c.close();
                    }
                    break;
                }

                let (page, metadata, updated, clear, enabled, is_focused_window) = {
                    let mut s = state_clone.lock().unwrap();
                    let page = s.page.clone();
                    let metadata = s.metadata.clone();
                    let updated = s.updated;
                    let clear = s.clear;
                    let enabled = s.enabled;
                    
                    let is_focused_window = match (&s.focused_label, &s.pending_label) {
                        (Some(focused), Some(pending)) => focused == pending,
                        (None, _) => true,
                        _ => false,
                    };
                    s.updated = false;
                    (page, metadata, updated, clear, enabled, is_focused_window)
                };

                if !enabled || clear {
                    if !was_clear {
                        if let Some(c) = &mut client {
                            if let Err(e) = c.clear_activity() {
                                eprintln!("[Discord RPC] Activity temizlenirken hata oluştu: {:?}", e);
                                let _ = c.close();
                                client = None;
                            }
                        }
                        was_clear = true;
                        current_page = None;
                        current_metadata = None;
                        last_sent_start_timestamp = None;
                    }
                    continue;
                }

                if !is_focused_window && updated {
                    continue;
                }

                let mut should_update = updated;

                if page == AppPage::Dashboard {
                    let now = Instant::now();
                    if current_page.as_ref() != Some(&AppPage::Dashboard) {
                        should_update = true;
                        dashboard_msg_index = 0;
                        last_dashboard_update = now;
                    } else if now.duration_since(last_dashboard_update) >= Duration::from_secs(10) {
                        should_update = true;
                        dashboard_msg_index = (dashboard_msg_index + 1) % DASHBOARD_MESSAGES.len();
                        last_dashboard_update = now;
                    }
                }

                if current_page.as_ref() != Some(&page) {
                    should_update = true;
                }

                if let (Some(ref curr_m), Some(ref new_m)) = (&current_metadata, &metadata) {
                    if curr_m.anime_name != new_m.anime_name
                        || curr_m.episode_no != new_m.episode_no
                        || curr_m.poster_url != new_m.poster_url
                        || curr_m.custom_title != new_m.custom_title
                        || curr_m.paused != new_m.paused
                        || curr_m.anime_slug != new_m.anime_slug
                    {
                        should_update = true;
                    }

                    if page == AppPage::Watch && new_m.paused == Some(false) {
                        if let (Some(_), Some(new_t)) = (curr_m.current_time, new_m.current_time) {
                            let epoch_now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;
                            let new_start = epoch_now - (new_t as i64);
                            
                            if let Some(last_start) = last_sent_start_timestamp {
                                if (new_start - last_start).abs() > 3 {
                                    should_update = true;
                                }
                            } else {
                                should_update = true;
                            }
                        }
                    }
                } else if current_metadata.is_some() != metadata.is_some() {
                    should_update = true;
                }

                if should_update {
                    let now = Instant::now();

                    if client.is_none() {
                        let can_connect = match last_connect_attempt {
                            Some(last) => now.duration_since(last) >= Duration::from_secs(10),
                            None => true,
                        };

                        if can_connect {
                            last_connect_attempt = Some(now);
                            println!("[Discord RPC] Discord IPC'ye bağlanmaya çalışılıyor...");
                            match DiscordIpcClient::new(CLIENT_ID) {
                                Ok(mut c) => {
                                    match c.connect() {
                                        Ok(_) => {
                                            println!("[Discord RPC] Bağlantı başarılı!");
                                            client = Some(c);
                                        }
                                        Err(e) => {
                                            eprintln!("[Discord RPC] Bağlantı başarısız: {:?}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[Discord RPC] Client oluşturulamadı: {:?}", e);
                                }
                            }
                        }
                    }

                    if let Some(c) = &mut client {
                        let mut activity = activity::Activity::new();
                        let details_str;
                        let state_str;
                        let mut timestamps = None;
                        
                        #[allow(unused_assignments)]
                        let mut anime_url_str = String::new();
                        #[allow(unused_assignments)]
                        let mut ep_url_str = String::new();
                        
                        
                    let name = metadata.as_ref()
                        .and_then(|m| m.anime_name.as_deref())
                        .unwrap_or("Anime");
                        
                        match page {
                            AppPage::Dashboard => {
                                let funny_state = DASHBOARD_MESSAGES[dashboard_msg_index];
                                details_str = "UPLOAD YAP(AM)IYOR".to_string();
                                state_str = funny_state.to_string();

                                activity = activity
                                    .details(&details_str)
                                    .state(&state_str);

                                let assets = activity::Assets::new()
                                    .large_image("https://i.imgur.com/IIcZBMH.jpeg")
                                    .large_text("Uploader");
                                activity = activity.assets(assets);
                            }
                            _ => {
                                details_str = match &page {
                                    AppPage::Home => "AnaSayfa | OpenAnime".to_string(),
                                    AppPage::Details => {
                                        format!("{} | OpenAnime", name)
                                    }
                                    AppPage::Watch => {
                                        let ep = metadata.as_ref()
                                            .and_then(|m| m.episode_no.as_deref())
                                            .unwrap_or("1");
                                        let ep_formatted = if ep.chars().all(|c| c.is_ascii_digit()) {
                                            format!("{}. Bölüm", ep)
                                        } else {
                                            ep.to_string()
                                        };
                                        format!("{} - {} | OpenAnime", name, ep_formatted)
                                    }
                                    AppPage::Custom => {
                                        let title = metadata.as_ref()
                                            .and_then(|m| m.custom_title.as_deref())
                                            .unwrap_or("OpenAnime");
                                        format!("{} | OpenAnime", title)
                                    }
                                    AppPage::Premium => "Abonelikler | OpenAnime".to_string(),
                                    AppPage::Calendar => "Takvim | OpenAnime".to_string(),
                                    AppPage::Theme => "Temalar | OpenAnime".to_string(),
                                    AppPage::Recommendations => "Kişiselleştirilmiş Öneriler | OpenAnime".to_string(),
                                    AppPage::Dashboard => unreachable!(),
                                };

                                state_str = match &page {
                                    AppPage::Home => "Geziniyor".to_string(),
                                    AppPage::Details => "İnceliyor".to_string(),
                                    AppPage::Watch => {
                                        let is_paused = metadata.as_ref()
                                            .and_then(|m| m.paused)
                                            .unwrap_or(false);
                                        if is_paused {
                                            "Animeyi Duraklattı".to_string()
                                        } else {
                                            "İzliyor".to_string()
                                        }
                                    }
                                    AppPage::Premium => "Abonelikleri İnceliyor".to_string(),
                                    AppPage::Custom => "Uygulamada Geziniyor".to_string(),
                                    AppPage::Calendar => "Yayın Akışını İnceliyor".to_string(),
                                    AppPage::Theme => "Temaları İnceliyor".to_string(),
                                    AppPage::Recommendations => "Önerileri İnceliyor".to_string(),
                                    AppPage::Dashboard => unreachable!(),
                                };

                                activity = activity
                                    .details(&details_str)
                                    .state(&state_str);

                                let is_paused = metadata.as_ref()
                                    .and_then(|m| m.paused)
                                    .unwrap_or(false);

                                let mut assets = activity::Assets::new();
                                let mut has_assets = false;

                                if let Some(ref meta) = metadata {
                                    if let Some(ref poster) = meta.poster_url {
                                        let poster_trimmed = poster.trim();
                                        let clean_poster = poster_trimmed
                                            .split('?').next().unwrap_or(poster_trimmed)
                                            .split('#').next().unwrap_or(poster_trimmed)
                                            .trim_end_matches('/');
                                        if !clean_poster.is_empty() 
                                            && clean_poster.len() <= 512
                                            && clean_poster.starts_with("http")
                                        {
                                            assets = assets.large_image(clean_poster);
                                            assets = assets.large_text("OpenAnime");
                                            has_assets = true;
                                        }
                                    }
                                }

                                if page == AppPage::Watch {
                                    if !has_assets {
                                        assets = assets.large_text("OpenAnime");
                                    }
                                    let small_image_url = if is_paused { PAUSE_ICON_URL } else { PLAY_ICON_URL };
                                    let small_text = if is_paused { "Duraklatıldı" } else { "İzliyor" };

                                    assets = assets.small_image(small_image_url);
                                    assets = assets.small_text(small_text);
                                    assets = assets.large_text(name);
                                    has_assets = true;
                                }
                                else if page == AppPage::Details {
                                    if !has_assets {
                                        assets = assets.large_text("Anime");
                                    }
                                    assets = assets.large_text(name);
                                    has_assets = true;
                                }

                                if has_assets {
                                    activity = activity.assets(assets);
                                }

                                let epoch_now = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs() as i64;
                                
                                if page == AppPage::Watch && !is_paused {
                                    let current_time_secs = metadata.as_ref()
                                        .and_then(|m| m.current_time)
                                        .unwrap_or(0.0) as i64;
                                    let start_timestamp = epoch_now - current_time_secs;
                                    let ts = activity::Timestamps::new().start(start_timestamp);
                                    activity = activity.timestamps(ts.clone());
                                    timestamps = Some(ts);
                                    last_sent_start_timestamp = Some(start_timestamp);
                                } else if page != AppPage::Watch {
                                    let ts = activity::Timestamps::new().start(epoch_now);
                                    activity = activity.timestamps(ts.clone());
                                    timestamps = Some(ts);
                                    last_sent_start_timestamp = None;
                                } else {
                                    last_sent_start_timestamp = None;
                                }

                                if let Some(ref slug) = metadata.as_ref().and_then(|m| m.anime_slug.as_ref()) {
                                    if !slug.trim().is_empty() {
                                        let clean_slug = slug.trim().trim_matches('/');
                                        anime_url_str = format!("https://openani.me/anime/{}", clean_slug);
                                        
                                        let mut button_vec = vec![];
                                        button_vec.push(activity::Button::new("Animeye Git", &anime_url_str));

                                        if page == AppPage::Watch {
                                            let ep = metadata.as_ref()
                                                .and_then(|m| m.episode_no.as_deref())
                                                .unwrap_or("1");
                                            let ep_url_segment = if ep.starts_with('S') && ep.contains('B') {
                                                if let Some(b_idx) = ep.find('B') {
                                                    let s_part = &ep[1..b_idx];
                                                    let e_part = &ep[b_idx+1..];
                                                    if let (Ok(s_num), Ok(e_num)) = (s_part.parse::<u32>(), e_part.parse::<u32>()) {
                                                        format!("{}/{}", s_num, e_num)
                                                    } else {
                                                        ep.to_string()
                                                    }
                                                } else {
                                                    ep.to_string()
                                                }
                                            } else {
                                                ep.to_string()
                                            };
                                            let clean_ep = ep_url_segment.trim().trim_matches('/');
                                            ep_url_str = format!("https://openani.me/anime/{}/{}", clean_slug, clean_ep);
                                            button_vec.push(activity::Button::new("Bölüme Git", &ep_url_str));
                                        }

                                        activity = activity.buttons(button_vec.clone());
                                    }
                                }
                            }
                        }

                        match c.set_activity(activity) {
                            Ok(_) => {
                                println!("[Discord RPC] Durum güncellendi: {:?}, Meta: {:?}", page, metadata);
                                was_clear = false;
                                current_page = Some(page.clone());
                                current_metadata = metadata.clone();
                            }
                            Err(e) => {
                                eprintln!("[Discord RPC] İlk durum güncelleme denemesi başarısız oldu: {:?}", e);

                                println!("[Discord RPC] Güvenli modda (resim yok, buton var) güncelleniyor...");
                                let mut safe_activity = activity::Activity::new()
                                    .details(&details_str)
                                    .state(&state_str);
                                
                                if let Some(ts) = timestamps {
                                    safe_activity = safe_activity.timestamps(ts);
                                }

                                let mut fallback_anime_url_owned = String::new();
                                let mut fallback_ep_url_owned = String::new();

                                let has_fallback_slug = metadata.as_ref()
                                    .and_then(|m| m.anime_slug.as_ref())
                                    .map(|slug| {
                                        let clean = slug.trim().trim_matches('/');
                                        if !clean.is_empty() {
                                            fallback_anime_url_owned = format!("https://openani.me/anime/{}", clean);
                                            
                                            if page == AppPage::Watch {
                                                if let Some(ref meta) = metadata {
                                                    let ep = meta.episode_no.as_deref().unwrap_or("1");
                                                    let ep_seg = if ep.starts_with('S') && ep.contains('B') {
                                                        if let Some(b_idx) = ep.find('B') {
                                                            let s_p = &ep[1..b_idx];
                                                            let e_p = &ep[b_idx+1..];
                                                            if let (Ok(s), Ok(e)) = (s_p.parse::<u32>(), e_p.parse::<u32>()) {
                                                                format!("{}/{}", s, e)
                                                            } else { ep.to_string() }
                                                        } else { ep.to_string() }
                                                    } else { ep.to_string() };
                                                    let clean_ep = ep_seg.trim().trim_matches('/');
                                                    fallback_ep_url_owned = format!("https://openani.me/anime/{}/{}", clean, clean_ep);
                                                }
                                            }
                                            true
                                        } else {
                                            false
                                        }
                                    })
                                    .unwrap_or(false);

                                if has_fallback_slug && !fallback_anime_url_owned.is_empty() {
                                    let mut fallback_buttons = vec![
                                        activity::Button::new("Animeye Git", &fallback_anime_url_owned)
                                    ];
                                    if !fallback_ep_url_owned.is_empty() {
                                        fallback_buttons.push(activity::Button::new("Bölüme Git", &fallback_ep_url_owned));
                                    }
                                    safe_activity = safe_activity.buttons(fallback_buttons);
                                }

                                match c.set_activity(safe_activity) {
                                    Ok(_) => {
                                        println!("[Discord RPC] Güvenli modda durum başarıyla güncellendi: {:?}", page);
                                        was_clear = false;
                                        current_page = Some(page.clone());
                                        current_metadata = metadata.clone();
                                    }
                                    Err(fallback_err) => {
                                        eprintln!("[Discord RPC] Güvenli modda da güncellenemedi: {:?}", fallback_err);
                                        let _ = c.close();
                                        client = None;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Self { state, tx }
    }

    pub fn update(&self, page: AppPage, metadata: Option<PresenceMetadata>, from_label: Option<String>) {
        if let Ok(mut s) = self.state.lock() {
            s.page = page;
            s.metadata = metadata;
            s.pending_label = from_label;
            s.updated = true;
            s.clear = false;
            let _ = self.tx.send(DiscordThreadSignal::Update);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.clear = true;
            s.updated = true;
            let _ = self.tx.send(DiscordThreadSignal::Update);
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        if let Ok(mut s) = self.state.lock() {
            s.enabled = enabled;
            s.updated = true;
            let _ = self.tx.send(DiscordThreadSignal::Update);
        }
    }

    pub fn set_focused_window(&self, label: Option<String>) {
        if let Ok(mut s) = self.state.lock() {
            let changed = s.focused_label != label;
            s.focused_label = label;
            if changed {
                s.updated = true;
                let _ = self.tx.send(DiscordThreadSignal::Update);
            }
        }
    }
}

impl Drop for DiscordState {
    fn drop(&mut self) {
        let _ = self.tx.send(DiscordThreadSignal::Shutdown);
    }
}