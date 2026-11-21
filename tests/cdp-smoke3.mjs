// CDP smoke test part 3: multi-slot loading, switching with modified flag,
// social inputs on a real save, save button (download fallback).
import { spawn } from "node:child_process";
import { get } from "node:http";

const CHROME = "C:/Hobby/Chrome for Testing/chrome.exe";
const PORT = 9335;
const PAGE = "http://127.0.0.1:8741/";

const chrome = spawn(CHROME, [
  "--headless=new", "--disable-gpu", "--no-sandbox",
  `--remote-debugging-port=${PORT}`, "about:blank",
], { stdio: "ignore" });
process.on("exit", () => chrome.kill());
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const httpJson = (url) => new Promise((resolve, reject) => {
  get(url, (res) => { let d = ""; res.on("data", (c) => d += c); res.on("end", () => resolve(JSON.parse(d))); }).on("error", reject);
});
let ws, msgId = 0;
const pending = new Map();
async function connect() {
  for (let i = 0; i < 40; i++) {
    try {
      const list = await httpJson(`http://127.0.0.1:${PORT}/json`);
      const page = list.find((t) => t.type === "page");
      if (page) {
        ws = new WebSocket(page.webSocketDebuggerUrl);
        ws.onmessage = (ev) => {
          const m = JSON.parse(ev.data);
          if (m.id && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
        };
        await new Promise((r) => ws.onopen = r);
        return;
      }
    } catch {}
    await sleep(250);
  }
  throw new Error("no chrome");
}
const cdp = (method, params = {}) => {
  const id = ++msgId;
  ws.send(JSON.stringify({ id, method, params }));
  return new Promise((r) => pending.set(id, r));
};
async function evaluate(expression) {
  const res = await cdp("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (res.result?.exceptionDetails) throw new Error(JSON.stringify(res.result.exceptionDetails).slice(0, 600));
  return res.result?.result?.value;
}

await connect();
await cdp("Page.navigate", { url: PAGE });
await sleep(3000);
const results = [];
const check = (name, ok, extra = "") =>
  results.push(`${ok ? "PASS" : "FAIL"} ${name}${extra ? " | " + extra : ""}`);
const status = () => evaluate(`document.querySelector('#status-msg').textContent`);

// load two slots at once
await evaluate(`(async () => {
  const dt = new DataTransfer();
  for (const name of ["data0001.bin", "data0001.binslot", "data0002.bin", "data0002.binslot"]) {
    const buf = await (await fetch("/t/" + name)).arrayBuffer();
    dt.items.add(new File([buf], name));
  }
  const input = document.querySelector("#file-input");
  input.files = dt.files;
  input.dispatchEvent(new Event("change", { bubbles: true }));
  await new Promise((r) => setTimeout(r, 2000));
})()`);
check("multi-load", (await status()).includes("data0002"), await status());
check("slot-label", await evaluate(
  `document.querySelector('#slot-num').textContent === 'スロット 1/2'`));

// switch to slot 2 (no modifications -> direct)
await evaluate(`document.querySelector('#btn-next').click()`);
await sleep(200);
check("slot2", await evaluate(
  `document.querySelector('#slot-desc').textContent.includes('data0002')`));

// social rows populated on this save
await evaluate(`document.querySelector('#nav [data-page="social"]').click()`);
const social = await evaluate(`({
  ranks: document.querySelectorAll('[data-commit="socialRank"]').length,
  maxBtns: document.querySelectorAll('[data-act="social-max"]').length,
})`);
check("social-inputs", social.ranks === 23 && social.maxBtns === 23,
  JSON.stringify(social));
const rankTest = await evaluate(`(() => {
  const inp = document.querySelector('[data-commit="socialRank"][data-idx="0"]');
  inp.value = "10";
  inp.dispatchEvent(new Event("change", { bubbles: true }));
  return document.querySelector('[data-commit="socialRank"][data-idx="0"]').value;
})()`);
check("social-rank-set", rankTest === "10", rankTest);

// switch back to slot1 with unsaved changes -> modal -> discard
await evaluate(`document.querySelector('#btn-prev').click()`);
await sleep(300);
const modal = await evaluate(
  `document.querySelector('.modal .msg')?.textContent || "no-modal"`);
check("switch-modal", modal.includes("保存されていない"), modal);
await evaluate(`[...document.querySelectorAll('.modal [data-midx]')].find((b) => b.textContent === '破棄して移動')?.click()`);
await sleep(200);
check("discarded", await evaluate(
  `document.querySelector('#slot-desc').textContent.includes('data0001') &&
   document.querySelector('#modified-flag').textContent === '保存済み'`));

// save button: download fallback path (no handles in fallback mode)
await cdp("Browser.setDownloadBehavior", { behavior: "allow", downloadPath: "C:/Temp" }).catch(() => {});
await evaluate(`document.querySelector('#nav [data-page="basic"]').click()`);
await evaluate(`document.querySelector('[data-act="money-max"]').click()`);
await evaluate(`document.querySelector('#btn-save').click()`);
await sleep(800);
check("saved", /保存しました/.test(await status()) &&
  (await evaluate(`document.querySelector('#modified-flag').textContent === '保存済み'`)),
  await status());

console.log(results.join("\n"));
const fails = results.filter((r) => r.startsWith("FAIL"));
console.log(fails.length ? `\n${fails.length} FAILURES` : "\nALL PASS");
chrome.kill();
process.exit(fails.length ? 1 : 0);
