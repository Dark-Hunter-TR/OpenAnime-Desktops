// === OpenAnime — Özel Tepsi (Tray) Menüsü (Native WPF/PowerShell) ===
//
// Native Windows bağlam menüsü YERİNE, toast'la aynı teknikle (PowerShell +
// WPF) render edilen özel tasarım bir menü. Neden:
//   • Native menü sitenin koyu/Fluent estetiğine uymuyor; özelleştirilemiyor.
//   • Toast zaten native WPF; aynı yaklaşımı kullanmak tutarlı ve WebView'a
//     bağımlı değil.
//
// İÇERİK super_notifications tarafından, oturum durumuna göre kurulur:
//   • Giriş yapılmış: kullanıcı adı başlığı, ardından Aç / Profil / Kütüphanem /
//     Son Eklenenler / Takvim, ayraç, Kapat.
//   • Giriş yok: yalnızca Aç + Kapat.
//
// TIKLAMA KÖPRÜSÜ: menü öğesine tıklanınca öğenin `action` etiketi bir sinyal
// dosyasına yazılır; Rust watcher (super_notifications::start_click_watcher)
// okuyup işler ("show" | "quit" | "nav:<url>"). WPF ayrı bir PowerShell süreci
// olduğundan Tauri'ye doğrudan geri kanal yok — köprü sinyal dosyası (toast'la
// aynı desen).
//
// KONUMLANDIRMA: imlecin konumundan (tepsi = sağ alt) yukarı-sola doğru açılır;
// çalışma alanına (görev çubuğu hariç) sığacak şekilde kelepçelenir. DPI ölçeği
// CompositionTarget ile düzeltilir.
//
// KAPANMA: iki bağımsız mekanizma var, çünkü tepsi ikonundan tetiklenen bir
// pencerenin `Activate()` çağrısı Windows'un "foreground kilidi" yüzünden
// güvenilir şekilde işe yaramayabilir (arka planda oluşturulan süreçler için
// SetForegroundWindow sık sık reddedilir). Yalnızca WPF `Deactivated`
// olayına güvenmek, sağ tık üst üste hızlıca basıldığında eski pencerelerin
// hiç kapanmadan yeni pencerelerin altında/üstünde YIĞILMASINA yol açıyordu:
//   1) Rust tarafı: yeni menü açılmadan önce bir öncekinin sürecini `kill()`
//      eder (LAST_MENU_PROC) — aynı anda en fazla bir pencere var olabilir,
//      stacklenme kökünden engellenir.
//   2) PS tarafı: düşük seviye global fare kancası (WH_MOUSE_LL) ekrandaki
//      HERHANGİ bir sol/sağ tıklamayı yakalar; tıklama pencerenin dışındaysa
//      pencere kapanır. Bu, `Deactivated`'ın güvenilmez olduğu durumlarda da
//      "dışarı tıklayınca kapanır" davranışını garanti eder. Esc de kapatır.

#![cfg(windows)]

use crate::native_toast::{escape_xml, run_ps_script};
use std::process::Child;
use std::sync::Mutex;

/// Menü öğesi tıklamasının yazıldığı sinyal dosyası (super_notifications izler).
pub const TRAY_ACTION_FILE: &str = "OpenAnime_tray_action.txt";

/// En son açılan tepsi menüsü süreci. Yeni bir menü açılmadan önce bu
/// öldürülür — aynı anda yalnızca bir menü penceresi var olabilir.
static LAST_MENU_PROC: Mutex<Option<Child>> = Mutex::new(None);

/// Menü başlığı (yalnızca giriş yapılmışsa). Avatar/çıkış butonu kasıtlı
/// olarak yok — kalabalık ve gereksiz bulunduğu için kaldırıldı; hesaptan
/// çıkış "Profil Görüntüle" üzerinden siteye gidilerek yapılabilir.
pub struct MenuHeader {
    pub name: String,
    pub subtitle: String,
}

/// Tek bir menü öğesi.
pub struct MenuEntry {
    pub label: String,
    /// Segoe Fluent / MDL2 icon codepoint.
    pub glyph: u32,
    /// Tıklanınca yazılacak eylem: "show" | "quit" | "nav:<url>".
    pub action: String,
    /// Yıkıcı görünüm (Çıkış) — kırmızımsı renk.
    pub danger: bool,
}

fn build_items_xaml(entries: &[MenuEntry]) -> String {
    let mut out = String::new();
    for (i, e) in entries.iter().enumerate() {
        // Yıkıcı öğeden (Çıkış) önce ayraç — ilk öğe değilse.
        if e.danger && i > 0 {
            out.push_str(
                "<Border Height=\"1\" Background=\"#2D3035\" Margin=\"8,5\"/>\n",
            );
        }
        let fg = if e.danger { "#F87171" } else { "#E5E7EB" };
        let gfg = if e.danger { "#F87171" } else { "#9CA3AF" };
        // Öğeler Button DEĞİL Border: Button tıklamada fareyi capture eder ve
        // pencerenin SubTree capture'ını çalar → LostMouseCapture erken tetiklenir
        // (tıklama yutulur). Border fareyi capture etmez; dismiss sağlam çalışır.
        out.push_str(&format!(
            r##"<Border Style="{{StaticResource MenuItem}}" Tag="{action}">
  <DockPanel>
    <TextBlock Text="&#x{glyph:X};" FontFamily="Segoe Fluent Icons, Segoe MDL2 Assets" FontSize="16" Width="30" Foreground="{gfg}" VerticalAlignment="Center"/>
    <TextBlock Text="{label}" Foreground="{fg}" FontSize="13.5" VerticalAlignment="Center"/>
  </DockPanel>
</Border>
"##,
            action = escape_xml(&e.action),
            glyph = e.glyph,
            gfg = gfg,
            label = escape_xml(&e.label),
            fg = fg,
        ));
    }
    out
}

fn build_header_xaml(h: &MenuHeader) -> String {
    format!(
        r##"
      <StackPanel Margin="14,12,14,8">
        <TextBlock Text="{name}" Foreground="#F9FAFB" FontSize="14" FontWeight="Bold" TextTrimming="CharacterEllipsis"/>
        <TextBlock Text="{sub}" Foreground="#9CA3AF" FontSize="11.5" Margin="0,1,0,0"/>
      </StackPanel>
      <Border Height="1" Background="#2D3035" Margin="0,0,0,4"/>
"##,
        name = escape_xml(&h.name),
        sub = escape_xml(&h.subtitle),
    )
}

/// Özel tepsi menüsünü tepsi İKONUNUN konumuna göre gösterir. Ateşle-unut.
/// Açılmadan önce bir öncekini (varsa) kapatır ki sağ tık üst üste
/// basıldığında pencereler yığılmasın.
/// `icon_rect`: (left, top, width, height) — ikonun fiziksel piksel
/// cinsinden ekran dikdörtgeni (Tauri `TrayIconEvent::Click`'ten gelir).
/// İmlecin o anki konumu YERİNE bunu kullanmamızın sebebi: PowerShell/WPF
/// süreci ayağa kalkana kadar (birkaç yüz ms) fare çoktan hareket etmiş
/// olabiliyordu, menü ikonun değil son fare konumunun yanında açılıyordu.
pub fn show(header: Option<MenuHeader>, entries: Vec<MenuEntry>, icon_rect: (f64, f64, f64, f64)) {
    let header_xaml = header.as_ref().map(build_header_xaml).unwrap_or_default();
    let items_xaml = build_items_xaml(&entries);

    let signal_ps = format!(
        "'{}'",
        std::env::temp_dir()
            .join(TRAY_ACTION_FILE)
            .to_string_lossy()
            .replace('\'', "''")
    );

    let (icon_left, icon_top, icon_width, icon_height) = icon_rect;

    let script = PS_TEMPLATE
        .replace("__SIGNALFILE_PS__", &signal_ps)
        .replace("__HEADER_XAML__", &header_xaml)
        .replace("__ITEMS_XAML__", &items_xaml)
        .replace("__ICON_LEFT__", &icon_left.to_string())
        .replace("__ICON_TOP__", &icon_top.to_string())
        .replace("__ICON_WIDTH__", &icon_width.to_string())
        .replace("__ICON_HEIGHT__", &icon_height.to_string());

    if let Ok(mut guard) = LAST_MENU_PROC.lock() {
        if let Some(mut prev) = guard.take() {
            let _ = prev.kill();
        }
        *guard = run_ps_script(&script);
    }
}

const PS_TEMPLATE: &str = r###"
Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

# Dışarı tıklamayı (soL/sağ, ekranın herhangi bir yerinde) yakalamak için
# düşük seviye global fare kancası. WPF `Deactivated` olayı, tepsi ikonundan
# tetiklenen arka plan süreçlerinde Windows'un foreground kilidi yüzünden
# güvenilmez olabiliyor; bu kanca menünün dışına her tıklamada kapanmasını
# garanti eder.
# PERFORMANS: Add-Type -TypeDefinition her çağrıda C#'ı SIFIRDAN derler
# (500ms-1sn+ sürebilir) — bu, menü açılışındaki gecikmenin ana kaynağıydı.
# Derlenmiş DLL'i %TEMP%'e BİR KEZ yazıp sonraki her açılışta -Path ile
# (derlemesiz, neredeyse anında) yüklüyoruz.
if (-not ('OaMouseHook' -as [type])) {
    $hookDllPath = Join-Path $env:TEMP "OpenAnime_TrayHook_v2.dll"
    if (Test-Path $hookDllPath) {
        try { Add-Type -Path $hookDllPath -ErrorAction SilentlyContinue } catch {}
    }
}
if (-not ('OaMouseHook' -as [type])) {
    $oaMouseHookSrc = @"
using System;
using System.Runtime.InteropServices;

public class OaMouseHook {
    public delegate IntPtr Proc(int nCode, IntPtr wParam, IntPtr lParam);
    public const int WH_MOUSE_LL = 14;
    public const int WM_LBUTTONDOWN = 0x0201;
    public const int WM_RBUTTONDOWN = 0x0204;

    [StructLayout(LayoutKind.Sequential)]
    public struct POINT { public int x; public int y; }

    [StructLayout(LayoutKind.Sequential)]
    public struct MSLLHOOKSTRUCT {
        public POINT pt;
        public uint mouseData;
        public uint flags;
        public uint time;
        public IntPtr dwExtraInfo;
    }

    private static IntPtr _hookId = IntPtr.Zero;
    private static Proc _proc;
    public static event Action<int, int> OnClick;

    public static void Install() {
        _proc = HookCallback;
        using (var curProcess = System.Diagnostics.Process.GetCurrentProcess())
        using (var curModule = curProcess.MainModule) {
            _hookId = SetWindowsHookEx(WH_MOUSE_LL, _proc, GetModuleHandle(curModule.ModuleName), 0);
        }
    }

    public static void Uninstall() {
        if (_hookId != IntPtr.Zero) { UnhookWindowsHookEx(_hookId); _hookId = IntPtr.Zero; }
    }

    private static IntPtr HookCallback(int nCode, IntPtr wParam, IntPtr lParam) {
        if (nCode >= 0 && (wParam.ToInt32() == WM_LBUTTONDOWN || wParam.ToInt32() == WM_RBUTTONDOWN)) {
            var hookStruct = (MSLLHOOKSTRUCT)Marshal.PtrToStructure(lParam, typeof(MSLLHOOKSTRUCT));
            var handler = OnClick;
            if (handler != null) handler(hookStruct.pt.x, hookStruct.pt.y);
        }
        return CallNextHookEx(_hookId, nCode, wParam, lParam);
    }

    [DllImport("user32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    private static extern IntPtr SetWindowsHookEx(int idHook, Proc lpfn, IntPtr hMod, uint dwThreadId);
    [DllImport("user32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool UnhookWindowsHookEx(IntPtr hhk);
    [DllImport("user32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    private static extern IntPtr CallNextHookEx(IntPtr hhk, int nCode, IntPtr wParam, IntPtr lParam);
    [DllImport("kernel32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    private static extern IntPtr GetModuleHandle(string lpModuleName);

    // DPI farkındalığı. Ayrı bir Add-Type ile eklemek her menü açılışına
    // 500ms+ C# derleme maliyeti bindirirdi; önbelleklenen bu DLL'e koyunca
    // maliyet yalnızca ilk çalıştırmada oluşur.
    [DllImport("user32.dll")]
    public static extern bool SetProcessDPIAware();
}
"@
    try {
        Add-Type -TypeDefinition $oaMouseHookSrc -OutputAssembly $hookDllPath -ErrorAction SilentlyContinue
        # ÖNEMLİ: `-OutputAssembly` yalnızca DLL'i DİSKE YAZAR — tipi geçerli
        # oturuma YÜKLEMEZ. Bu yüzden DLL'i ilk kez üreten çalıştırmada
        # 'OaMouseHook' hâlâ tanımsız kalıyordu ve aşağıdaki
        # `if ('OaMouseHook' -as [type])` guard'ı yüzünden fare kancası SESSİZCE
        # kurulmuyordu (yani "dışarı tıklayınca kapan" ilk açılışta hiç
        # çalışmıyordu; ancak ikinci çalıştırmada üstteki `Add-Type -Path`
        # devreye girdiğinde çalışıyordu). Yazdıktan hemen sonra yükle.
        if (-not ('OaMouseHook' -as [type])) {
            try { Add-Type -Path $hookDllPath -ErrorAction SilentlyContinue } catch {}
        }
    } catch {
        try { Add-Type -TypeDefinition $oaMouseHookSrc -ErrorAction SilentlyContinue } catch {}
    }
}

# ── DPI FARKINDALIĞI — İLK PENCEREDEN ÖNCE ÇAĞRILMALI ────────────────
# Rust bize tepsi ikonunun dikdörtgenini GERÇEK FİZİKSEL piksel olarak veriyor
# (ör. 1575,947). Süreç DPI-UNAWARE olduğunda ise System.Windows.Forms.Screen
# SANALLAŞTIRILMIŞ koordinat döndürüyor: %125 ölçekli 1920x1080 bir ekran
# 1536x864 olarak görünüyor. İki farklı koordinat uzayı karşılaştırılınca
# ikon merkezi (1600) çalışma alanının sağ kenarından (1536) büyük çıkıp
# "görev çubuğu SAĞDA" sanılıyordu → menü yanlış kenara yaslanıyordu.
# DPI-aware olunca Forms da fiziksel piksel döndürür (1920x1020), ikon
# rect'iyle AYNI uzaya gelir; WPF'in TransformFromDevice'ı da fiziksel→DIP
# dönüşümünü doğru yapar. Ekran boyutu/ölçek ne olursa olsun tutarlı çalışır.
if ('OaMouseHook' -as [type]) { try { [OaMouseHook]::SetProcessDPIAware() | Out-Null } catch {} }

$signalFile = __SIGNALFILE_PS__

# Tepsi ikonunun fiziksel piksel cinsinden ekran dikdörtgeni (Rust'tan gelir).
$iconLeftPx = __ICON_LEFT__
$iconTopPx = __ICON_TOP__
$iconWidthPx = __ICON_WIDTH__
$iconHeightPx = __ICON_HEIGHT__

[xml]$XAML = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="TrayMenu" SizeToContent="WidthAndHeight"
        WindowStyle="None" AllowsTransparency="True" Background="Transparent"
        Topmost="True" ShowInTaskbar="False" WindowStartupLocation="Manual"
        ShowActivated="False" UseLayoutRounding="True" SnapsToDevicePixels="True"
        TextOptions.TextFormattingMode="Display" TextOptions.TextRenderingMode="ClearType"
        RenderOptions.ClearTypeHint="Enabled">
    <Window.Resources>
        <Style x:Key="MenuItem" TargetType="Border">
            <Setter Property="Background" Value="Transparent"/>
            <Setter Property="CornerRadius" Value="7"/>
            <Setter Property="Padding" Value="12,9"/>
            <Setter Property="Cursor" Value="Hand"/>
            <Style.Triggers>
                <Trigger Property="IsMouseOver" Value="True">
                    <Setter Property="Background" Value="#2A2E33"/>
                </Trigger>
            </Style.Triggers>
        </Style>
    </Window.Resources>

    <Border Padding="10">
        <Border CornerRadius="12" Background="#1E1E1E" BorderBrush="#2D3035" BorderThickness="1"
                RenderOptions.ClearTypeHint="Enabled">
            <Border.Effect>
                <DropShadowEffect Color="Black" BlurRadius="24" Opacity="0.55" Direction="270" ShadowDepth="6"/>
            </Border.Effect>
            <StackPanel Width="248" Margin="6">
                __HEADER_XAML__
                <StackPanel x:Name="Items" Margin="0,2,0,2">
                    __ITEMS_XAML__
                </StackPanel>
            </StackPanel>
        </Border>
    </Border>
</Window>
'@

$reader = (New-Object System.Xml.XmlNodeReader $XAML)
$window = [Windows.Markup.XamlReader]::Load($reader)

# Öğe tıklamaları: action'ı sinyal dosyasına yaz, menüyü kapat. (Öğeler Border;
# ayraç Border'ının Tag'i yok → atlanır.)
$items = $window.FindName("Items")
if ($items) {
    foreach ($child in $items.Children) {
        if ($child -is [System.Windows.Controls.Border] -and $child.Tag) {
            $child.Add_MouseLeftButtonUp({
                try { Set-Content -Path $signalFile -Value $this.Tag -Force -Encoding UTF8 } catch {}
                try { $window.Close() } catch {}
            })
        }
    }
}

$script:hookInstalled = $false
# Pencerenin gerçekten yerleşip gösterildiği an. `Deactivated` guard'ı bunu
# kullanır (bkz. Add_Deactivated).
$script:shownAt = $null

$window.Add_Loaded({
    # Konumlandırma: tepsi İKONUNUN kendi dikdörtgenine göre (imlece göre
    # DEĞİL). Süreç ayağa kalkana kadar geçen sürede fare hareket etmiş
    # olabilir; ikonun rect'i her zaman doğru referans.
    $src = [System.Windows.PresentationSource]::FromVisual($window)

    # Fiziksel px -> DIP (DPI ölçekleme)
    $ix = [double]$iconLeftPx
    $iy = [double]$iconTopPx
    $iw = [double]$iconWidthPx
    $ih = [double]$iconHeightPx
    if ($src -and $src.CompositionTarget) {
        $ip1 = $src.CompositionTarget.TransformFromDevice.Transform([System.Windows.Point]::new($iconLeftPx, $iconTopPx))
        $ip2 = $src.CompositionTarget.TransformFromDevice.Transform([System.Windows.Point]::new(([double]$iconLeftPx + [double]$iconWidthPx), ([double]$iconTopPx + [double]$iconHeightPx)))
        $ix = $ip1.X
        $iy = $ip1.Y
        $iw = $ip2.X - $ip1.X
        $ih = $ip2.Y - $ip1.Y
    }

    $w = $window.ActualWidth
    $h = $window.ActualHeight

    # Ekran sınırlarına kelepçele (menü ekrandan taşmasın) — ikonun bulunduğu
    # monitörü ikon dikdörtgeninin merkez noktasından bul.
    $iconCenterPhys = New-Object System.Drawing.Point([int]([double]$iconLeftPx + [double]$iconWidthPx / 2), [int]([double]$iconTopPx + [double]$iconHeightPx / 2))
    $screen = [System.Windows.Forms.Screen]::FromPoint($iconCenterPhys)
    $bounds = $screen.WorkingArea

    # DIP cinsinden bounds
    $bx = [double]$bounds.X
    $by = [double]$bounds.Y
    $bw = [double]$bounds.Width
    $bh = [double]$bounds.Height
    if ($src -and $src.CompositionTarget) {
        $bp1 = $src.CompositionTarget.TransformFromDevice.Transform([System.Windows.Point]::new($bounds.X, $bounds.Y))
        $bp2 = $src.CompositionTarget.TransformFromDevice.Transform([System.Windows.Point]::new($bounds.Width, $bounds.Height))
        $bx = $bp1.X
        $by = $bp1.Y
        $bw = $bp2.X
        $bh = $bp2.Y
    }

    # ── Konumlandırma (native Windows tepsi menüsü davranışı) ─────────────
    #
    # 1) GÖLGE TELAFİSİ: XAML'deki dış `<Border Padding="10">` yalnızca drop
    #    shadow için var ve ActualWidth/ActualHeight'a DAHİL. Hizalamayı ham
    #    pencere ölçüsüyle yapmak, GÖRÜNÜR menüyü ikondan/görev çubuğundan
    #    10px kaydırıyordu. Artık hizalama görünür dikdörtgen üzerinden.
    #
    # 2) DİKEY ÇAPA: Eskiden `$top = $iy - $h` ile menü İKONUN ÜST KENARINA
    #    yaslanıyordu. Ama ikon görev çubuğunun İÇİNDE ve dikey konumu görev
    #    çubuğu yüksekliğine/DPI'a/ikonun taşma panelinde olup olmamasına göre
    #    değişiyor → menü her açılışta biraz farklı yerde çıkıyordu (kullanıcının
    #    "stabil değil" dediği davranış). Native menüler (TrackPopupMenu +
    #    TPM_BOTTOMALIGN) menüyü ÇALIŞMA ALANI kenarına yaslar; çalışma alanı
    #    kenarı sabit bir referans olduğu için menü her seferinde AYNI yerde
    #    açılır. Artık öyle yapıyoruz.
    #
    # 3) Dört görev çubuğu kenarı da (alt/üst/sol/sağ) destekleniyor.
    $shadow = 10.0
    $vw = $w - 2 * $shadow   # görünür menü genişliği
    $vh = $h - 2 * $shadow   # görünür menü yüksekliği
    $gap = 2.0               # görev çubuğu ile menü arasındaki nefes payı

    $iconCx = $ix + $iw / 2
    $iconCy = $iy + $ih / 2

    # Görev çubuğu hangi kenarda? İkon, çalışma alanının DIŞINDA kalan
    # kenardadır (çalışma alanı görev çubuğunu zaten dışlar).
    $tbTop   = $iconCy -le $by
    $tbLeft  = $iconCx -le $bx
    $tbRight = $iconCx -ge ($bx + $bw)

    if ($tbLeft -or $tbRight) {
        # Dikey görev çubuğu: menü ikonun yanında, dikeyde ikonla ortalanır.
        $vy = $iconCy - $vh / 2
        if ($tbLeft) { $vx = $bx + $gap } else { $vx = ($bx + $bw) - $vw - $gap }
    } else {
        # Yatay görev çubuğu (en yaygın): menünün SAĞ kenarı ikonun sağ
        # kenarıyla hizalı.
        $vx = $ix + $iw - $vw
        if ($tbTop) {
            # Görev çubuğu ÜSTTE: ikonun ALT kenarı ile çalışma alanının ÜST
            # kenarından hangisi daha AŞAĞIDAYSA ona yaslan.
            $vy = [Math]::Max(($iy + $ih), $by) + $gap
        } else {
            # Görev çubuğu ALTTA. Menünün alt kenarı; ikonun ÜST kenarı ile
            # çalışma alanının ALT kenarından hangisi daha YUKARIDAYSA ona yaslanır:
            #   • İkon doğrudan görev çubuğundaysa -> ikonun üstü görev çubuğunun
            #     içinde kalır, çalışma alanı kenarı seçilir. Sabit referans →
            #     menü her açılışta AYNI yerde (native/TranslucentTB davranışı).
            #   • İkon "gizli simgeler" panelindeyse -> o panel görev çubuğunun
            #     ÜSTÜNDE yüzdüğü için ikonun üst kenarı seçilir; menü ikonun
            #     hemen üstünde açılır, panelin üzerine binmez.
            # Ölçüm: çalışma alanı alt kenarı 1020, panel içindeki ikon üstü 947.
            $vy = [Math]::Min($iy, ($by + $bh)) - $vh - $gap
        }
    }

    # Çalışma alanına kelepçele — GÖRÜNÜR dikdörtgen üzerinden, böylece menü
    # hiçbir kenardan taşmaz (çoklu monitörde de doğru: bounds ikonun bulunduğu
    # monitörden geliyor).
    if ($vx -lt $bx) { $vx = $bx }
    if (($vx + $vw) -gt ($bx + $bw)) { $vx = ($bx + $bw) - $vw }
    if ($vy -lt $by) { $vy = $by }
    if (($vy + $vh) -gt ($by + $bh)) { $vy = ($by + $bh) - $vh }

    # Görünür dikdörtgen -> pencere konumu (gölge dolgusunu geri ekle).
    $window.Left = $vx - $shadow
    $window.Top  = $vy - $shadow

    # ODAK ÇALMIYORUZ (bilinçli). Eskiden burada Activate()+Focus() vardı ve
    # Windows 11'in "gizli simgeleri göster" taşma paneli LIGHT-DISMISS bir
    # yüzey olduğu için odağı kaybettiği anda KAPANIYORDU — kullanıcı menüyü
    # açar açmaz tepsi paneli kapanıyordu. Pencere ShowActivated="False" ile
    # gösterildiğinden ve artık aktivasyon istemediğimizden panel açık kalır.
    # Dismiss zaten odağa DEĞİL, global fare kancasına (konum bazlı) dayanıyor;
    # bu yüzden odak almamak dismiss davranışını bozmaz.

    # Fare kancasını pencere yerleşip konumlandıktan SONRA kur — açılışı
    # tetikleyen tıklamanın kendisi "dışarı tıklama" sayılıp menüyü anında
    # kapatmasın diye.
    try {
        if ('OaMouseHook' -as [type]) {
            [OaMouseHook]::Install()
            $script:hookInstalled = $true
            $outsideClickAction = {
                param($px, $py)
                try {
                    $pt = [System.Windows.Point]::new([double]$px, [double]$py)
                    $src2 = [System.Windows.PresentationSource]::FromVisual($window)
                    if ($src2 -and $src2.CompositionTarget) {
                        $pt = $src2.CompositionTarget.TransformFromDevice.Transform($pt)
                    }
                    $inside = ($pt.X -ge $window.Left) -and ($pt.X -le ($window.Left + $window.ActualWidth)) -and
                              ($pt.Y -ge $window.Top) -and ($pt.Y -le ($window.Top + $window.ActualHeight))
                    if (-not $inside) {
                        $window.Dispatcher.Invoke([Action]{ try { $window.Close() } catch {} })
                    }
                } catch {}
            }
            [OaMouseHook]::add_OnClick([Action[int,int]]$outsideClickAction)
        }
    } catch {}

    # Artık gösterildik — `Deactivated` bu andan itibaren geçerli sayılır.
    $script:shownAt = [DateTime]::UtcNow
})

$window.Add_Closed({
    if ($script:hookInstalled) { try { [OaMouseHook]::Uninstall() } catch {} }
})

# DISMISS MANTIĞI — iki mekanizma, net iş bölümü:
#
#   BİRİNCİL: global fare kancası (WH_MOUSE_LL). Menüyü YALNIZCA gerçek bir
#   dışarı TIKLAMASINDA kapatır. Doğru davranışın kaynağı bu:
#     • Menü İÇİNDE hover/scroll/gezinme → hiçbir tıklama yok → KAPANMAZ.
#       (Kanca yalnızca WM_LBUTTONDOWN/WM_RBUTTONDOWN dinler; hover ve
#       tekerlek olayları hiç gelmez.)
#     • Menü içine tıklama → nokta pencere dikdörtgeninin içinde → KAPANMAZ
#       (öğe kendi MouseLeftButtonUp'ında zaten kapatıyor).
#     • Görev çubuğu, "gizli simgeleri göster" paneli, masaüstü, başka bir
#       pencere → nokta dışarıda → KAPANIR.
#
#   YEDEK: `Deactivated`. SADECE kanca kurulamadıysa devreye girer.
#
# NEDEN KOŞULA BAĞLANDI: `Deactivated` yalnızca "odak kaybı"nı bilir, farenin
# NEREDE olduğunu bilmez. Menü, tepsi ikonundan tetiklenen ARKA PLAN bir
# PowerShell sürecinde açılıyor ve Windows'un foreground kilidi böyle süreçlere
# SetForegroundWindow'u sık sık reddediyor (bkz. dosya başındaki not). Bu yüzden
# kullanıcı menünün İÇİNDE gezinirken bile odak kabuğa/görev çubuğuna kayabiliyor
# ve Deactivated tetiklenip menüyü kullanıcının burnunun dibinde kapatıyordu —
# istenen "menü içindeyken KESİNLİKLE kapanmasın" kuralının doğrudan ihlali.
# Kanca zaten daha isabetli (konum bazlı) karar verdiğinden, kanca varken
# Deactivated'a hiç güvenmiyoruz.
$window.Add_Deactivated({
    # Kanca çalışıyorsa dismiss kararı ONUN — odak kaybına göre kapatma.
    if ($script:hookInstalled) { return }
    # Kancasız yedek yol: açılıştaki yerleşme anında gelen (pencere daha
    # görünmeden foreground reddedilmesinden kaynaklanan) Deactivated'ı yut.
    if ($null -eq $script:shownAt) { return }
    if (([DateTime]::UtcNow - $script:shownAt).TotalMilliseconds -lt 700) { return }
    try { $window.Close() } catch {}
})

$window.Add_KeyDown({ if ($_.Key -eq 'Escape') { try { $window.Close() } catch {} } })

# DEJENERE DURUM KORUMASI: Fare kancası kurulamıyorsa (tip hiç yüklenemedi)
# geriye dismiss için TEK yol olarak odak kaybı kalır. O senaryoda pencereyi
# istisnai olarak aktive edilebilir yaparız — aksi halde menü ne tıklamayla ne
# odakla kapanabilir, ekranda ASILI KALIRDI. Kanca varsa aktivasyonu bilerek
# İSTEMİYORUZ (tepsi taşma paneli açık kalsın).
# Bu kontrol Show'dan ÖNCE yapılmalı; kanca tipinin varlığı burada zaten belli.
if (-not ('OaMouseHook' -as [type])) { $window.ShowActivated = $true }

# ShowDialog() KULLANILMIYOR: modal gösterim pencereyi her hâlükârda aktive
# eder ve ShowActivated="False" ayarını yok sayar — bu da tepsi taşma panelini
# yine kapatırdı. Application.Run, Show() semantiğiyle çalışıp ShowActivated'a
# saygı duyar; pencere kapandığında mesaj döngüsü biter ve süreç çıkar.
$app = New-Object System.Windows.Application
$app.ShutdownMode = [System.Windows.ShutdownMode]::OnLastWindowClose
$app.Run($window) | Out-Null
"###;
