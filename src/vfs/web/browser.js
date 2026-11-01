// The browser storage behind Sheath's web filesystem: OPFS for the app's own KDL files, and the
// File System Access API for folders the user shares with the tab.

let roots = new Map();
let nextId = 1;

async function opfs() {
    return navigator.storage.getDirectory();
}

export async function loadFiles(names) {
    let root;
    try {
        root = await opfs();
    } catch {
        return names.map(() => null);
    }
    const out = [];
    for (const name of names) {
        try {
            const file = await (await root.getFileHandle(name)).getFile();
            out.push(await file.text());
        } catch {
            out.push(null);
        }
    }
    return out;
}

export function saveFile(name, text) {
    opfs()
        .then(async (root) => {
            const handle = await root.getFileHandle(name, { create: true });
            const writable = await handle.createWritable();
            await writable.write(text);
            await writable.close();
        })
        .catch((error) => console.warn("sheath: could not save", name, error));
}

export function removeFile(name) {
    opfs()
        .then((root) => root.removeEntry(name))
        .catch(() => {});
}

// Text files small enough for the inspector's preview, read while the folder is listed.
const TEXT = /\.(txt|md|markdown|rs|toml|json|kdl|js|mjs|ts|css|html|sh|py|c|h|cpp|yml|yaml|log|csv)$/i;
const PREVIEW_LIMIT = 24 * 1024;

export function pickFolder(cb) {
    if (!window.showDirectoryPicker) {
        cb(JSON.stringify({ error: "This browser has no folder picker; try Chrome or Edge." }));
        return;
    }
    window
        .showDirectoryPicker({ mode: "readwrite" })
        .then((handle) => {
            const id = nextId++;
            roots.set(id, handle);
            cb(JSON.stringify({ id, name: handle.name }));
        })
        .catch((error) => cb(JSON.stringify({ error: String(error) })));
}

export function listFolder(id, path, cb) {
    (async () => {
        let dir = roots.get(id);
        if (!dir) throw new Error("that folder is not open in this tab any more");
        for (const part of path.split("/").filter(Boolean)) dir = await dir.getDirectoryHandle(part);
        const out = [];
        for await (const entry of dir.values()) {
            if (entry.kind === "directory") {
                out.push({ name: entry.name, dir: true, size: 0, modified: 0, text: null });
            } else {
                const file = await entry.getFile();
                const text =
                    file.size <= PREVIEW_LIMIT && TEXT.test(entry.name) ? await file.text() : null;
                out.push({ name: entry.name, dir: false, size: file.size, modified: file.lastModified, text });
            }
        }
        cb(JSON.stringify(out));
    })().catch((error) => cb(JSON.stringify({ error: String(error) })));
}

async function resolveDir(id, path) {
    let dir = roots.get(id);
    if (!dir) throw new Error("that folder is not open in this tab any more");
    for (const part of path.split("/").filter(Boolean)) dir = await dir.getDirectoryHandle(part);
    return dir;
}

async function locate(id, path) {
    const parts = path.split("/").filter(Boolean);
    const name = parts.pop();
    const dir = await resolveDir(id, parts.join("/"));
    return { dir, name };
}

async function handleAt(dir, name) {
    try {
        return await dir.getFileHandle(name);
    } catch {
        return await dir.getDirectoryHandle(name);
    }
}

async function copyHandle(source, destDir, name) {
    if (source.kind === "directory") {
        const dest = await destDir.getDirectoryHandle(name, { create: true });
        for await (const child of source.values()) await copyHandle(child, dest, child.name);
    } else {
        const dest = await destDir.getFileHandle(name, { create: true });
        const writable = await dest.createWritable();
        await writable.write(await (await source.getFile()).arrayBuffer());
        await writable.close();
    }
}

export function writeFolderFile(id, path, contents, cb) {
    (async () => {
        const { dir, name } = await locate(id, path);
        const handle = await dir.getFileHandle(name, { create: true });
        const writable = await handle.createWritable();
        await writable.write(contents);
        await writable.close();
        cb("");
    })().catch((error) => cb(String(error)));
}

export function createFolder(id, path, cb) {
    (async () => {
        const { dir, name } = await locate(id, path);
        await dir.createDirectory(name);
        cb("");
    })().catch((error) => cb(String(error)));
}

export function removeEntry(id, path, recursive, cb) {
    (async () => {
        const { dir, name } = await locate(id, path);
        await dir.removeEntry(name, { recursive });
        cb("");
    })().catch((error) => cb(String(error)));
}

export function moveEntry(id, from, to, cb) {
    (async () => {
        const source = await locate(id, from);
        const handle = await handleAt(source.dir, source.name);
        const target = await locate(id, to);
        const sameDir = from.split("/").slice(0, -1).join("/") === to.split("/").slice(0, -1).join("/");
        if (sameDir && typeof handle.move === "function") {
            await handle.move(target.name);
        } else {
            await copyHandle(handle, target.dir, target.name);
            await source.dir.removeEntry(source.name, { recursive: true });
        }
        cb("");
    })().catch((error) => cb(String(error)));
}

export function copyEntry(id, from, to, cb) {
    (async () => {
        const source = await locate(id, from);
        const handle = await handleAt(source.dir, source.name);
        const target = await locate(id, to);
        await copyHandle(handle, target.dir, target.name);
        cb("");
    })().catch((error) => cb(String(error)));
}

// The browser's own decoders handle WebP, AVIF and whatever else it reads, and `from-image`
// turns the picture upright using its EXIF orientation. SVG is the exception: no browser decodes
// it to a bitmap, but one drawn into an <img> can be painted to a canvas, so that is the fallback.
// `done` releases whichever of the two the caller ends up using.
async function decodePicture(file, max) {
    for (const options of [{ imageOrientation: "from-image" }, undefined]) {
        try {
            const bitmap = await createImageBitmap(file, options);
            return {
                source: bitmap,
                width: bitmap.width,
                height: bitmap.height,
                done: () => bitmap.close?.(),
            };
        } catch {}
    }
    const url = URL.createObjectURL(file);
    try {
        const image = await new Promise((resolve, reject) => {
            const picture = new Image();
            picture.onload = () => resolve(picture);
            picture.onerror = () => reject(new Error("the browser could not decode that picture"));
            picture.src = url;
        });
        // An SVG without an intrinsic size loads at 0×0; draw it at the preview box instead.
        return {
            source: image,
            width: image.naturalWidth || max,
            height: image.naturalHeight || max,
            done: () => URL.revokeObjectURL(url),
        };
    } catch (error) {
        URL.revokeObjectURL(url);
        throw error;
    }
}

// Scales a decoded picture to `max` on its longest side, draws it to a 2D canvas and reads the
// pixels back as RGBA.
function raster(source, width, height, max) {
    const scale = Math.min(1, max / Math.max(width, height));
    const target = [Math.max(1, Math.round(width * scale)), Math.max(1, Math.round(height * scale))];
    const [w, h] = target;
    const canvas =
        typeof OffscreenCanvas === "function"
            ? new OffscreenCanvas(w, h)
            : Object.assign(document.createElement("canvas"), { width: w, height: h });
    const context = canvas.getContext("2d");
    context.drawImage(source, 0, 0, w, h);
    const pixels = context.getImageData(0, 0, w, h).data;
    return { width: w, height: h, data: pixels.buffer };
}

export function previewFile(id, path, max, cb) {
    (async () => {
        const parts = path.split("/").filter(Boolean);
        const name = parts.pop();
        let dir = roots.get(id);
        if (!dir) throw new Error("that folder is not open in this tab any more");
        for (const part of parts) dir = await dir.getDirectoryHandle(part);
        const file = await (await dir.getFileHandle(name)).getFile();
        const image = await decodePicture(file, max);
        let result;
        try {
            result = raster(image.source, image.width, image.height, max);
        } finally {
            image.done();
        }
        cb(result);
    })().catch((error) => cb({ error: String(error) }));
}

// The repositories behind the demo's Projects folders. Each tree is fetched once, when its folder
// is first opened, and a file's bytes only when the app reads one. `raw.githubusercontent.com`
// allows any origin and has no budget worth counting; the tree API allows 60 an hour per address,
// which one tab spends once per repository. The caller names the branch, since not every
// repository's default is `main`.

export function repoTree(repo, branch, cb) {
    (async () => {
        const response = await fetch(
            `https://api.github.com/repos/${repo}/git/trees/${branch}?recursive=1`,
            { headers: { Accept: "application/vnd.github+json" } },
        );
        if (!response.ok) throw new Error(`GitHub answered ${response.status} for the tree`);
        const { tree } = await response.json();
        const rows = tree
            .filter((entry) => entry.type === "blob" || entry.type === "tree")
            .map((entry) => ({
                path: entry.path,
                dir: entry.type === "tree",
                size: entry.size ?? 0,
            }));
        cb(JSON.stringify(rows));
    })().catch((error) => cb(JSON.stringify({ error: String(error) })));
}

export function repoFile(repo, branch, path, cb) {
    (async () => {
        const escaped = path.split("/").map(encodeURIComponent).join("/");
        const response = await fetch(`https://raw.githubusercontent.com/${repo}/${branch}/${escaped}`);
        if (!response.ok) throw new Error(`GitHub answered ${response.status} for ${path}`);
        cb(await response.arrayBuffer());
    })().catch(() => cb(null));
}

export function readFolderFile(id, path, cb) {
    (async () => {
        const parts = path.split("/").filter(Boolean);
        const name = parts.pop();
        let dir = roots.get(id);
        if (!dir) throw new Error("that folder is not open in this tab any more");
        for (const part of parts) dir = await dir.getDirectoryHandle(part);
        const file = await (await dir.getFileHandle(name)).getFile();
        cb(await file.arrayBuffer());
    })().catch((error) => console.warn("sheath: could not read", path, error));
}
