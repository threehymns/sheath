//! The window's backdrop material: Mica or Acrylic on Windows, and the system's own vibrancy
//! (through Rune's system materials) on macOS.
//!
//! The desktop draws a material *behind* the window, so the window is created transparent on
//! the platforms that have one (see `launch`), and the shell's background is a translucent tint
//! while a material is chosen. The tint is the kit's `bg_deep`, which paints the sidebar and
//! the frame around the content card; the card itself stays solid, so text keeps its contrast.
//!
//! On Linux only a KDE Plasma Wayland session has an equivalent: KWin blurs the window when the
//! app asks it to, which `kwin` does. Every other Linux session and the web have none, so the
//! setting is not offered there and a value written into `config.kdl` by hand leaves the window
//! exactly as it was.

#[cfg(target_os = "linux")]
mod kwin;

#[cfg(any(target_os = "windows", target_os = "macos"))]
use rune::WindowHandle;
use rune_kit::Kit;
use rune_theme_opencode as theme;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

/// The backdrop the desktop draws behind the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Material {
    /// No material: the window paints its own solid background.
    #[default]
    Off,
    /// Windows: Mica, a wallpaper tint. macOS: the popover vibrancy material.
    Mica,
    /// Windows: Acrylic, a blur of the desktop. macOS: Liquid Glass (the popover material
    /// before macOS 26).
    Acrylic,
}

impl Material {
    pub const ALL: &'static [Material] = &[Material::Off, Material::Mica, Material::Acrylic];

    /// The name `config.kdl` stores.
    pub const FILE_NAME: &'static str = "window-material";

    /// What the settings row says under its label.
    pub const HELP: &'static str = if cfg!(target_os = "macos") {
        "The system's own vibrancy behind the window. Off keeps the solid background."
    } else if cfg!(target_os = "linux") {
        "KWin's blur behind the window, in a Plasma Wayland session. Off keeps the solid background."
    } else {
        "Windows 11's Mica and Acrylic backdrops behind the window. Off keeps the solid background."
    };

    pub fn name(self) -> &'static str {
        match self {
            Material::Off => "off",
            Material::Mica => "mica",
            Material::Acrylic => "acrylic",
        }
    }

    pub fn from_name(name: &str) -> Option<Material> {
        Material::ALL
            .iter()
            .copied()
            .find(|material| material.name() == name)
    }

    /// Whether this session draws a material behind its windows at all. Windows and macOS do;
    /// elsewhere the settings page leaves the row out.
    #[cfg(not(target_os = "linux"))]
    pub fn supported() -> bool {
        cfg!(any(target_os = "windows", target_os = "macos"))
    }

    /// A Plasma Wayland session has KWin blur on request; every other Linux session, and the
    /// web, has no material.
    #[cfg(target_os = "linux")]
    pub fn supported() -> bool {
        kwin::detected()
    }

    /// The choices the settings panel offers. Windows and macOS each ask their system for two
    /// materials; KWin has a single blur, so Linux names only that.
    #[cfg(not(target_os = "linux"))]
    pub fn choices() -> &'static [Material] {
        Material::ALL
    }

    #[cfg(target_os = "linux")]
    pub fn choices() -> &'static [Material] {
        &[Material::Off, Material::Acrylic]
    }

    /// The choice to show for this value where the platform names fewer of them, so a value
    /// written by hand for Windows or macOS still reads as KWin's single blur.
    pub fn choice(self) -> Material {
        if cfg!(target_os = "linux") && self != Material::Off {
            return Material::Acrylic;
        }
        self
    }

    /// The label the settings page shows for this choice.
    pub fn label(self) -> &'static str {
        if cfg!(target_os = "macos") {
            match self {
                Material::Off => "Off",
                Material::Mica => "Vibrancy",
                Material::Acrylic => "Glass",
            }
        } else if cfg!(target_os = "linux") {
            match self {
                Material::Off => "Off",
                _ => "Blur",
            }
        } else {
            match self {
                Material::Off => "Off",
                Material::Mica => "Mica",
                Material::Acrylic => "Acrylic",
            }
        }
    }

    /// The alpha the shell's background takes while this material is on. The material is what
    /// the sidebar and the frame around the content card are made of, so the tint stays light
    /// enough to show it; the card itself is not tinted, so rows and text keep the background
    /// they were designed against.
    fn tint_alpha(self) -> Option<f32> {
        match self {
            Material::Off => None,
            Material::Mica => Some(0.35),
            Material::Acrylic => Some(0.25),
        }
    }
}

/// The theme's kit, with the shell's background tinted while a material is on. Cached, because
/// callers hold it as `&'static Kit` for the lifetime of a frame, and a kit is cheap only once.
pub fn kit(dark: bool, material: Material) -> &'static Kit {
    static KITS: [[OnceLock<Kit>; 3]; 2] = [const { [const { OnceLock::new() }; 3] }; 2];
    let material = effective(material);
    KITS[usize::from(dark)][material as usize].get_or_init(|| {
        let mut kit = theme::kit(dark).clone();
        if let Some(alpha) = material.tint_alpha() {
            kit.c.bg_deep = kit.c.bg_deep.alpha(alpha);
        }
        kit
    })
}

/// The element that asks Rune for a native macOS material under the window's content. Windows
/// asks DWM for its backdrop instead, and the other platforms have nothing to ask.
#[cfg(target_os = "macos")]
pub fn backdrop(material: Material) -> Option<rune::El> {
    use rune::SystemMaterial;
    let material = match effective(material) {
        Material::Off => return None,
        Material::Mica => SystemMaterial::Popover,
        Material::Acrylic => SystemMaterial::Glass,
    };
    Some(
        rune::el()
            .key("window-material")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .system_material(material),
    )
}

#[cfg(not(target_os = "macos"))]
pub fn backdrop(_material: Material) -> Option<rune::El> {
    None
}

/// Creates the window so a material can be drawn behind it, and keeps its handle for applying
/// one when the setting changes while the app runs. Elsewhere the app is left as it was.
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn configure<S: 'static>(app: rune::App<S>) -> rune::App<S> {
    app.window_options(rune::WindowOptions {
        transparent: true,
        ..Default::default()
    })
    .on_window(|window| remember(window.clone()))
}

/// A Plasma Wayland session draws its blur behind a transparent window, so the window is
/// created transparent and handed to `kwin` as soon as it exists. Other Linux sessions are
/// left alone, as the web is.
#[cfg(target_os = "linux")]
pub fn configure<S: 'static>(app: rune::App<S>) -> rune::App<S> {
    if !kwin::detected() {
        return app;
    }
    app.window_options(rune::WindowOptions {
        transparent: true,
        ..Default::default()
    })
    .on_window(kwin::attach)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn configure<S: 'static>(app: rune::App<S>) -> rune::App<S> {
    app
}

/// The window's native handle, kept across native creations so a changed setting can reach the
/// window that is on screen.
#[cfg(any(target_os = "windows", target_os = "macos"))]
static WINDOW: OnceLock<std::sync::Mutex<Option<WindowHandle>>> = OnceLock::new();

/// The chosen material, for a window created after the choice was made.
static CURRENT: AtomicU8 = AtomicU8::new(0);

/// The appearance the app draws in. Windows asks DWM for the light or dark variant of its
/// material by this flag, and its own default is the light one, which paints a pale backdrop
/// under a dark app.
#[cfg(target_os = "windows")]
static DARK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Whether the desktop actually draws a material: 0 before the window exists, 1 when it does,
/// 2 when the platform refused the request (Windows 10, say).
static NATIVE: AtomicU8 = AtomicU8::new(0);

/// Remembers a freshly created window and applies the material the app already chose.
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn remember(window: WindowHandle) {
    *WINDOW.get_or_init(Default::default).lock().unwrap() = Some(window);
    apply(remembered());
}

/// The material the app last chose, for a window created after the choice was made.
pub(super) fn remembered() -> Material {
    from_index(CURRENT.load(Ordering::Relaxed))
}

/// Tells the material which appearance the app draws in, so the window's material is the same
/// light or dark variant as its content and title bar. Only Windows keeps the two apart.
pub fn appearance(dark: bool) {
    #[cfg(target_os = "windows")]
    {
        if DARK.swap(dark, Ordering::Relaxed) != dark {
            apply_windows(remembered());
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = dark;
}

/// Records whether the desktop drew what was asked, so the shell keeps its solid background
/// where it did not.
#[cfg(target_os = "linux")]
pub(super) fn remember_native(drawn: bool) {
    NATIVE.store(if drawn { 1 } else { 2 }, Ordering::Relaxed);
}

/// Applies a material to the window. A window that does not exist yet keeps the choice for the
/// one that opens; a platform without materials keeps its solid background.
pub fn apply(material: Material) {
    CURRENT.store(material as u8, Ordering::Relaxed);
    #[cfg(target_os = "windows")]
    apply_windows(material);
    #[cfg(target_os = "macos")]
    {
        let _ = material;
        NATIVE.store(1, Ordering::Relaxed);
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(drawn) = kwin::set(material) {
            remember_native(drawn);
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = material;
        NATIVE.store(2, Ordering::Relaxed);
    }
}

/// What the shell draws as: the chosen material when the desktop can draw it, otherwise `Off`.
fn effective(material: Material) -> Material {
    if material == Material::Off || !Material::supported() {
        return Material::Off;
    }
    if NATIVE.load(Ordering::Relaxed) == 2 {
        return Material::Off;
    }
    material
}

fn from_index(index: u8) -> Material {
    match index {
        1 => Material::Mica,
        2 => Material::Acrylic,
        _ => Material::Off,
    }
}

/// Gives the window a DWM backdrop that the app's own translucent pixels let through: Mica,
/// Acrylic, or none. Windows 10 and older answer with an error and keep the solid shell.
#[cfg(target_os = "windows")]
fn apply_windows(material: Material) {
    use rune::raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmExtendFrameIntoClientArea,
        DwmSetWindowAttribute,
    };
    use windows_sys::Win32::UI::Controls::MARGINS;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER,
        SetWindowPos,
    };

    // DWMSBT_NONE, DWMSBT_MAINWINDOW and DWMSBT_TRANSIENTWINDOW.
    let backdrop = match material {
        Material::Off => 1,
        Material::Mica => 2,
        Material::Acrylic => 3,
    };
    // DWM glazes the window's frame, and -1 on every side is its "sheet of glass": the client
    // area becomes part of the frame, which is what carries the backdrop behind the app's own
    // translucent pixels. Without it only the title bar has a material, and the shell blends
    // with the desktop instead, which reads as a flat, lighter grey.
    let margins = if backdrop == 1 {
        MARGINS::default()
    } else {
        MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        }
    };
    let Some(window) = WINDOW
        .get()
        .and_then(|window| window.lock().unwrap().clone())
    else {
        return;
    };
    let result = window.with_raw_handle(|handle| match handle.as_raw() {
        RawWindowHandle::Win32(handle) => {
            let hwnd = handle.hwnd.get() as *mut core::ffi::c_void;
            let dark = i32::from(DARK.load(Ordering::Relaxed));
            // SAFETY: `hwnd` is the live window this handle came from, read on the UI thread.
            unsafe {
                DwmExtendFrameIntoClientArea(hwnd, &margins);
                // The margins only apply as a frame change, so ask for one without moving,
                // resizing, reordering or activating the window.
                SetWindowPos(
                    hwnd,
                    core::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED
                        | SWP_NOMOVE
                        | SWP_NOSIZE
                        | SWP_NOZORDER
                        | SWP_NOOWNERZORDER
                        | SWP_NOACTIVATE,
                );
                // The title bar and the material's own variant follow this flag, and DWM
                // defaults it off: a dark app would get the light Mica behind its chrome.
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                    &dark as *const i32 as *const core::ffi::c_void,
                    std::mem::size_of::<i32>() as u32,
                );
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_SYSTEMBACKDROP_TYPE as u32,
                    &backdrop as *const i32 as *const core::ffi::c_void,
                    std::mem::size_of::<i32>() as u32,
                )
            }
        }
        _ => -1,
    });
    NATIVE.store(
        if matches!(result, Ok(0)) { 1 } else { 2 },
        Ordering::Relaxed,
    );
}
