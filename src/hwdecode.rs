//! What `fresco check` / `frescod --check` can say about hardware decode, and
//! the install command to suggest, without assuming `vainfo` or `apt`.

use std::path::{Path, PathBuf};

use crate::cli::which;
use crate::config::GpuVendors;

/// The distro package manager, for install hints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pm {
    Apt,
    Dnf,
    Pacman,
    Zypper,
}

/// Package names for the `vainfo` tool, in [`Pm`] order (apt, dnf, pacman, zypper).
pub const VAINFO_PKG: [&str; 4] = ["vainfo", "libva-utils", "libva-utils", "libva-utils"];
/// VA-API drivers. dnf's `mesa-va-drivers-freeworld` is from RPM Fusion.
pub const DRIVER_PKGS: [&str; 4] = [
    "intel-media-va-driver mesa-va-drivers",
    "intel-media-driver mesa-va-drivers-freeworld",
    "intel-media-driver libva-mesa-driver",
    "intel-media-driver Mesa-libva",
];
/// `gdbus` (MPRIS widgets).
pub const GDBUS_PKG: [&str; 4] = ["libglib2.0-bin", "glib2", "glib2", "glib2-tools"];
/// `pw-cat` / `parec` (audio visualiser).
pub const AUDIO_PKGS: [&str; 4] = [
    "pipewire-bin or pulseaudio-utils",
    "pipewire-utils or pulseaudio-utils",
    "pipewire or libpulse",
    "pipewire-tools or pulseaudio-utils",
];

impl Pm {
    /// From the text of `/etc/os-release`: `ID` first, then each `ID_LIKE`
    /// word, so derivatives (Mint, Pop, Alma, EndeavourOS...) resolve.
    pub fn from_os_release(text: &str) -> Option<Pm> {
        let value = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
                .map(|v| v.trim().trim_matches('"').to_ascii_lowercase())
                .unwrap_or_default()
        };
        let (id, like) = (value("ID"), value("ID_LIKE"));
        std::iter::once(id.as_str())
            .chain(like.split_whitespace())
            .find_map(|id| match id {
                "debian" | "ubuntu" | "deepin" | "linuxmint" | "pop" => Some(Pm::Apt),
                "fedora" | "rhel" | "centos" => Some(Pm::Dnf),
                "arch" | "manjaro" | "endeavouros" => Some(Pm::Pacman),
                "suse" => Some(Pm::Zypper),
                id if id.starts_with("opensuse") => Some(Pm::Zypper),
                _ => None,
            })
    }

    /// `/etc/os-release`, else whichever package manager is on `PATH`.
    pub fn detect() -> Option<Pm> {
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|t| Pm::from_os_release(&t))
            .or_else(|| {
                [
                    ("apt-get", Pm::Apt),
                    ("dnf", Pm::Dnf),
                    ("pacman", Pm::Pacman),
                    ("zypper", Pm::Zypper),
                ]
                .into_iter()
                .find_map(|(bin, pm)| which(bin).then_some(pm))
            })
    }
}

/// `apt install <pkgs>` (or the dnf / pacman / zypper equivalent) with `pkgs`
/// in [`Pm`] order; a bare `install <apt names>` when the distro is unknown.
pub fn install_hint(pm: Option<Pm>, pkgs: [&str; 4]) -> String {
    match pm {
        Some(Pm::Apt) => format!("apt install {}", pkgs[0]),
        Some(Pm::Dnf) => format!("dnf install {}", pkgs[1]),
        Some(Pm::Pacman) => format!("pacman -S {}", pkgs[2]),
        Some(Pm::Zypper) => format!("zypper install {}", pkgs[3]),
        None => format!("install {}", pkgs[0]),
    }
}

/// What was found on disk. See [`classify`].
#[derive(Default)]
pub struct Facts {
    pub vainfo: bool,
    pub render_node: bool,
    /// Every render node is an NVIDIA GPU.
    pub only_nvidia: bool,
    pub nvcuvid: bool,
    pub va_driver: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum HwDecode {
    /// NVIDIA-only box with libnvcuvid: Fresco decodes with NVDEC, not VA-API.
    Nvdec,
    /// `vainfo` is installed, so the user can verify decode themselves.
    Vainfo,
    /// No `vainfo`, but a render node and a VA driver are on disk. The stack
    /// is most likely fine; only the diagnostic tool is missing.
    DriversPresent,
    /// No render node or no driver: hardware decode genuinely cannot work.
    Missing,
}

pub fn classify(f: &Facts) -> HwDecode {
    if f.only_nvidia && f.nvcuvid {
        HwDecode::Nvdec
    } else if f.vainfo {
        HwDecode::Vainfo
    } else if f.render_node && f.va_driver {
        HwDecode::DriversPresent
    } else {
        HwDecode::Missing
    }
}

pub fn probe() -> HwDecode {
    let vendors = render_node_vendors();
    let only_nvidia = vendors.nvidia && !vendors.intel && !vendors.amd;
    classify(&Facts {
        vainfo: which("vainfo"),
        render_node: render_node_present(),
        only_nvidia,
        nvcuvid: only_nvidia && nvcuvid_present(),
        va_driver: va_driver_present(),
    })
}

fn dir_has(dir: &Path, pred: impl Fn(&str) -> bool) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().any(|e| pred(&e.file_name().to_string_lossy())))
        .unwrap_or(false)
}

/// Any `/dev/dri/renderD*`, not just `renderD128` (multi-GPU boxes).
pub fn render_node_present() -> bool {
    dir_has(Path::new("/dev/dri"), |n| n.starts_with("renderD"))
}

fn render_node_vendors() -> GpuVendors {
    let ids = std::fs::read_dir("/sys/class/drm")
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("renderD"))
        .filter_map(|e| std::fs::read_to_string(e.path().join("device/vendor")).ok());
    GpuVendors::from_vendor_ids(ids)
}

/// `/usr/lib/*`, the multiarch directories (`x86_64-linux-gnu`, ...).
fn usr_lib_subdirs() -> Vec<PathBuf> {
    std::fs::read_dir("/usr/lib")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect()
}

/// A libva driver (`*_drv_video.so`) in `$LIBVA_DRIVERS_PATH` or the usual
/// per-distro directories. Inside Flatpak the drivers live in runtime
/// extensions we can't enumerate, so they are assumed present.
fn va_driver_present() -> bool {
    if Path::new("/.flatpak-info").exists() {
        return true;
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("LIBVA_DRIVERS_PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    dirs.extend(usr_lib_subdirs().into_iter().map(|d| d.join("dri")));
    dirs.extend(
        [
            "/usr/lib/dri",
            "/usr/lib64/dri",
            "/usr/lib64/dri-nonfree",
            "/usr/lib64/dri-freeworld",
        ]
        .map(PathBuf::from),
    );
    dirs.iter()
        .any(|d| dir_has(d, |n| n.ends_with("_drv_video.so")))
}

/// NVDEC's userspace half, `libnvcuvid.so.1`, in the common lib dirs or the
/// loader cache.
fn nvcuvid_present() -> bool {
    let mut dirs = vec![PathBuf::from("/usr/lib"), PathBuf::from("/usr/lib64")];
    dirs.extend(usr_lib_subdirs());
    dirs.iter().any(|d| d.join("libnvcuvid.so.1").exists())
        || std::process::Command::new("/sbin/ldconfig")
            .arg("-p")
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("libnvcuvid.so.1"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue #41: a missing `vainfo` binary must not read as "no VA-API" when
    /// the render node and a driver are right there.
    #[test]
    fn missing_vainfo_is_not_reported_as_missing_vaapi() {
        let f = |vainfo, render_node, only_nvidia, nvcuvid, va_driver| {
            classify(&Facts {
                vainfo,
                render_node,
                only_nvidia,
                nvcuvid,
                va_driver,
            })
        };
        assert_eq!(f(true, false, false, false, false), HwDecode::Vainfo);
        assert_eq!(f(false, true, false, false, true), HwDecode::DriversPresent);
        assert_eq!(f(false, true, false, false, false), HwDecode::Missing);
        assert_eq!(f(false, false, false, false, true), HwDecode::Missing);
        // NVIDIA-only: NVDEC wins over VA-API hints, even with vainfo around.
        assert_eq!(f(true, true, true, true, false), HwDecode::Nvdec);
        assert_eq!(f(false, true, true, true, false), HwDecode::Nvdec);
        // ...but without libnvcuvid it is just a box with no VA driver.
        assert_eq!(f(false, true, true, false, false), HwDecode::Missing);
    }

    #[test]
    fn os_release_maps_to_a_package_manager() {
        let pm = |s| Pm::from_os_release(s);
        assert_eq!(pm("ID=ubuntu\nID_LIKE=debian\n"), Some(Pm::Apt));
        assert_eq!(pm("ID=deepin\n"), Some(Pm::Apt));
        assert_eq!(
            pm("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n"),
            Some(Pm::Apt)
        );
        assert_eq!(pm("ID=\"fedora\"\n"), Some(Pm::Dnf));
        assert_eq!(
            pm("ID=almalinux\nID_LIKE=\"rhel centos fedora\"\n"),
            Some(Pm::Dnf)
        );
        assert_eq!(pm("ID=arch\n"), Some(Pm::Pacman));
        assert_eq!(pm("ID=cachyos\nID_LIKE=arch\n"), Some(Pm::Pacman));
        assert_eq!(pm("ID=endeavouros\nID_LIKE=arch\n"), Some(Pm::Pacman));
        assert_eq!(
            pm("ID=opensuse-tumbleweed\nID_LIKE=\"opensuse suse\"\n"),
            Some(Pm::Zypper)
        );
        assert_eq!(
            pm("ID=\"opensuse-leap\"\nID_LIKE=\"suse opensuse\"\n"),
            Some(Pm::Zypper)
        );
        assert_eq!(pm("ID=nixos\n"), None);
        assert_eq!(pm(""), None);
    }

    #[test]
    fn install_hints_use_the_detected_package_manager() {
        assert_eq!(
            install_hint(Some(Pm::Apt), VAINFO_PKG),
            "apt install vainfo"
        );
        assert_eq!(
            install_hint(Some(Pm::Dnf), VAINFO_PKG),
            "dnf install libva-utils"
        );
        assert_eq!(
            install_hint(Some(Pm::Pacman), DRIVER_PKGS),
            "pacman -S intel-media-driver libva-mesa-driver"
        );
        assert_eq!(
            install_hint(Some(Pm::Zypper), DRIVER_PKGS),
            "zypper install intel-media-driver Mesa-libva"
        );
        assert_eq!(
            install_hint(None, DRIVER_PKGS),
            "install intel-media-va-driver mesa-va-drivers"
        );
    }
}
