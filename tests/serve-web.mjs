import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { extname, join, normalize } from "node:path";

const root = normalize(fileURLToPath(new URL("../web/", import.meta.url)));
const tmpRoot = process.env.P4G_TMP_SAVE ? normalize(process.env.P4G_TMP_SAVE) : null;
const mime = {
  ".html": "text/html", ".css": "text/css", ".js": "text/javascript",
  ".wasm": "application/wasm", ".ico": "image/x-icon", ".json": "application/json",
};
createServer(async (req, res) => {
  try {
    const url = new URL(req.url, "http://x");
    const base = url.pathname.startsWith("/t/") && tmpRoot ? tmpRoot : root;
    let path = normalize(join(base, decodeURIComponent(url.pathname.replace(/^\/t/, ""))));
    if (!path.startsWith(base)) { res.writeHead(403); res.end(); return; }
    if (url.pathname === "/") path = join(path, "index.html");
    const body = await readFile(path);
    res.writeHead(200, { "content-type": mime[extname(path)] || "application/octet-stream" });
    res.end(body);
  } catch {
    res.writeHead(404); res.end();
  }
}).listen(8741, () => console.log("listening"));
