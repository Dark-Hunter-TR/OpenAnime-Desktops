// === OpenAnime — Native WPF Toast (PowerShell) ===
//
// Süper Bildirim toast'ı WebView penceresiyle DEĞİL, PowerShell üzerinden
// render edilen bir WPF penceresiyle gösterilir. Neden:
//   • Ana WebView uzak siteyi (openani.me) sarıyor; toast'ı bundle'lanmış bir
//     HTML asset'ine (toast.html) bağlamak kırılgandı — pencere kuruluyor ama
//     ölçüm/gösterim aşamasında sessizce takılabiliyordu.
//   • Native WPF toast; WebView'a, dev server'a, asset pipeline'ına hiç bağlı
//     değil — her koşulda çıkar. Görünüm sitenin Fluent/Malwarebytes tarzıyla
//     hizalı: sol accent şeridi, uygulama ikonu, ilerleme çubuğu, yaylanan
//     giriş animasyonu, bildirim sesi.
//
// ZENGİN İÇERİK (v2):
//   • Bildirim TİPİNE göre rozet ikonu + aksan rengi (beğeni→kalp/pembe,
//     yanıt→balon/mavi, yeni bölüm→oynat/yeşil, takip→kişi/mor).
//   • Anime POSTERİ: super_notifications href'ten slug çıkarıp API'den posteri
//     indirir, path'i buraya verir → rozet yerine anime kapağı gösterilir.
//   • TIKLANABİLİR: toast'a tıklanınca clickUrl bir sinyal dosyasına yazılır;
//     Rust tarafı (super_notifications::start_click_watcher) bunu izleyip
//     uygulamayı açar ve ilgili sayfaya gider. WPF ayrı bir PowerShell süreci
//     olduğundan Tauri'ye doğrudan geri kanal yok — köprü sinyal dosyası.
//
// Nexus'taki show_windows_toast'tan uyarlandı. Nexus SYSTEM servisinden
// çalıştığı için CreateProcessAsUserW ile kullanıcı oturumuna geçmek zorundaydı;
// OpenAnime zaten kullanıcı bağlamında çalıştığından düz `powershell.exe` spawn
// yeterli.
//
// GÜVENLİK NOTU: XAML tek tırnaklı here-string (@'...'@) içine gömülür — böylece
// bildirim başlığı/gövdesindeki `$` veya backtick PowerShell tarafından
// YORUMLANMAZ. Metin ayrıca XML olarak kaçışlanır (XAML geçerliliği için).
// Poster path'i ve clickUrl PowerShell tek-tırnaklı string'e (tırnak ikilenerek)
// gömülür — enjeksiyon güvenli.

#![cfg(windows)]

const APP_NAME: &str = "OpenAnime";
/// Tıklama köprüsü sinyal dosyası. Toast'a tıklanınca clickUrl buraya yazılır;
/// Rust watcher okuyup siler ve sayfaya gider. super_notifications ile paylaşılır.
pub const CLICK_SIGNAL_FILE: &str = "OpenAnime_toast_click.txt";
/// Uygulama ikonu binary'e gömülür; runtime'da %TEMP%'e yazılıp WPF'e path
/// verilir. Böylece bundle/dev fark etmeksizin ikon her zaman görünür.
const ICON_PNG: &[u8] = include_bytes!("../icons/icon.png");

/// Toast'a verilecek zengin içerik. Alanlar super_notifications tarafından
/// doldurulur; `notif_type`/`poster_path`/`url` boş bırakılabilir (test yolu).
pub struct ToastContent<'a> {
    pub title: &'a str,
    pub body: &'a str,
    /// Sunucunun bildirim `type`'ı (ör. "comment-like"). Rozet ikonu + rengini
    /// belirler. Boş → varsayılan zil/mavi.
    pub notif_type: &'a str,
    /// İndirilmiş anime posterinin yerel dosya yolu (varsa rozet yerine gösterilir).
    pub poster_path: Option<&'a str>,
    /// Tıklanınca gidilecek mutlak URL (varsa toast tıklanabilir olur).
    pub url: Option<&'a str>,
}

/// Bildirim tipine göre (Segoe Fluent glyph codepoint, aksan rengi hex6) döner.
/// Eşleşme substring bazlı — sunucu tip adlarını değiştirse bile yakın kalır.
fn type_style(notif_type: &str) -> (u32, &'static str) {
    let t = notif_type.to_lowercase();
    if t.contains("like") || t.contains("beğen") {
        (0xEB51, "FF5C8A") // kalp (dolu) · pembe
    } else if t.contains("reply") || t.contains("comment") || t.contains("yanıt") || t.contains("yorum") {
        (0xE90A, "62CDFE") // konuşma balonu · mavi
    } else if t.contains("episode") || t.contains("bölüm") || t.contains("new") || t.contains("yeni") {
        (0xE768, "4ADE80") // oynat · yeşil
    } else if t.contains("follow") || t.contains("takip") || t.contains("friend") {
        (0xE8FA, "A78BFA") // kişi ekle · mor
    } else {
        (0xE7E7, "62CDFE") // zil · mavi (varsayılan)
    }
}

/// XAML Text alanları için XML kaçışı.
pub(crate) fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Bir string'i PowerShell tek-tırnaklı literal olarak paketler ('' ile kaçış).
/// None/boş → `$null`.
pub(crate) fn ps_literal(s: Option<&str>) -> String {
    match s.filter(|v| !v.is_empty()) {
        Some(v) => format!("'{}'", v.replace('\'', "''")),
        None => "$null".to_string(),
    }
}

/// Bağımlılıksız UTF-16LE → Base64 (PowerShell -EncodedCommand için).
fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity((bytes.len() + 2) / 3 * 4);
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i] as u32;
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3F) as usize]);
        out.push(TABLE[((n >> 12) & 0x3F) as usize]);
        out.push(if i + 1 < bytes.len() { TABLE[((n >> 6) & 0x3F) as usize] } else { b'=' });
        out.push(if i + 2 < bytes.len() { TABLE[(n & 0x3F) as usize] } else { b'=' });
        i += 3;
    }
    String::from_utf8(out).unwrap()
}

/// PowerShell script'ini gizli pencerede (-EncodedCommand UTF-16LE→Base64) çalıştırır.
/// Ateşle-unut. Toast ve tepsi menüsü ortak kullanır. Çağıran, dönen `Child`'ı
/// isterse tutup daha sonra `kill()` edebilir (bkz. native_tray_menu — üst üste
/// açılan menü pencerelerini önlemek için kullanılıyor).
pub(crate) fn run_ps_script(script: &str) -> Option<std::process::Child> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let utf16: Vec<u16> = script.encode_utf16().collect();
    let bytes: Vec<u8> = utf16.iter().flat_map(|w| w.to_le_bytes()).collect();
    let b64 = encode_base64(&bytes);

    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-EncodedCommand",
            &b64,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .ok()
}

/// Gömülü ikonu %TEMP%'e (bir kez) yazar, mutlak path'i döner.
pub(crate) fn ensure_icon_path() -> Option<String> {
    let mut path = std::env::temp_dir();
    path.push("openanime-toast-icon.png");
    if !path.exists() {
        if std::fs::write(&path, ICON_PNG).is_err() {
            return None;
        }
    }
    Some(path.to_string_lossy().into_owned())
}

/// Zengin native WPF toast'ı gösterir. Ateşle-unut: hemen döner, PowerShell
/// süreci kendi başına çalışır ve ~6.5 sn sonra kapanır.
pub fn show_rich(content: &ToastContent) {
    let title_esc = escape_xml(content.title);
    let body_esc = escape_xml(content.body);

    let (glyph_cp, accent_hex) = type_style(content.notif_type);
    let accent = format!("#{}", accent_hex);
    let accent_soft = format!("#33{}", accent_hex); // ~20% opak rozet zemini
    let glyph_entity = format!("&#x{:X};", glyph_cp);

    let body_block = if body_esc.is_empty() {
        String::new()
    } else {
        format!(
            r##"<TextBlock Text="{}" Foreground="#9CA3AF" FontSize="12.5" TextWrapping="Wrap" LineHeight="19" Margin="0,3,0,0"/>"##,
            body_esc
        )
    };

    let icon_path_ps = ps_literal(ensure_icon_path().as_deref());
    let poster_path_ps = ps_literal(content.poster_path);
    let click_url_ps = ps_literal(content.url);
    let signal_file_ps = format!(
        "'{}'",
        std::env::temp_dir()
            .join(CLICK_SIGNAL_FILE)
            .to_string_lossy()
            .replace('\'', "''")
    );

    // Şablon: format! DEĞİL, .replace() ile doldurulur — böylece PowerShell/XAML
    // içindeki yüzlerce { } süslü parantezi ikiye katlamak gerekmez.
    let script = PS_TEMPLATE
        .replace("__ICONPATH_PS__", &icon_path_ps)
        .replace("__POSTERPATH_PS__", &poster_path_ps)
        .replace("__CLICKURL_PS__", &click_url_ps)
        .replace("__SIGNALFILE_PS__", &signal_file_ps)
        .replace("__ACCENT_SOFT__", &accent_soft)
        .replace("__ACCENT__", &accent)
        .replace("__APPNAME__", APP_NAME)
        .replace("__GLYPH__", &glyph_entity)
        .replace("__BODYBLOCK__", &body_block)
        // Başlık en son: içinde tesadüfen başka bir placeholder dizisi olamaz
        // ama yine de kaçışlı olduğu için güvenli.
        .replace("__TITLE__", &title_esc);

    let _ = run_ps_script(&script);
}

const PS_TEMPLATE: &str = r###"
Add-Type -AssemblyName PresentationFramework

# Rust'ın %TEMP%'e yazdığı uygulama ikonu (yoksa $null)
$iconPath = __ICONPATH_PS__
# Anime posteri (varsa rozet yerine gösterilir; yoksa $null)
$posterPath = __POSTERPATH_PS__
# Tıklanınca gidilecek URL + Rust'ın izlediği sinyal dosyası
$clickUrl = __CLICKURL_PS__
$signalFile = __SIGNALFILE_PS__

# XAML — tek tırnaklı here-string: içindeki $ ve backtick LİTERAL.
[xml]$XAML = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="Toast" SizeToContent="Height" Width="460"
        WindowStyle="None" AllowsTransparency="True" Background="Transparent"
        Topmost="True" ShowInTaskbar="False" WindowStartupLocation="Manual" ShowActivated="False">
    <Window.Resources>
        <Storyboard x:Key="FadeIn">
            <DoubleAnimation Storyboard.TargetProperty="Opacity" From="0" To="1" Duration="0:0:0.30"/>
            <ThicknessAnimation Storyboard.TargetProperty="Margin" From="60,0,-60,0" To="0,0,0,0" Duration="0:0:0.38">
                <ThicknessAnimation.EasingFunction><CubicEase EasingMode="EaseOut"/></ThicknessAnimation.EasingFunction>
            </ThicknessAnimation>
        </Storyboard>
        <Storyboard x:Key="FadeOut">
            <DoubleAnimation Storyboard.TargetProperty="Opacity" From="1" To="0" Duration="0:0:0.25"/>
            <ThicknessAnimation Storyboard.TargetProperty="Margin" From="0,0,0,0" To="60,0,-60,0" Duration="0:0:0.25">
                <ThicknessAnimation.EasingFunction><CubicEase EasingMode="EaseIn"/></ThicknessAnimation.EasingFunction>
            </ThicknessAnimation>
        </Storyboard>
    </Window.Resources>

    <Border Padding="12">
        <Border Name="MainBorder" Opacity="0" CornerRadius="10" Background="#202020">
            <Border.Effect>
                <DropShadowEffect Color="Black" BlurRadius="28" Opacity="0.7" Direction="270" ShadowDepth="8"/>
            </Border.Effect>
            <Grid>
                <!-- Sol accent şeridi -->
                <Border Width="6" HorizontalAlignment="Left" Background="__ACCENT__" CornerRadius="10,0,0,10"/>

                <DockPanel Margin="18,0,0,0">

                    <!-- HEADER: uygulama ikonu + isim + kapat -->
                    <Border DockPanel.Dock="Top" BorderThickness="0,0,0,1" BorderBrush="#2D3035" Padding="0,10,12,10">
                        <DockPanel LastChildFill="False">
                            <Border Width="22" Height="22" Margin="0,0,8,0" VerticalAlignment="Center"
                                    CornerRadius="4" ClipToBounds="True" Name="AppIconBorder">
                                <Image Name="AppIcon" Stretch="UniformToFill" RenderOptions.BitmapScalingMode="HighQuality"/>
                            </Border>
                            <TextBlock Text="__APPNAME__" Foreground="#E5E7EB" FontSize="13" FontWeight="Bold" VerticalAlignment="Center"/>
                            <Button Name="CloseBtn" DockPanel.Dock="Right" HorizontalAlignment="Right"
                                    Width="28" Height="28" Background="Transparent" BorderThickness="0" Cursor="Hand">
                                <Button.Template>
                                    <ControlTemplate TargetType="Button">
                                        <Border Background="{TemplateBinding Background}" CornerRadius="14">
                                            <Path Data="M8,8 L20,20 M20,8 L8,20" Stroke="#6B7280" StrokeThickness="1.7"
                                                  StrokeStartLineCap="Round" StrokeEndLineCap="Round"
                                                  HorizontalAlignment="Center" VerticalAlignment="Center"/>
                                        </Border>
                                        <ControlTemplate.Triggers>
                                            <Trigger Property="IsMouseOver" Value="True">
                                                <Setter Property="Background" Value="#2D3035"/>
                                            </Trigger>
                                        </ControlTemplate.Triggers>
                                    </ControlTemplate>
                                </Button.Template>
                            </Button>
                        </DockPanel>
                    </Border>

                    <!-- İÇERİK: rozet/poster + metin -->
                    <Grid Margin="0,16,16,16" DockPanel.Dock="Top">
                        <Grid.ColumnDefinitions>
                            <ColumnDefinition Width="58"/>
                            <ColumnDefinition Width="*"/>
                        </Grid.ColumnDefinitions>

                        <Grid Grid.Column="0" Width="46" Height="46" HorizontalAlignment="Left" VerticalAlignment="Top">
                            <!-- Tip rozeti (poster yoksa / poster yüklenemezse) -->
                            <Border Name="GlyphBadge" Width="44" Height="44" CornerRadius="22" Background="__ACCENT_SOFT__">
                                <TextBlock Text="__GLYPH__" FontFamily="Segoe Fluent Icons, Segoe MDL2 Assets"
                                           FontSize="22" Foreground="__ACCENT__"
                                           HorizontalAlignment="Center" VerticalAlignment="Center"/>
                            </Border>
                            <!-- Anime posteri (varsa) -->
                            <Border Name="PosterBadge" Width="44" Height="46" CornerRadius="8" ClipToBounds="True"
                                    Visibility="Collapsed">
                                <Border.Effect>
                                    <DropShadowEffect Color="Black" BlurRadius="8" Opacity="0.45" ShadowDepth="0"/>
                                </Border.Effect>
                                <Image Name="PosterImg" Stretch="UniformToFill" RenderOptions.BitmapScalingMode="HighQuality"/>
                            </Border>
                        </Grid>

                        <StackPanel Grid.Column="1" VerticalAlignment="Top">
                            <TextBlock Text="__TITLE__" Foreground="#F9FAFB" FontSize="15" FontWeight="Bold" TextWrapping="Wrap"/>
                            __BODYBLOCK__
                        </StackPanel>
                    </Grid>

                    <!-- İlerleme çubuğu -->
                    <Rectangle Name="PBar" DockPanel.Dock="Bottom" Height="3" HorizontalAlignment="Stretch"
                               Fill="__ACCENT__" Opacity="0.45">
                        <Rectangle.RenderTransform><ScaleTransform ScaleX="1" CenterX="0"/></Rectangle.RenderTransform>
                    </Rectangle>

                </DockPanel>
            </Grid>
        </Border>
    </Border>
</Window>
'@

$reader = (New-Object System.Xml.XmlNodeReader $XAML)
$window = [Windows.Markup.XamlReader]::Load($reader)

$desktop = [System.Windows.SystemParameters]::WorkArea

# ── Aynı anda tek toast: önceki PowerShell toast'ını kapat ──────
$pidFile = "$env:TEMP\OpenAnime_toast_pid.txt"
try {
    if (Test-Path $pidFile) {
        $oldPid = [int](Get-Content $pidFile -ErrorAction SilentlyContinue)
        if ($oldPid -and $oldPid -ne $PID) {
            try { Stop-Process -Id $oldPid -Force -ErrorAction SilentlyContinue } catch {}
        }
    }
} catch {}
try { Set-Content -Path $pidFile -Value $PID -Force } catch {}

$mainBorder = $window.FindName("MainBorder")

# Yardımcı: bir Image kontrolüne dosyadan bitmap yükle (OnLoad → dosya kilitlenmez)
function Load-Bitmap($imageCtrl, $path) {
    $bmp = New-Object System.Windows.Media.Imaging.BitmapImage
    $bmp.BeginInit()
    $bmp.UriSource = [Uri]::new($path, [System.UriKind]::Absolute)
    $bmp.CacheOption = [System.Windows.Media.Imaging.BitmapCacheOption]::OnLoad
    $bmp.EndInit()
    $imageCtrl.Source = $bmp
}

# İkon yüklenemezse header ikonu gizlenir (sadece uygulama adı kalır)
if ($iconPath) {
    $appIcon = $window.FindName("AppIcon")
    if ($appIcon) {
        try { Load-Bitmap $appIcon $iconPath } catch {
            $b = $window.FindName("AppIconBorder"); if ($b) { $b.Visibility = "Collapsed" }
        }
    }
} else {
    $b = $window.FindName("AppIconBorder"); if ($b) { $b.Visibility = "Collapsed" }
}

# Poster varsa: tip rozeti yerine anime kapağını göster (yüklenemezse rozet kalır)
if ($posterPath) {
    $posterImg = $window.FindName("PosterImg")
    if ($posterImg) {
        try {
            Load-Bitmap $posterImg $posterPath
            $pb = $window.FindName("PosterBadge"); if ($pb) { $pb.Visibility = "Visible" }
            $gb = $window.FindName("GlyphBadge"); if ($gb) { $gb.Visibility = "Collapsed" }
        } catch {}
    }
}

# Tıklanabilir: içerik alanına tıklayınca URL'yi sinyal dosyasına yaz, Rust açsın.
# (Kapat düğmesi bir Button olduğundan MouseLeftButtonUp'ı yutar — X'e tıklamak
#  gezinmeyi TETİKLEMEZ, sadece kapatır.)
if ($clickUrl) {
    $mainBorder.Cursor = "Hand"
    $mainBorder.Add_MouseLeftButtonUp({
        try { Set-Content -Path $signalFile -Value $clickUrl -Force -Encoding UTF8 } catch {}
        $window.Resources["FadeOut"].Begin($mainBorder)
        $ct = New-Object System.Windows.Threading.DispatcherTimer
        $ct.Interval = [TimeSpan]::FromSeconds(0.2)
        $ct.Add_Tick({ $args[0].Stop(); $window.Close() })
        $ct.Start()
    })
}

$window.Add_Closed({
    try {
        if (Test-Path $pidFile) {
            $curPid = [int](Get-Content $pidFile -ErrorAction SilentlyContinue)
            if ($curPid -eq $PID) { Remove-Item $pidFile -Force -ErrorAction SilentlyContinue }
        }
    } catch {}
})

$window.Add_Loaded({
    # Gerçek boyut yüklendikten sonra sağ alta konumlandır (görev çubuğu hariç)
    $window.Left = $desktop.Width  - $window.ActualWidth  - 24
    $window.Top  = $desktop.Height - $window.ActualHeight - 12

    $window.Resources["FadeIn"].Begin($mainBorder)

    $closeBtn = $window.FindName("CloseBtn")
    if ($closeBtn) {
        $closeBtn.Add_Click({
            $window.Resources["FadeOut"].Begin($mainBorder)
            $t = New-Object System.Windows.Threading.DispatcherTimer
            $t.Interval = [TimeSpan]::FromSeconds(0.26)
            $t.Add_Tick({ $window.Close() })
            $t.Start()
        })
    }

    $pbar = $window.FindName("PBar")
    if ($pbar) {
        $anim = New-Object System.Windows.Media.Animation.DoubleAnimation
        $anim.From = 1.0
        $anim.To   = 0.0
        $anim.Duration = [System.Windows.Duration]::new([TimeSpan]::FromSeconds(6.5))
        $pbar.RenderTransform.BeginAnimation([System.Windows.Media.ScaleTransform]::ScaleXProperty, $anim)
    }

    # Otomatik kapanış. Tick içinde timer'ı durdurmuyoruz: pencere kapanınca
    # ShowDialog döner ve süreç sonlanır (timer'a erişim scope'a bağlı ve
    # gereksiz). $args[0] = tik'i tetikleyen timer.
    $timer = New-Object System.Windows.Threading.DispatcherTimer
    $timer.Interval = [TimeSpan]::FromSeconds(6.5)
    $timer.Add_Tick({
        $args[0].Stop()
        $window.Resources["FadeOut"].Begin($mainBorder)
        $t2 = New-Object System.Windows.Threading.DispatcherTimer
        $t2.Interval = [TimeSpan]::FromSeconds(0.26)
        $t2.Add_Tick({ $args[0].Stop(); $window.Close() })
        $t2.Start()
    })
    $timer.Start()

    try {
        $p = New-Object System.Media.SoundPlayer
        $p.SoundLocation = "C:\Windows\Media\Windows Notify System Generic.wav"
        $p.Play()
    } catch {}
})

$window.ShowDialog() | Out-Null
"###;
