// === OpenAnime — Özel Tepsi (Tray) Menüsü (Native WPF/PowerShell) ===
//
// Native Windows bağlam menüsü YERİNE, toast'la aynı teknikle (PowerShell +
// WPF) render edilen özel tasarım bir menü. Neden:
//   • Native menü sitenin koyu/Fluent estetiğine uymuyor; özelleştirilemiyor.
//   • Toast zaten native WPF; aynı yaklaşımı kullanmak tutarlı ve WebView'a
//     bağımlı değil.
//
// İÇERİK super_notifications tarafından, oturum durumuna göre kurulur:
//   • Giriş yapılmış: avatar + kullanıcı adı başlığı, ardından Aç / Profil /
//     Kütüphanem / Son Eklenenler / Takvim, ayraç, Çıkış.
//   • Giriş yok: yalnızca Aç + Çıkış.
//
// TIKLAMA KÖPRÜSÜ: menü öğesine tıklanınca öğenin `action` etiketi bir sinyal
// dosyasına yazılır; Rust watcher (super_notifications::start_click_watcher)
// okuyup işler ("show" | "quit" | "nav:<url>"). WPF ayrı bir PowerShell süreci
// olduğundan Tauri'ye doğrudan geri kanal yok — köprü sinyal dosyası (toast'la
// aynı desen).
//
// KONUMLANDIRMA: imlecin konumundan (tepsi = sağ alt) yukarı-sola doğru açılır;
// çalışma alanına (görev çubuğu hariç) sığacak şekilde kelepçelenir. DPI ölçeği
// CompositionTarget ile düzeltilir. Dışarı tıklama (Deactivated) veya Esc kapatır.

#![cfg(windows)]

use crate::native_toast::{escape_xml, ps_literal, run_ps_script};

/// Menü öğesi tıklamasının yazıldığı sinyal dosyası (super_notifications izler).
pub const TRAY_ACTION_FILE: &str = "OpenAnime_tray_action.txt";

const ACCENT: &str = "#62CDFE";

/// Menü başlığı (yalnızca giriş yapılmışsa).
pub struct MenuHeader {
    pub name: String,
    pub subtitle: String,
    /// İndirilmiş avatarın yerel yolu; yoksa kişi ikonu (fallback) gösterilir.
    pub avatar_path: Option<String>,
    /// Avatarın hemen sağındaki çıkış butonunun eylemi (hesaptan çıkış):
    /// "nav:<logout-url>". Menü öğeleriyle aynı sinyal-dosyası köprüsünü kullanır.
    pub logout_action: String,
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
      <DockPanel Margin="12,12,12,8">
        <Grid Width="40" Height="40" Margin="0,0,12,0" DockPanel.Dock="Left" VerticalAlignment="Center">
          <Border Name="AvatarGlyph" CornerRadius="20" Background="#3362CDFE">
            <TextBlock Text="&#xE77B;" FontFamily="Segoe Fluent Icons, Segoe MDL2 Assets" FontSize="20"
                       Foreground="{accent}" HorizontalAlignment="Center" VerticalAlignment="Center"/>
          </Border>
          <Border Name="AvatarClip" CornerRadius="20" ClipToBounds="True" Visibility="Collapsed">
            <Image Name="AvatarImg" Stretch="UniformToFill" RenderOptions.BitmapScalingMode="HighQuality"/>
          </Border>
          <!-- Yuvarlak kenar halkası: avatarı her koşulda daire olarak çerçeveler. -->
          <Border CornerRadius="20" BorderBrush="#33FFFFFF" BorderThickness="1" IsHitTestVisible="False"/>
        </Grid>
        <!-- Avatarın hemen sağında: hesaptan çıkış. Tag = logout eylemi; PS
             tarafı LogoutBtn'i öğelerle aynı şekilde sinyal dosyasına bağlar. -->
        <Border Name="LogoutBtn" Tag="{logout}" Style="{{StaticResource IconButton}}"
                DockPanel.Dock="Right" Width="34" Height="34" VerticalAlignment="Center"
                ToolTip="Hesaptan Çıkış">
          <TextBlock Text="&#xF3B1;" FontFamily="Segoe Fluent Icons, Segoe MDL2 Assets" FontSize="16"
                     Foreground="#F87171" HorizontalAlignment="Center" VerticalAlignment="Center"/>
        </Border>
        <StackPanel VerticalAlignment="Center">
          <TextBlock Text="{name}" Foreground="#F9FAFB" FontSize="14" FontWeight="Bold" TextTrimming="CharacterEllipsis"/>
          <TextBlock Text="{sub}" Foreground="#9CA3AF" FontSize="11.5" Margin="0,1,0,0"/>
        </StackPanel>
      </DockPanel>
      <Border Height="1" Background="#2D3035" Margin="0,0,0,4"/>
"##,
        accent = ACCENT,
        logout = escape_xml(&h.logout_action),
        name = escape_xml(&h.name),
        sub = escape_xml(&h.subtitle),
    )
}

/// Özel tepsi menüsünü imlecin yanında gösterir. Ateşle-unut.
pub fn show(header: Option<MenuHeader>, entries: Vec<MenuEntry>) {
    let header_xaml = header.as_ref().map(build_header_xaml).unwrap_or_default();
    let items_xaml = build_items_xaml(&entries);

    // Avatar: başlıkta verilmişse onu, yoksa uygulama ikonunu DENEME (fallback
    // glyph zaten var; ikon avatar yerine geçmesin diye yalnızca gerçek avatar
    // verilmişse yükleriz).
    let avatar_ps = ps_literal(header.as_ref().and_then(|h| h.avatar_path.as_deref()));

    let signal_ps = format!(
        "'{}'",
        std::env::temp_dir()
            .join(TRAY_ACTION_FILE)
            .to_string_lossy()
            .replace('\'', "''")
    );

    let script = PS_TEMPLATE
        .replace("__SIGNALFILE_PS__", &signal_ps)
        .replace("__AVATARPATH_PS__", &avatar_ps)
        .replace("__ACCENT__", ACCENT)
        .replace("__HEADER_XAML__", &header_xaml)
        .replace("__ITEMS_XAML__", &items_xaml);

    run_ps_script(&script);
}

const PS_TEMPLATE: &str = r###"
Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$signalFile = __SIGNALFILE_PS__
$avatarPath = __AVATARPATH_PS__

[xml]$XAML = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="TrayMenu" SizeToContent="WidthAndHeight"
        WindowStyle="None" AllowsTransparency="True" Background="Transparent"
        Topmost="True" ShowInTaskbar="False" WindowStartupLocation="Manual"
        ShowActivated="True">
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
        <!-- Başlıktaki çıkış (logout) butonu: küçük kare, üzerine gelince kırmızımsı. -->
        <Style x:Key="IconButton" TargetType="Border">
            <Setter Property="Background" Value="Transparent"/>
            <Setter Property="CornerRadius" Value="8"/>
            <Setter Property="Cursor" Value="Hand"/>
            <Style.Triggers>
                <Trigger Property="IsMouseOver" Value="True">
                    <Setter Property="Background" Value="#3A2226"/>
                </Trigger>
            </Style.Triggers>
        </Style>
    </Window.Resources>

    <Border Padding="10">
        <Border CornerRadius="12" Background="#1E1E1E" BorderBrush="#2D3035" BorderThickness="1">
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

# Avatar (varsa): fallback glyph yerine yuvarlak kapak
if ($avatarPath) {
    $ai = $window.FindName("AvatarImg")
    if ($ai) {
        try {
            $bmp = New-Object System.Windows.Media.Imaging.BitmapImage
            $bmp.BeginInit()
            $bmp.UriSource = [Uri]::new($avatarPath, [System.UriKind]::Absolute)
            $bmp.CacheOption = [System.Windows.Media.Imaging.BitmapCacheOption]::OnLoad
            $bmp.EndInit()
            $ai.Source = $bmp
            $clip = $window.FindName("AvatarClip"); if ($clip) { $clip.Visibility = "Visible" }
            $gl = $window.FindName("AvatarGlyph"); if ($gl) { $gl.Visibility = "Collapsed" }
        } catch {}
    }
}

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

# Başlıktaki çıkış (logout) butonu — öğelerle aynı köprü (Tag → sinyal dosyası).
$logoutBtn = $window.FindName("LogoutBtn")
if ($logoutBtn -and $logoutBtn.Tag) {
    $logoutBtn.Add_MouseLeftButtonUp({
        try { Set-Content -Path $signalFile -Value $this.Tag -Force -Encoding UTF8 } catch {}
        try { $window.Close() } catch {}
    })
}

$window.Add_Loaded({
    # Konumlandırma: İmlecin konumunu alıp onun yanına yerleştirelim!
    $cursor = [System.Windows.Forms.Cursor]::Position
    $cx = [double]$cursor.X
    $cy = [double]$cursor.Y

    # Fiziksel px -> DIP (DPI ölçekleme)
    $src = [System.Windows.PresentationSource]::FromVisual($window)
    if ($src -and $src.CompositionTarget) {
        $p = $src.CompositionTarget.TransformFromDevice.Transform([System.Windows.Point]::new($cx, $cy))
        $cx = $p.X
        $cy = $p.Y
    }

    $w = $window.ActualWidth
    $h = $window.ActualHeight
    
    # Ekran sınırlarına kelepçele (menü ekrandan taşmasın)
    $screen = [System.Windows.Forms.Screen]::FromPoint($cursor)
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

    # Menüyü konumlandır (sağ altta olduğunda sola ve yukarı açılması varsayılan)
    $left = $cx - $w
    $top = $cy - $h

    # Kelepçeleme kuralları
    if ($left -lt $bx) { $left = $cx }
    if ($top -lt $by) { $top = $cy }
    if (($left + $w) -gt ($bx + $bw)) { $left = ($bx + $bw) - $w - 4 }
    if (($top + $h) -gt ($by + $bh)) { $top = ($by + $bh) - $h - 4 }

    $window.Left = $left
    $window.Top = $top

    $window.Activate()
    $window.Focus() | Out-Null
})

$window.Add_Deactivated({
    try { $window.Close() } catch {}
})

$window.Add_KeyDown({ if ($_.Key -eq 'Escape') { try { $window.Close() } catch {} } })

$window.ShowDialog() | Out-Null
"###;
