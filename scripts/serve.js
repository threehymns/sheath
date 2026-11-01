// Serve the built web bundle with Bun. Run after `scripts/web`:
//
//     bun scripts/serve.js            # http://localhost:3000
//     PORT=8080 bun scripts/serve.js
//
// The whole `target/web` folder is served from its root, with the module and wasm MIME types
// the browser requires; a missing file is a 404, never the HTML page.

const root = new URL("../target/web/", import.meta.url);
const port = Number(process.env.PORT ?? 3000);

// The wasm module is most of the download, and gzip takes it from 8.6 MB to 3.7 MB. Compressed
// bodies are kept per file until the next build replaces it, so a request never re-zips 8 MB.
const compressible = new Set([".html", ".js", ".wasm", ".css", ".json", ".svg"]);
const zipped = new Map();

const types = {
    ".html": "text/html; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".wasm": "application/wasm",
    ".css": "text/css; charset=utf-8",
    ".json": "application/json; charset=utf-8",
    ".svg": "image/svg+xml",
    ".png": "image/png",
    ".ico": "image/x-icon",
};

Bun.serve({
    port,
    async fetch(request) {
        const url = new URL(request.url);
        let path = decodeURIComponent(url.pathname);
        if (path.endsWith("/")) path += "index.html";
        const target = new URL("." + path, root);
        if (!target.href.startsWith(root.href)) {
            return new Response("Not found", { status: 404 });
        }
        const file = Bun.file(target);
        if (!(await file.exists())) {
            return new Response("Not found", { status: 404 });
        }
        const ext = path.slice(path.lastIndexOf("."));
        const headers = {
            "content-type": types[ext] ?? "application/octet-stream",
            "cache-control": "no-cache",
        };
        let body = file;
        if (
            compressible.has(ext) &&
            /\bgzip\b/.test(request.headers.get("accept-encoding") ?? "")
        ) {
            const mtime = file.lastModified;
            let cached = zipped.get(target.href);
            if (cached?.mtime !== mtime) {
                cached = { mtime, body: Bun.gzipSync(new Uint8Array(await file.arrayBuffer()), { level: 9 }) };
                zipped.set(target.href, cached);
            }
            headers["content-encoding"] = "gzip";
            headers["content-length"] = String(cached.body.byteLength);
            body = cached.body;
        }
        return new Response(body, { headers });
    },
});

console.log(`Sheath on http://localhost:${port}`);
