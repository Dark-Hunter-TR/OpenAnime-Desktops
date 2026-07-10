use std::fs;

#[derive(Debug, serde::Serialize, Clone)]
pub struct GstReport {
    pub gstreamer_installed: bool,
    pub missing_elements: Vec<String>,
    pub recommended_command: String,
    pub recommended_packages: String,
}

pub fn detect_gstreamer() -> GstReport {
    #[cfg(target_os = "linux")]
    {
        // 1. Check if GStreamer libraries are loadable and initialize
        if gstreamer::init().is_err() {
            let distro = determine_distro();
            let (pkg, cmd) = get_gst_install_instructions("all", &distro);
            return GstReport {
                gstreamer_installed: false,
                missing_elements: vec!["gstreamer-core".to_string()],
                recommended_command: cmd,
                recommended_packages: pkg,
            };
        }

        // 2. Check for required elements in GStreamer registry
        let required_elements = vec![
            "playbin3",
            "uridecodebin3",
            "subtitleoverlay",
            "assrender",
            "autoaudiosink",
            "videoconvert",
            "appsink",
        ];

        let mut missing = Vec::new();
        for el in required_elements {
            if gstreamer::ElementFactory::find(el).is_none() {
                missing.push(el.to_string());
            }
        }

        let gstreamer_installed = missing.is_empty();
        let (recommended_packages, recommended_command) = if !gstreamer_installed {
            let distro = determine_distro();
            // Determine which GStreamer plugin set is missing
            let mut missing_category = "plugins";
            if missing.contains(&"assrender".to_string()) || missing.contains(&"subtitleoverlay".to_string()) {
                missing_category = "bad"; // gst-plugins-bad contains assrender
            }
            get_gst_install_instructions(missing_category, &distro)
        } else {
            (String::new(), String::new())
        };

        GstReport {
            gstreamer_installed,
            missing_elements: missing,
            recommended_command,
            recommended_packages,
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        GstReport {
            gstreamer_installed: true,
            missing_elements: Vec::new(),
            recommended_command: String::new(),
            recommended_packages: String::new(),
        }
    }
}

fn determine_distro() -> String {
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("ID=") {
                return line.trim_start_matches("ID=").trim_matches('"').to_string();
            }
        }
    }
    "unknown".to_string()
}

fn get_gst_install_instructions(category: &str, distro: &str) -> (String, String) {
    match distro {
        "arch" | "manjaro" => {
            let pkgs = match category {
                "bad" => "gst-plugins-bad libass".to_string(),
                "plugins" => "gst-plugins-base gst-plugins-good gst-plugins-bad gst-libav".to_string(),
                _ => "gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-libav gstreamer-vaapi".to_string(),
            };
            let cmd = format!("sudo pacman -S --noconfirm {}", pkgs);
            (pkgs, cmd)
        }
        "ubuntu" | "debian" | "pop" | "mint" => {
            let pkgs = match category {
                "bad" => "gstreamer1.0-plugins-bad libass-dev".to_string(),
                "plugins" => "gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav".to_string(),
                _ => "libgstreamer1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav gstreamer1.0-vaapi libass-dev".to_string(),
            };
            let cmd = format!("sudo apt update && sudo apt install -y {}", pkgs);
            (pkgs, cmd)
        }
        "fedora" => {
            let pkgs = match category {
                "bad" => "gstreamer1-plugins-bad-free".to_string(),
                "plugins" => "gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-plugins-bad-free gstreamer1-libav".to_string(),
                _ => "gstreamer1-devel gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-plugins-bad-free gstreamer1-libav gstreamer1-vaapi".to_string(),
            };
            let cmd = format!("sudo dnf install -y {}", pkgs);
            (pkgs, cmd)
        }
        "opensuse" | "opensuse-tumbleweed" => {
            let pkgs = match category {
                "bad" => "gstreamer-plugins-bad".to_string(),
                "plugins" => "gstreamer-plugins-base gstreamer-plugins-good gstreamer-plugins-bad gstreamer-plugins-libav".to_string(),
                _ => "gstreamer-devel gstreamer-plugins-base gstreamer-plugins-good gstreamer-plugins-bad gstreamer-plugins-libav".to_string(),
            };
            let cmd = format!("sudo zypper install -y {}", pkgs);
            (pkgs, cmd)
        }
        _ => (
            "GStreamer 1.0, plugins-base, plugins-good, plugins-bad, libav".to_string(),
            "Lütfen sisteminiz için GStreamer ve gerekli plugin paketlerini (base, good, bad, libav) kurun.".to_string(),
        ),
    }
}
