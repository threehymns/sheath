//! The browser API behind the web filesystem: OPFS for Sheath's own files, and the File System
//! Access API for folders the user shares with the tab.
//!
//! The page's long-lived state (the granted handles) lives in JavaScript; this module only
//! starts the work and parses what comes back. Everything here must stay off the render path:
//! the tree in `vfs::web` answers from its cache, and a request that hasn't landed yet only
//! marks the folder pending.

use js_sys::{Array, Function, Promise, Reflect, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(module = "/src/vfs/web/browser.js")]
extern "C" {
    #[wasm_bindgen(js_name = loadFiles)]
    fn load_files(names: &Array) -> Promise;
    #[wasm_bindgen(js_name = saveFile)]
    fn save_file(name: &str, text: &str);
    #[wasm_bindgen(js_name = removeFile)]
    fn remove_file(name: &str);
    #[wasm_bindgen(js_name = pickFolder)]
    fn pick_folder(cb: &Function);
    #[wasm_bindgen(js_name = listFolder)]
    fn list_folder(id: u32, path: &str, cb: &Function);
    #[wasm_bindgen(js_name = readFolderFile)]
    fn read_folder_file(id: u32, path: &str, cb: &Function);
    #[wasm_bindgen(js_name = previewFile)]
    fn preview_file(id: u32, path: &str, max: u32, cb: &Function);
    #[wasm_bindgen(js_name = writeFolderFile)]
    fn write_folder_file(id: u32, path: &str, contents: &[u8], cb: &Function);
    #[wasm_bindgen(js_name = createFolder)]
    fn create_folder(id: u32, path: &str, cb: &Function);
    #[wasm_bindgen(js_name = removeEntry)]
    fn js_remove_entry(id: u32, path: &str, recursive: bool, cb: &Function);
    #[wasm_bindgen(js_name = moveEntry)]
    fn js_move_entry(id: u32, from: &str, to: &str, cb: &Function);
    #[wasm_bindgen(js_name = copyEntry)]
    fn js_copy_entry(id: u32, from: &str, to: &str, cb: &Function);
    #[wasm_bindgen(js_name = repoTree)]
    fn repo_tree(repo: &str, branch: &str, cb: &Function);
    #[wasm_bindgen(js_name = repoFile)]
    fn repo_file(repo: &str, branch: &str, path: &str, cb: &Function);
}

/// One row from a granted folder, as the browser reported it.
pub struct EntryInfo {
    pub name: String,
    pub dir: bool,
    pub size: u64,
    pub modified: Option<f64>,
    /// Small text files arrive with the listing, so their preview needs no second trip.
    pub text: Option<String>,
}

pub enum Listing {
    Entries(Vec<EntryInfo>),
    Failed(String),
}

pub struct Picked {
    pub id: u32,
    pub name: String,
}

fn field(value: &JsValue, name: &str) -> Option<JsValue> {
    Reflect::get(value, &JsValue::from_str(name))
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

fn parse(text: &str) -> Option<JsValue> {
    js_sys::JSON::parse(text).ok()
}

fn error_of(value: &JsValue) -> Option<String> {
    field(value, "error").and_then(|error| error.as_string())
}

/// Reads the saved KDL files, in the order asked for; `None` where OPFS has no such file.
pub async fn load(names: &[&str]) -> Vec<Option<String>> {
    let list = Array::new();
    for name in names {
        list.push(&JsValue::from_str(name));
    }
    let Ok(value) = JsFuture::from(load_files(&list)).await else {
        return names.iter().map(|_| None).collect();
    };
    let Ok(values) = value.dyn_into::<Array>() else {
        return names.iter().map(|_| None).collect();
    };
    (0..values.length())
        .map(|at| values.get(at).as_string())
        .collect()
}

pub fn save(name: &str, text: &str) {
    save_file(name, text);
}

pub fn remove(name: &str) {
    remove_file(name);
}

/// A callback the page calls with `""` on success, or the error it hit.
fn completion(cb: impl FnOnce(Option<String>) + 'static) -> Function {
    let callback = Closure::once_into_js(move |value: JsValue| {
        cb(value.as_string().filter(|error| !error.is_empty()));
    });
    callback.unchecked_into()
}

pub fn write(id: u32, path: &str, contents: &[u8], cb: impl FnOnce(Option<String>) + 'static) {
    write_folder_file(id, path, contents, &completion(cb));
}

pub fn create_dir(id: u32, path: &str, cb: impl FnOnce(Option<String>) + 'static) {
    create_folder(id, path, &completion(cb));
}

pub fn remove_entry(
    id: u32,
    path: &str,
    recursive: bool,
    cb: impl FnOnce(Option<String>) + 'static,
) {
    js_remove_entry(id, path, recursive, &completion(cb));
}

pub fn move_entry(id: u32, from: &str, to: &str, cb: impl FnOnce(Option<String>) + 'static) {
    js_move_entry(id, from, to, &completion(cb));
}

pub fn copy_entry(id: u32, from: &str, to: &str, cb: impl FnOnce(Option<String>) + 'static) {
    js_copy_entry(id, from, to, &completion(cb));
}

/// Asks the user for a folder. The callback runs from the page's task queue.
pub fn pick(cb: impl FnOnce(Result<Picked, String>) + 'static) {
    let callback = Closure::once_into_js(move |value: JsValue| {
        let Some(text) = value.as_string() else {
            cb(Err("The folder picker gave no answer.".into()));
            return;
        };
        let Some(parsed) = parse(&text) else {
            cb(Err("The folder picker gave no answer.".into()));
            return;
        };
        if let Some(error) = error_of(&parsed) {
            cb(Err(error));
            return;
        }
        let id = field(&parsed, "id")
            .and_then(|id| id.as_f64())
            .unwrap_or(0.0) as u32;
        let name = field(&parsed, "name")
            .and_then(|name| name.as_string())
            .unwrap_or_default();
        if id == 0 {
            cb(Err("That folder could not be opened.".into()));
            return;
        }
        cb(Ok(Picked { id, name }));
    });
    pick_folder(callback.unchecked_ref());
}

pub fn list(id: u32, path: &str, cb: impl FnOnce(Listing) + 'static) {
    let callback = Closure::once_into_js(move |value: JsValue| {
        let Some(text) = value.as_string() else {
            cb(Listing::Failed("That folder could not be read.".into()));
            return;
        };
        let Some(parsed) = parse(&text) else {
            cb(Listing::Failed("That folder could not be read.".into()));
            return;
        };
        if let Some(error) = error_of(&parsed) {
            cb(Listing::Failed(error));
            return;
        }
        let Ok(rows) = parsed.dyn_into::<Array>() else {
            cb(Listing::Failed("That folder could not be read.".into()));
            return;
        };
        let mut entries = Vec::with_capacity(rows.length() as usize);
        for at in 0..rows.length() {
            let row = rows.get(at);
            let name = field(&row, "name")
                .and_then(|name| name.as_string())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            entries.push(EntryInfo {
                name,
                dir: field(&row, "dir")
                    .and_then(|dir| dir.as_bool())
                    .unwrap_or(false),
                size: field(&row, "size")
                    .and_then(|size| size.as_f64())
                    .unwrap_or(0.0) as u64,
                modified: field(&row, "modified")
                    .and_then(|modified| modified.as_f64())
                    .filter(|millis| *millis > 0.0),
                text: field(&row, "text").and_then(|text| text.as_string()),
            });
        }
        cb(Listing::Entries(entries));
    });
    list_folder(id, path, callback.unchecked_ref());
}

pub enum Previewed {
    Rgba {
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
    Failed(String),
}

/// Decodes one picture from a granted folder in the browser, shrunk to `max` on its longest
/// side and turned upright by its EXIF orientation.
pub fn preview(id: u32, path: &str, max: u32, cb: impl FnOnce(Previewed) + 'static) {
    let callback = Closure::once_into_js(move |value: JsValue| {
        if let Some(error) = field(&value, "error").and_then(|error| error.as_string()) {
            cb(Previewed::Failed(error));
            return;
        }
        let width = field(&value, "width")
            .and_then(|width| width.as_f64())
            .unwrap_or(0.0) as u32;
        let height = field(&value, "height")
            .and_then(|height| height.as_f64())
            .unwrap_or(0.0) as u32;
        let Some(data) =
            field(&value, "data").and_then(|data| data.dyn_into::<js_sys::ArrayBuffer>().ok())
        else {
            cb(Previewed::Failed("the browser gave no pixels".into()));
            return;
        };
        if width == 0 || height == 0 {
            cb(Previewed::Failed("that picture has no size".into()));
            return;
        }
        cb(Previewed::Rgba {
            width,
            height,
            pixels: Uint8Array::new(&data).to_vec(),
        });
    });
    preview_file(id, path, max, callback.unchecked_ref());
}

/// Reads one file from a granted folder. The callback gets nothing when the read failed.
pub fn read(id: u32, path: &str, cb: impl FnOnce(Option<Vec<u8>>) + 'static) {
    let callback = Closure::once_into_js(move |value: JsValue| {
        let bytes = value
            .dyn_into::<js_sys::ArrayBuffer>()
            .ok()
            .map(|buffer| Uint8Array::new(&buffer).to_vec());
        cb(bytes);
    });
    read_folder_file(id, path, callback.unchecked_ref());
}

/// One entry of the repository's tree, as GitHub reported it.
pub struct RepoEntry {
    pub path: String,
    pub dir: bool,
    pub size: u64,
}

/// Asks GitHub for `repo`'s whole tree at `branch`. Paths are relative to its root.
pub fn tree(repo: &str, branch: &str, cb: impl FnOnce(Result<Vec<RepoEntry>, String>) + 'static) {
    let callback = Closure::once_into_js(move |value: JsValue| {
        let Some(text) = value.as_string() else {
            cb(Err("GitHub returned no tree".into()));
            return;
        };
        let Ok(parsed) = js_sys::JSON::parse(&text) else {
            cb(Err("GitHub returned something that is not JSON".into()));
            return;
        };
        if let Some(error) = field(&parsed, "error").and_then(|error| error.as_string()) {
            cb(Err(error));
            return;
        }
        let rows = Array::from(&parsed);
        let mut entries = Vec::with_capacity(rows.length() as usize);
        for row in rows.iter() {
            let Some(path) = field(&row, "path").and_then(|path| path.as_string()) else {
                continue;
            };
            entries.push(RepoEntry {
                path,
                dir: field(&row, "dir").is_some_and(|dir| dir.is_truthy()),
                size: field(&row, "size")
                    .and_then(|size| size.as_f64())
                    .unwrap_or(0.0) as u64,
            });
        }
        cb(Ok(entries));
    });
    repo_tree(repo, branch, callback.unchecked_ref());
}

/// Fetches one file from `repo` at `branch`. The callback gets nothing when the fetch failed.
pub fn file(repo: &str, branch: &str, path: &str, cb: impl FnOnce(Option<Vec<u8>>) + 'static) {
    let callback = Closure::once_into_js(move |value: JsValue| {
        let bytes = value
            .dyn_into::<js_sys::ArrayBuffer>()
            .ok()
            .map(|buffer| Uint8Array::new(&buffer).to_vec());
        cb(bytes);
    });
    repo_file(repo, branch, path, callback.unchecked_ref());
}
