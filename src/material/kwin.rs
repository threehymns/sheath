//! KWin's blur behind the window.
//!
//! A Wayland client has to ask for the blur; the compositor draws none behind a surface that
//! never asked. Rune's winit can ask, but Rune hands out the window's raw handles only, so this
//! module asks itself — on the connection winit already owns, because a protocol object can only
//! be created for a surface on the connection that owns it. `wayland-client` documents this
//! route: `Backend::from_foreign_display` wraps winit's `wl_display` as a guest, and
//! `ObjectId::from_ptr` with `Proxy::from_id` wraps its `wl_surface` without taking ownership of
//! it (the object data stays winit's, so dropping the wrapper cannot destroy its surface).
//!
//! Plasma 6.7 and later (and Mutter and niri) take the standard
//! `ext_background_effect_manager_v1`; earlier Plasma takes KWin's own
//! `org_kde_kwin_blur_manager`, which Plasma 6.7 dropped. Both are asked for, whichever the
//! compositor advertises.

use crate::material::Material;
use rune::WindowHandle;
use rune::raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::cell::RefCell;
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_region::WlRegion;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1;
use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1;
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur::OrgKdeKwinBlur;
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager;

/// A blur region big enough for any window. It is in surface coordinates, and the compositor
/// clips it to the surface, so covering everything needs no knowledge of the window's size.
const COVER: i32 = 1 << 20;

/// Whether this is a session KWin would blur in: a Wayland session on KDE Plasma. A Plasma X11
/// session, another desktop's Wayland session and the web keep the solid background, and the
/// settings panel leaves the row out there.
pub fn detected() -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return false;
    }
    if matches!(std::env::var("WINIT_UNIX_BACKEND").as_deref(), Ok("x11")) {
        return false;
    }
    std::env::var("XDG_CURRENT_DESKTOP")
        .is_ok_and(|desktop| desktop.to_ascii_uppercase().contains("KDE"))
}

/// Asks the compositor to blur behind the window's surface, or stops it. Called from
/// `on_window` with the window Rune just created, and again whenever the setting changes.
pub fn attach(window: &WindowHandle) {
    let Some(mut session) = Session::new(window) else {
        super::remember_native(false);
        return;
    };
    let supported = session.show(super::remembered());
    super::remember_native(supported);
    SESSION.with_borrow_mut(|slot| *slot = Some(session));
}

/// Blurs the window or stops blurring it: `Some(true)` when the request was sent, `Some(false)`
/// when this compositor has no blur protocol, and `None` before a window exists to ask about.
pub fn set(material: Material) -> Option<bool> {
    SESSION.with_borrow_mut(|slot| slot.as_mut().map(|session| session.show(material)))
}

thread_local! {
    /// The blur objects of this thread's window. Wayland is single-threaded here: the window,
    /// its connection and every call into this module live on the UI thread.
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

/// The protocol objects this window's blur lives on. Kept for as long as the window is: the
/// surface belongs to winit, and everything else is created here on the same connection.
struct Session {
    // The proxies go before the connection that owns them, so dropping a session destroys its
    // objects on a backend that is still alive.
    active: Option<Active>,
    effect: Option<ExtBackgroundEffectManagerV1>,
    compositor: Option<WlCompositor>,
    blur: Option<OrgKdeKwinBlurManager>,
    surface: WlSurface,
    queue: EventQueue<State>,
    connection: Connection,
}

/// The object that currently asks for blur, in whichever protocol the compositor advertised.
enum Active {
    Effect(ExtBackgroundEffectSurfaceV1),
    Kwin(OrgKdeKwinBlur),
}

/// The client data of the objects made here. None of them send events this app uses: the
/// managers only answer requests, and the capability event arrives before there is anything to
/// do about it.
struct State;

impl Session {
    fn new(window: &WindowHandle) -> Option<Session> {
        let display = window
            .with_display_handle(|handle| match handle.as_raw() {
                RawDisplayHandle::Wayland(display) => Some(display.display),
                _ => None,
            })
            .ok()??;
        let surface = window
            .with_raw_handle(|handle| match handle.as_raw() {
                RawWindowHandle::Wayland(surface) => Some(surface.surface),
                _ => None,
            })
            .ok()??;
        // SAFETY: both pointers belong to the window winit created, and this runs on the UI
        // thread that owns it, inside `on_window`. The backend is a guest on winit's connection:
        // it never reads that connection's queue and never closes it.
        let backend = unsafe { Backend::from_foreign_display(display.as_ptr() as *mut _) };
        let connection = Connection::from_backend(backend);
        let (globals, queue) = registry_queue_init::<State>(&connection).ok()?;
        let qh = queue.handle();
        // SAFETY: the surface is the live one winit created on this same display, and wrapping it
        // shares winit's object data rather than taking it over.
        let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.as_ptr() as *mut _) }
            .ok()?;
        let surface = WlSurface::from_id(&connection, id).ok()?;
        let effect: Option<ExtBackgroundEffectManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
        let compositor: Option<WlCompositor> = globals.bind(&qh, 1..=4, ()).ok();
        // A blur region is a compositor object; no compositor, no effect to describe.
        let effect = effect.filter(|_| compositor.is_some());
        let blur: Option<OrgKdeKwinBlurManager> = globals.bind(&qh, 1..=1, ()).ok();
        if effect.is_none() && blur.is_none() {
            return None;
        }
        Some(Session {
            active: None,
            effect,
            compositor,
            blur,
            surface,
            queue,
            connection,
        })
    }

    /// Blurs the window for a material, or stops for `Off`. True when the window ends up as
    /// asked: blurring, or solid on request.
    fn show(&mut self, material: Material) -> bool {
        let wanted = material != Material::Off;
        if wanted && self.active.is_none() {
            self.start();
        } else if !wanted {
            self.stop();
        }
        let _ = self.connection.flush();
        self.active.is_some() || !wanted
    }

    fn start(&mut self) {
        let qh = self.queue.handle();
        if let (Some(effect), Some(compositor)) = (self.effect.as_ref(), self.compositor.as_ref()) {
            let blur = effect.get_background_effect(&self.surface, &qh, ());
            let region = compositor.create_region(&qh, ());
            region.add(0, 0, COVER, COVER);
            blur.set_blur_region(Some(&region));
            // The request copies the region, so its object has done its work.
            region.destroy();
            self.active = Some(Active::Effect(blur));
        } else if let Some(manager) = self.blur.as_ref() {
            // Without a region, KWin's own protocol blurs the whole surface.
            let blur = manager.create(&self.surface, &qh, ());
            blur.commit();
            self.active = Some(Active::Kwin(blur));
        }
    }

    fn stop(&mut self) {
        match self.active.take() {
            Some(Active::Effect(blur)) => blur.destroy(),
            Some(Active::Kwin(blur)) => {
                if let Some(manager) = self.blur.as_ref() {
                    manager.unset(&self.surface);
                }
                blur.release();
            }
            None => {}
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
        let _ = self.connection.flush();
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlCompositor, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlCompositor,
        _: <WlCompositor as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlRegion, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlRegion,
        _: <WlRegion as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtBackgroundEffectManagerV1,
        _: <ExtBackgroundEffectManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtBackgroundEffectSurfaceV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtBackgroundEffectSurfaceV1,
        _: <ExtBackgroundEffectSurfaceV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<OrgKdeKwinBlurManager, ()> for State {
    fn event(
        _: &mut Self,
        _: &OrgKdeKwinBlurManager,
        _: <OrgKdeKwinBlurManager as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<OrgKdeKwinBlur, ()> for State {
    fn event(
        _: &mut Self,
        _: &OrgKdeKwinBlur,
        _: <OrgKdeKwinBlur as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
