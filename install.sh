#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────
# OpenAnime Desktop — Evrensel Linux Kurulum Scripti
# Kullanım:
#   bash <(curl -s https://raw.githubusercontent.com/Dark-Hunter-TR/OpenAnime-Desktops/main/install.sh)
#   bash install.sh                    # yerel
#   bash install.sh --user            # sadece kullanıcı için kur
# ──────────────────────────────────────────────────────────

REPO_OWNER="Dark-Hunter-TR"
REPO_NAME="OpenAnime-Desktops"
REPO_URL="https://github.com/$REPO_OWNER/$REPO_NAME"
API_URL="https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases/latest"
BASE_URL="https://github.com/$REPO_OWNER/$REPO_NAME/releases/latest/download"

# Renkler
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

info()  { echo -e "${CYAN}[INFO]${NC} $1"; }
ok()    { echo -e "${GREEN}[OK]${NC} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
err()   { echo -e "${RED}[HATA]${NC} $1"; }

cleanup() {
    [[ -n "${TMPDIR:-}" && -d "$TMPDIR" ]] && rm -rf "$TMPDIR"
}
trap cleanup EXIT

usage() {
    cat <<EOF
OpenAnime Desktop Kurulum Scripti

Kullanım:
  bash install.sh              Sisteme kur (sudo gerekebilir)
  bash install.sh --user       Sadece kullanıcı için kur (~/.local/bin)
  bash install.sh --help       Bu yardım mesajı

Dağıtım otomatik algılanır:
  • Arch tabanlı (CachyOS, Arch, Manjaro, EndeavourOS) → PKGBUILD ile binary
  • Debian/Ubuntu/Mint/Pop   → .deb ile kurulum
  • Fedora/RHEL              → .rpm ile kurulum
  • Diğer (NixOS, Void, Solus, Gentoo) → AppImage
EOF
    exit 0
}

# ─── Dağıtım Algılama ──────────────────────────────────────
detect_distro() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        DISTRO_ID="$ID"
        DISTRO_LIKE="${ID_LIKE:-}"
        DISTRO_NAME="$NAME"
    elif [[ -f /etc/arch-release ]]; then
        DISTRO_ID="arch"
        DISTRO_LIKE=""
        DISTRO_NAME="Arch Linux"
    else
        DISTRO_ID="unknown"
        DISTRO_LIKE=""
        DISTRO_NAME="Linux"
    fi
    DISTRO_ID="${DISTRO_ID,,}"       # küçük harf
    DISTRO_LIKE="${DISTRO_LIKE,,}"

    info "Dağıtım tespit edildi: $DISTRO_NAME (id=$DISTRO_ID, like=$DISTRO_LIKE)"
}

# ─── Sudo Kontrolü ─────────────────────────────────────────
check_sudo() {
    if [[ "${INSTALL_MODE:-system}" == "user" ]]; then
        return 0  # --user modunda sudo gerekmez
    fi
    if [[ $EUID -ne 0 ]]; then
        if command -v sudo &>/dev/null; then
            info "Sudo yetkisi gerekiyor..."
            sudo -v || {
                err "Sudo yetkisi alınamadı. 'bash install.sh --user' dene."
                exit 1
            }
        else
            err "Bu script sudo gerektirir. root ile çalıştır veya --user dene."
            exit 1
        fi
    fi
}

# ─── En Son Release Bilgisini Al ───────────────────────────
get_latest_release() {
    info "En son sürüm bilgisi alınıyor..."
    if command -v curl &>/dev/null; then
        LATEST_TAG=$(curl -sL "$API_URL" | grep '"tag_name":' | head -1 | sed 's/.*"tag_name": "v//;s/".*//')
    elif command -v wget &>/dev/null; then
        LATEST_TAG=$(wget -qO- "$API_URL" | grep '"tag_name":' | head -1 | sed 's/.*"tag_name": "v//;s/".*//')
    else
        err "curl veya wget gerekli. Önce kur: sudo pacman -S curl (Arch) / sudo apt install curl (Debian)"
        exit 1
    fi

    if [[ -z "$LATEST_TAG" ]]; then
        warn "API'den sürüm alınamadı. GitHub rate limit'e takılmış olabilirsin."
        warn "En son release'i manuel kontrol et: $REPO_URL/releases"
        # Fallback: varsayılan bir tag dene
        LATEST_TAG="1.0.2-beta"
    fi

    ok "En son sürüm: v$LATEST_TAG"
}

# ─── Bağımlılık Kontrolü ───────────────────────────────────
check_deps() {
    local deps=("$@")
    local missing=()
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &>/dev/null; then
            missing+=("$dep")
        fi
    done
    if [[ ${#missing[@]} -gt 0 ]]; then
        warn "Eksik bağımlılıklar: ${missing[*]}"
        return 1
    fi
    return 0
}

install_deps_arch() {
    info "Gerekli paketler kontrol ediliyor..."
    local pkgs=("curl" "wget" "base-devel")
    if command -v pacman &>/dev/null; then
        sudo pacman -S --needed --noconfirm "${pkgs[@]}" 2>/dev/null || true
    fi
}

install_deps_debian() {
    local pkgs=("curl" "wget")
    if command -v apt-get &>/dev/null; then
        sudo apt-get update -qq
        sudo apt-get install -y -qq "${pkgs[@]}" 2>/dev/null || true
    fi
}

find_installed_launcher() {
    local candidate

    for candidate in \
        "$(command -v openanime-desktops 2>/dev/null || true)" \
        "$(command -v openanime 2>/dev/null || true)" \
        "$(command -v OpenAnime 2>/dev/null || true)"; do
        if [[ -n "$candidate" && -x "$candidate" ]]; then
            echo "$candidate"
            return 0
        fi
    done

    for candidate in \
        /usr/bin/openanime-desktops \
        /usr/bin/openanime \
        /usr/bin/OpenAnime \
        /usr/local/bin/openanime-desktops \
        /usr/local/bin/openanime \
        /usr/local/bin/OpenAnime \
        "$HOME/.local/bin/openanime-desktops" \
        "$HOME/.local/bin/openanime" \
        "$HOME/.local/bin/OpenAnime"; do
        if [[ -x "$candidate" ]]; then
            echo "$candidate"
            return 0
        fi
    done

    return 1
}

install_launcher_alias() {
    local source_bin="${1:-}"
    local target_bin

    if [[ -z "$source_bin" || ! -x "$source_bin" ]]; then
        return 1
    fi

    if [[ "${INSTALL_MODE:-system}" == "user" ]]; then
        mkdir -p "$HOME/.local/bin"
        target_bin="$HOME/.local/bin/openanime-desktops"
        ln -sfn "$source_bin" "$target_bin"
    else
        sudo mkdir -p /usr/bin
        target_bin="/usr/bin/openanime-desktops"
        sudo ln -sfn "$source_bin" "$target_bin"
        sudo mkdir -p /usr/local/bin
        sudo ln -sfn "$source_bin" /usr/local/bin/openanime-desktops 2>/dev/null || true
    fi

    echo "$target_bin"
}

extract_deb_payload() {
    local deb_path="$1"
    local extract_dir="$2"

    mkdir -p "$extract_dir"

    if command -v bsdtar &>/dev/null; then
        bsdtar -xf "$deb_path" -C "$extract_dir"
    elif command -v ar &>/dev/null; then
        (cd "$extract_dir" && ar x "$deb_path")
    else
        err "Deb paketini açmak için bsdtar veya ar gerekli."
        return 1
    fi

    local data_tar
    data_tar=$(find "$extract_dir" -maxdepth 1 -type f -name 'data.tar.*' | head -1)
    if [[ -z "$data_tar" ]]; then
        err "Deb veri katmanı bulunamadı."
        return 1
    fi

    if command -v bsdtar &>/dev/null; then
        bsdtar -xf "$data_tar" -C "$extract_dir"
    else
        tar -xf "$data_tar" -C "$extract_dir"
    fi
}

find_release_binary() {
    local root_dir="$1"
    local candidate

    for candidate in \
        "$root_dir/usr/bin/openanime-desktops" \
        "$root_dir/usr/bin/openanime" \
        "$root_dir/usr/bin/OpenAnime" \
        "$root_dir/usr/lib/openanime-desktops/openanime-desktops" \
        "$root_dir/usr/lib/openanime/openanime"; do
        if [[ -x "$candidate" ]]; then
            echo "$candidate"
            return 0
        fi
    done

    if [[ -d "$root_dir/usr/bin" ]]; then
        while IFS= read -r candidate; do
            if [[ -x "$candidate" ]]; then
                echo "$candidate"
                return 0
            fi
        done < <(find "$root_dir/usr/bin" -maxdepth 1 -type f -perm -111 2>/dev/null)
    fi

    return 1
}

install_desktop_entry() {
    local exec_line="$1"
    local icon_line="${2:-openanime-desktops}"
    local resolved_exec="$exec_line"

    if [[ "$exec_line" == "openanime-desktops" ]]; then
        if [[ "${INSTALL_MODE:-system}" == "user" ]]; then
            resolved_exec="$HOME/.local/bin/openanime-desktops"
        else
            resolved_exec="/usr/bin/openanime-desktops"
        fi
    fi

    if [[ "${INSTALL_MODE:-system}" == "user" ]]; then
        mkdir -p "$HOME/.local/share/applications"
        cat > "$HOME/.local/share/applications/openanime-desktops.desktop" << EOF
[Desktop Entry]
Name=OpenAnime Desktop
Comment=OpenAnime masaüstü istemcisi
Exec=$resolved_exec
Terminal=false
Type=Application
Categories=Network;WebBrowser;
Icon=$icon_line
EOF
    else
        sudo mkdir -p /usr/share/applications
        sudo tee /usr/share/applications/openanime-desktops.desktop > /dev/null << EOF
[Desktop Entry]
Name=OpenAnime Desktop
Comment=OpenAnime masaüstü istemcisi
Exec=$resolved_exec
Terminal=false
Type=Application
Categories=Network;WebBrowser;
Icon=$icon_line
EOF
    fi
}

# ─── Yöntem 1: Arch tabanlı (PKGBUILD binary) ──────────────
install_arch_pkgbuild() {
    info "Arch tabanlı sistem tespit edildi. PKGBUILD ile binary kurulum..."

    local pkgbuild_dir
    pkgbuild_dir=$(mktemp -d)
    TMPDIR="$pkgbuild_dir"

    # GitHub'dan PKGBUILD dosyasını indir
    if command -v curl &>/dev/null; then
        curl -sL "$REPO_URL/raw/main/packaging/arch/PKGBUILD" -o "$pkgbuild_dir/PKGBUILD"
    else
        wget -qO "$pkgbuild_dir/PKGBUILD" "$REPO_URL/raw/main/packaging/arch/PKGBUILD"
    fi

    if [[ ! -f "$pkgbuild_dir/PKGBUILD" ]]; then
        err "PKGBUILD indirilemedi!"
        exit 1
    fi

    # Veriyi çek varmış gibi göster, doğrudan binary indir
    cd "$pkgbuild_dir"

    # Sadece binary source'u indir (build atla)
    info "Binary indiriliyor (15 MB)..."
    makepkg -o --noconfirm 2>/dev/null || true

    if [[ ! -f "$pkgbuild_dir/openanime-desktops-$LATEST_TAG.deb" ]]; then
        local deb_url="$BASE_URL/openanime_$LATEST_TAG\_amd64.deb"
        local deb_file="$pkgbuild_dir/openanime-desktops-$LATEST_TAG.deb"
        info "makepkg ile indirme başarısız, doğrudan release .deb indiriliyor..."
        wget -q "$deb_url" -O "$deb_file" || curl -sL "$deb_url" -o "$deb_file"
    fi

    # PKGBUILD'i düzenle: source'u güncelle
    sed -i "s/pkgver=.*/pkgver=${LATEST_TAG//-/_}/" "$pkgbuild_dir/PKGBUILD"
    sed -i "s/_pkgver=.*/_pkgver=$LATEST_TAG/" "$pkgbuild_dir/PKGBUILD"

    info "Paket kuruluyor (sudo gerekebilir)..."
    makepkg -si --noconfirm 2>/dev/null || {
        warn "makepkg -si başarısız. Doğrudan .deb çıkarılıyor..."

        local deb_path
        local extract_dir
        local release_binary

        deb_path=$(ls "$pkgbuild_dir/"*.deb 2>/dev/null | head -1)
        if [[ -z "$deb_path" ]]; then
            err "Kurulum için .deb dosyası bulunamadı."
            exit 1
        fi

        extract_dir="$pkgbuild_dir/extracted"
        extract_deb_payload "$deb_path" "$extract_dir" || {
            err "Deb paketi açılamadı."
            exit 1
        }

        release_binary=$(find_release_binary "$extract_dir") || {
            err "Çıkarılan paket içinde çalıştırılabilir binary bulunamadı."
            exit 1
        }

        if [[ "${INSTALL_MODE:-system}" == "user" ]]; then
            mkdir -p "$HOME/.local/bin"
            install -Dm755 "$release_binary" "$HOME/.local/bin/$(basename "$release_binary")"
            install_launcher_alias "$HOME/.local/bin/$(basename "$release_binary")"
            install_desktop_entry "openanime-desktops"
        else
            sudo mkdir -p /usr/bin
            sudo install -Dm755 "$release_binary" "/usr/bin/$(basename "$release_binary")"
            install_launcher_alias "/usr/bin/$(basename "$release_binary")"
            install_desktop_entry "openanime-desktops"
        fi
    }

    if ! install_launcher_alias "$(find_installed_launcher)" >/dev/null 2>&1; then
        warn "Çalıştırılabilir komut bulunamadı; openanime-desktops kısayolu oluşturulamadı."
    fi

    install_desktop_entry "openanime-desktops"

    ok "OpenAnime Desktop başarıyla kuruldu!"
    info "Çalıştırmak için: openanime-desktops"
}

# ─── Yöntem 2: Debian tabanlı (.deb) ───────────────────────
install_deb() {
    info "Debian tabanlı sistem tespit edildi. .deb ile kurulum..."

    local tmpdir
    tmpdir=$(mktemp -d)
    TMPDIR="$tmpdir"

    local deb_url="$BASE_URL/openanime_$LATEST_TAG\_amd64.deb"
    local deb_file="$tmpdir/openanime.deb"

    info "İndiriliyor: $deb_url"
    if command -v wget &>/dev/null; then
        wget -q --show-progress "$deb_url" -O "$deb_file"
    else
        curl -#L "$deb_url" -o "$deb_file"
    fi

    if [[ ! -f "$deb_file" ]]; then
        err ".deb dosyası indirilemedi!"
        exit 1
    fi

    info "Kuruluyor..."
    sudo dpkg -i "$deb_file" || {
        warn "Bağımlılık hatası oluştu. Tamamlanıyor..."
        sudo apt-get install -f -y -qq
    }

    if ! install_launcher_alias "$(find_installed_launcher)" >/dev/null 2>&1; then
        warn "Kurulu binary bulunamadı; openanime-desktops kısayolu oluşturulamadı."
    fi

    ok "OpenAnime Desktop başarıyla kuruldu!"
    info "Çalıştırmak için: openanime-desktops"
}

# ─── Yöntem 3: Fedora/RHEL (.rpm) ──────────────────────────
install_rpm() {
    info "Fedora/RHEL tespit edildi. .rpm ile kurulum..."

    local rpm_url="$BASE_URL/openanime-$LATEST_TAG-1.x86_64.rpm"

    info "İndiriliyor: $rpm_url"
    if command -v dnf &>/dev/null; then
        sudo dnf install -y "$rpm_url"
    elif command -v yum &>/dev/null; then
        sudo yum install -y "$rpm_url"
    elif command -v zypper &>/dev/null; then
        sudo zypper install -y "$rpm_url"
    else
        err "RPM yöneticisi bulunamadı (dnf/yum/zypper)"
        exit 1
    fi

    ok "OpenAnime Desktop başarıyla kuruldu!"
    info "Çalıştırmak için: openanime-desktops"
}

# ─── Yöntem 4: AppImage (tüm dağıtımlar) ───────────────────
install_appimage() {
    info "AppImage ile kurulum yapılıyor..."

    local appimage_url
    local appimage_name="OpenAnime_${LATEST_TAG}_amd64.AppImage"

    # Önce .deb'den küçük binary dene
    local deb_url="$BASE_URL/openanime_$LATEST_TAG\_amd64.deb"
    local tmpdir
    tmpdir=$(mktemp -d)
    TMPDIR="$tmpdir"

    info "Önce .deb içinden binary alınmaya çalışılıyor (daha küçük)..."
    wget -q "$deb_url" -O "$tmpdir/openanime.deb" 2>/dev/null || curl -sL "$deb_url" -o "$tmpdir/openanime.deb" 2>/dev/null || true

    if [[ -f "$tmpdir/openanime.deb" ]]; then
        # .deb içinden binary çıkar
        cd "$tmpdir"
        bsdtar -xf "$tmpdir/openanime.deb" 2>/dev/null || true
        bsdtar -xf "$tmpdir/data.tar."* 2>/dev/null || true

        local binary
        binary=$(find "$tmpdir/usr" -name "openanime-desktops" -type f 2>/dev/null | head -1)
        if [[ -n "$binary" ]]; then
            if [[ "${INSTALL_MODE:-system}" == "user" ]]; then
                mkdir -p "$HOME/.local/bin"
                install -Dm755 "$binary" "$HOME/.local/bin/openanime-desktops"
                # desktop entry
                mkdir -p "$HOME/.local/share/applications"
                cat > "$HOME/.local/share/applications/openanime-desktops.desktop" << EOF
[Desktop Entry]
Name=OpenAnime Desktop
Comment=OpenAnime masaüstü istemcisi
Exec=$HOME/.local/bin/openanime-desktops
Terminal=false
Type=Application
Categories=Network;WebBrowser;
Icon=openanime-desktops
EOF
                # icon'u da kopyala
                find "$tmpdir" -name "*.png" -path "*/icons/*" -exec install -Dm644 {} "$HOME/.local/share/icons/hicolor/512x512/apps/openanime-desktops.png" \; 2>/dev/null || true
                ok "OpenAnime Desktop kuruldu: ~/.local/bin/openanime-desktops"
            else
                sudo install -Dm755 "$binary" /usr/bin/openanime-desktops
                # desktop entry
                sudo mkdir -p /usr/share/applications
                sudo tee /usr/share/applications/openanime-desktops.desktop > /dev/null << EOF
[Desktop Entry]
Name=OpenAnime Desktop
Comment=OpenAnime masaüstü istemcisi
Exec=/usr/bin/openanime-desktops
Terminal=false
Type=Application
Categories=Network;WebBrowser;
Icon=openanime-desktops
EOF
                find "$tmpdir" -name "*.png" -path "*/icons/*" -exec sudo install -Dm644 {} /usr/share/icons/hicolor/512x512/apps/openanime-desktops.png \; 2>/dev/null || true
                ok "OpenAnime Desktop kuruldu: /usr/bin/openanime-desktops"
            fi
            return 0
        fi
    fi

    # .deb'den binary çıkmazsa AppImage'a düş
    warn ".deb içinden binary alınamadı, AppImage kullanılıyor (daha büyük ~120 MB)..."
    appimage_url="$BASE_URL/$appimage_name"

    local dest
    if [[ "${INSTALL_MODE:-system}" == "user" ]]; then
        mkdir -p "$HOME/.local/bin"
        dest="$HOME/.local/bin/OpenAnime.AppImage"
    else
        sudo mkdir -p /opt/openanime
        dest="/opt/openanime/OpenAnime.AppImage"
    fi

    info "İndiriliyor: $appimage_url"
    if command -v wget &>/dev/null; then
        wget -q --show-progress "$appimage_url" -O "$dest"
    else
        curl -#L "$appimage_url" -o "$dest"
    fi
    chmod +x "$dest"

    if [[ "${INSTALL_MODE:-system}" != "user" ]]; then
        # Symlink
        sudo mkdir -p /usr/bin
        sudo ln -sf "$dest" /usr/bin/openanime-desktops 2>/dev/null || true
        sudo mkdir -p /usr/local/bin
        sudo ln -sf "$dest" /usr/local/bin/openanime-desktops 2>/dev/null || true
    fi

    # Desktop entry
    local desktop_dest
    local bin_path
    if [[ "${INSTALL_MODE:-system}" == "user" ]]; then
        desktop_dest="$HOME/.local/share/applications"
        bin_path="openanime-desktops"
        mkdir -p "$desktop_dest"
    else
        desktop_dest="/usr/share/applications"
        bin_path="openanime-desktops"
        sudo mkdir -p "$desktop_dest"
    fi

    cat > "/tmp/openanime-desktops.desktop" << EOF
[Desktop Entry]
Name=OpenAnime Desktop
Comment=OpenAnime masaüstü istemcisi (AppImage)
Exec=$bin_path
Terminal=false
Type=Application
Categories=Network;WebBrowser;
Icon=openanime-desktops
EOF

    if [[ "${INSTALL_MODE:-system}" == "user" ]]; then
        mv /tmp/openanime-desktops.desktop "$desktop_dest/"
    else
        sudo mv /tmp/openanime-desktops.desktop "$desktop_dest/"
    fi

    # Icon (AppImage içinden çıkar)
    "$dest" --appimage-extract "*.png" 2>/dev/null || true
    if [[ -d "squashfs-root" ]]; then
        local icon_file
        icon_file=$(find "squashfs-root" -name "icon.png" -type f | head -1)
        if [[ -n "$icon_file" ]]; then
            if [[ "${INSTALL_MODE:-system}" == "user" ]]; then
                mkdir -p "$HOME/.local/share/icons/hicolor/512x512/apps"
                cp "$icon_file" "$HOME/.local/share/icons/hicolor/512x512/apps/openanime-desktops.png"
            else
                sudo mkdir -p /usr/share/icons/hicolor/512x512/apps
                sudo cp "$icon_file" /usr/share/icons/hicolor/512x512/apps/openanime-desktops.png
            fi
        fi
        rm -rf squashfs-root
    fi

    if ! install_launcher_alias "$(find_installed_launcher)" >/dev/null 2>&1; then
        if [[ "${INSTALL_MODE:-system}" == "user" ]]; then
            ln -sfn "$dest" "$HOME/.local/bin/openanime-desktops" 2>/dev/null || true
        else
            sudo ln -sfn "$dest" /usr/bin/openanime-desktops 2>/dev/null || true
            sudo ln -sfn "$dest" /usr/local/bin/openanime-desktops 2>/dev/null || true
        fi
    fi

    ok "OpenAnime Desktop kuruldu!"
    info "Çalıştırmak için: openanime-desktops"
}

# ─── Ana Akış ──────────────────────────────────────────────
main() {
    echo ""
    echo -e "${CYAN}╔════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║     OpenAnime Desktop — Linux Kurulum     ║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════════════╝${NC}"
    echo ""

    # Parametreler
    INSTALL_MODE="system"
    for arg in "$@"; do
        case "$arg" in
            --user) INSTALL_MODE="user" ;;
            --help|-h) usage ;;
        esac
    done

    detect_distro
    check_sudo
    get_latest_release

    # Dağıtım bazlı karar
    case "$DISTRO_ID" in
        arch|manjaro|endeavour*|artix|arcolinux|cachy*)
            install_deps_arch
            install_arch_pkgbuild
            ;;
        *)
            # ID_LIKE kontrolü
            case "$DISTRO_LIKE" in
                *arch*)
                    install_deps_arch
                    install_arch_pkgbuild
                    ;;
                *debian*|*ubuntu*)
                    install_deps_debian
                    install_deb
                    ;;
                *fedora*|*rhel*|*centos*)
                    install_rpm
                    ;;
                *suse*)
                    install_rpm
                    ;;
                *)
                    # Diğer tüm dağıtımlar → AppImage veya .deb çıkarma
                    install_appimage
                    ;;
            esac
            ;;
    esac

    # Masaüstü kısayol güncelle
    if command -v update-desktop-database &>/dev/null; then
        if [[ "${INSTALL_MODE:-system}" == "user" ]]; then
            update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
        else
            sudo update-desktop-database 2>/dev/null || true
        fi
    fi

    echo ""
    ok "Kurulum tamamlandı! 🎉"
    echo ""
    echo -e "  ${CYAN}▶ Çalıştırmak için:${NC} openanime-desktops"
    echo -e "  ${CYAN}▶ Kaldırmak için:${NC}"
    echo -e "     Arch:  sudo pacman -R openanime-desktops"
    echo -e "     Debian: sudo apt remove openanime-desktops"
    echo -e "     Fedora: sudo dnf remove openanime-desktops"
    echo -e "     Diğer:  rm /usr/bin/openanime-desktops (veya ~/.local/bin/openanime-desktops)"
    echo ""
}

main "$@"
