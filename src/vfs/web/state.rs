//! What browser work leaves behind for the next frame.
//!
//! Nothing on the render path waits for the browser: a request marks its path pending and the
//! answer lands here, where the `take_*` functions drain it. The waker draws the frame that shows
//! it.

use super::browser;
use super::grant::mount;
use super::with;
use rune::Waker;
use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

struct Grant {
    root: PathBuf,
    id: u32,
}

#[derive(Default)]
struct WebState {
    grants: Vec<Grant>,
    picked: Vec<PathBuf>,
    error: Option<String>,
    dirty: bool,
    wake: Option<Waker>,
    previews: VecDeque<Decoded>,
    previewing: BTreeSet<PathBuf>,
}

/// A picture from a shared folder, decoded by the browser at preview size.
pub struct Decoded {
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub error: Option<String>,
}

thread_local! {
    static STATE: RefCell<WebState> = RefCell::new(WebState::default());
}

/// The shared folder a path is in, and the path inside it, as the browser names it.
pub(super) fn grant_for(path: &Path) -> Option<(u32, String)> {
    STATE.with(|state| {
        let state = state.borrow();
        state
            .grants
            .iter()
            .filter(|grant| path == grant.root || path.starts_with(&grant.root))
            .max_by_key(|grant| grant.root.as_os_str().len())
            .map(|grant| {
                let rest = path.strip_prefix(&grant.root).unwrap_or(Path::new(""));
                (grant.id, rest.to_string_lossy().into_owned())
            })
    })
}

/// Remembers a folder the browser handed out, so paths inside it route to the browser.
pub(super) fn add_grant(root: PathBuf, id: u32) {
    STATE.with(|state| state.borrow_mut().grants.push(Grant { root, id }));
}

pub(super) fn wake() {
    if let Some(wake) = STATE.with(|state| state.borrow().wake.clone()) {
        wake();
    }
}

pub(super) fn touched() {
    STATE.with(|state| state.borrow_mut().dirty = true);
    wake();
}

/// Shows a browser error on the next frame.
pub(super) fn fail(error: String) {
    STATE.with(|state| state.borrow_mut().error = Some(error));
}

/// A write the browser finished. A failure shows in the app, and every shared listing is
/// dropped so the next frame asks the folder again instead of trusting the change.
pub(super) fn op_done(error: Option<String>) {
    if let Some(error) = error {
        fail(error);
        with(|tree| tree.listed.clear());
    }
    touched();
}

/// The window's waker, so a browser read that lands later draws a frame.
pub fn set_waker(wake: Waker) {
    STATE.with(|state| state.borrow_mut().wake = Some(wake));
}

/// Whether browser work landed since the last frame.
pub fn take_dirty() -> bool {
    STATE.with(|state| std::mem::take(&mut state.borrow_mut().dirty))
}

/// Asks the browser to decode one shared picture at preview size. The pixels land in
/// `take_preview`; a second request for the same file while the first is in flight is ignored.
pub fn request_preview(path: &Path, modified: Option<SystemTime>, max: u32) -> bool {
    let Some((id, rel)) = grant_for(path) else {
        return false;
    };
    if rel.is_empty() {
        return false;
    }
    let path = path.to_path_buf();
    if !STATE.with(|state| state.borrow_mut().previewing.insert(path.clone())) {
        return true;
    }
    browser::preview(id, &rel, max, move |result| {
        let decoded = match result {
            browser::Previewed::Rgba {
                width,
                height,
                pixels,
            } => Decoded {
                path,
                modified,
                width,
                height,
                pixels,
                error: None,
            },
            browser::Previewed::Failed(error) => Decoded {
                path,
                modified,
                width: 0,
                height: 0,
                pixels: Vec::new(),
                error: Some(error),
            },
        };
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.previewing.remove(&decoded.path);
            state.previews.push_back(decoded);
        });
        wake();
    });
    true
}

/// A picture the browser finished decoding, waiting for the next frame to draw it.
pub fn take_preview() -> Option<Decoded> {
    STATE.with(|state| state.borrow_mut().previews.pop_front())
}

/// The folder the user shared, waiting for the next frame to open it.
pub fn take_granted() -> Option<PathBuf> {
    STATE.with(|state| state.borrow_mut().picked.pop())
}

/// Why the last folder request failed, waiting for the next frame to show it.
pub fn take_error() -> Option<String> {
    STATE.with(|state| state.borrow_mut().error.take())
}

/// Asks the browser for a folder; the answer lands in `take_granted`.
pub fn pick_folder() {
    browser::pick(|result| {
        match result {
            Ok(picked) => {
                let root = mount(picked);
                STATE.with(|state| state.borrow_mut().picked.push(root));
            }
            Err(error) => fail(error),
        }
        touched();
    });
}
