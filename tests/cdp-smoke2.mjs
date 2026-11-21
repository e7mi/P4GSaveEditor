// CDP smoke test part 2: tools page flows and remaining actions.
import { spawn } from "node:child_process";
import { get } from "node:http";

const CHROME = "C:/Hobby/Chrome for Testing/chrome.exe";
const PORT = 9334;
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
await evaluate(`(async () => {
  const dt = new DataTransfer();
  for (const name of ["data0001.bin", "data0001.binslot"]) {
    const buf = await (await fetch("/t/" + name)).arrayBuffer();
    dt.items.add(new File([buf], name));
  }
  const input = document.querySelector("#file-input");
  input.files = dt.files;
  input.dispatchEvent(new Event("change", { bubbles: true }));
  await new Promise((r) => setTimeout(r, 1500));
})()`);

const results = [];
const check = (name, ok, extra = "") =>
  results.push(`${ok ? "PASS" : "FAIL"} ${name}${extra ? " | " + extra : ""}`);
const status = () => evaluate(`document.querySelector('#status-msg').textContent`);

// party cycle
const party0 = await evaluate(`(() => {
  const b = document.querySelectorAll('[data-act="party-cycle"]')[0];
  const before = b.textContent; b.click();
  const after = document.querySelectorAll('[data-act="party-cycle"]')[0].textContent;
  return before + ">" + after;
})()`);
check("party-cycle", party0.includes(">"), party0);

// difficulty cycle
const diff = await evaluate(`(() => {
  const before = document.querySelector('[data-act="diff-cycle"]').parentElement.textContent;
  document.querySelector('[data-act="diff-cycle"]').click();
  const after = document.querySelector('[data-act="diff-cycle"]').parentElement.textContent;
  return before + ">" + after;
})()`);
check("diff-cycle", !diff.startsWith("undefined") && diff.includes(">"), diff);

// date page: phase cycle + sync
await evaluate(`document.querySelector('#nav [data-page="date"]').click()`);
const phase = await evaluate(`(() => {
  const before = document.querySelector('[data-act="phase-next"]').parentElement.textContent;
  document.querySelector('[data-act="phase-next"]').click();
  return before + ">" + document.querySelector('[data-act="phase-next"]').parentElement.textContent;
})()`);
check("phase-cycle", phase.includes(">"), phase);
await evaluate(`document.querySelector('[data-act="sync-next"]').click()`);
check("sync-next", (await status()) !== "エラー");

// date input commit
const dateCommit = await evaluate(`(() => {
  const inp = document.querySelector('[data-commit="date"][data-key="cur"]');
  inp.value = "2011/12/25";
  inp.dispatchEvent(new Event("change", { bubbles: true }));
  return document.querySelector('[data-commit="date"][data-key="cur"]').value;
})()`);
check("date-commit", dateCommit === "2011/12/25", dateCommit);

// detail toggle shows internal values
await evaluate(`document.querySelector('#btn-detail').click()`);
check("detail-internals", await evaluate(
  `!!document.querySelector('[data-commit="daySerial"]')`));
await evaluate(`document.querySelector('#btn-detail').click()`);

// party page: equip-none on unit1
await evaluate(`document.querySelector('#nav [data-page="party"]').click()`);
await evaluate(`document.querySelector('[data-sel="unit"][data-idx="1"]').click()`);
const equip = await evaluate(`(() => {
  const before = document.querySelector('[data-act="equip-none"][data-slot="0"]')
    .parentElement.querySelector('.grow').textContent;
  document.querySelector('[data-act="equip-none"][data-slot="0"]').click();
  const after = document.querySelectorAll('[data-act="equip-none"]')[0]
    .parentElement.querySelector('.grow').textContent;
  return before + ">" + after;
})()`);
check("equip-none", equip.endsWith(">なし"), equip);

// equip picker filtered to weapon
await evaluate(`document.querySelector('[data-act="equip-pick"][data-slot="0"]').click()`);
await sleep(300);
const eqPick = await evaluate(`({
  title: document.querySelector('.picker-modal h3')?.textContent,
  count: document.querySelector('#picker-count')?.textContent,
  catBtn: !!document.querySelector('#picker-cat'),
})`);
check("equip-picker", eqPick.title === "アイテムを選択" && !eqPick.catBtn,
  JSON.stringify(eqPick));
await evaluate(`document.querySelector('#picker-close').click()`);

// skill clear on unit1 persona
await evaluate(`document.querySelector('[data-act="skill-clear"][data-pslot="0"]').click()`);
const skill0 = await evaluate(`(() => {
  const rows = document.querySelectorAll('#content .row');
  for (const r of rows) if (r.textContent.includes("000  なし")) return "cleared";
  return "still-set";
})()`);
check("skill-clear", skill0 === "cleared", skill0);

// item-add picker flow
await evaluate(`document.querySelector('#nav [data-page="items"]').click()`);
await evaluate(`document.querySelector('[data-act="item-add"]').click()`);
await sleep(300);
const addPick = await evaluate(
  `document.querySelector('.picker-modal h3')?.textContent`);
check("item-picker", addPick === "アイテムを選択", String(addPick));
await evaluate(`document.querySelector('#picker-list .virt-row').click()`);
check("item-added", /追加しました|所持しています/.test(await status()), await status());

// tools: verify, backup, restore, clone, dump
await evaluate(`document.querySelector('#nav [data-page="tools"]').click()`);
await evaluate(`document.querySelector('[data-act="verify"]').click()`);
check("verify", /チェックサム: すべて OK/.test(await status()), await status());

await evaluate(`document.querySelector('[data-act="backup-now"]').click()`);
check("backup-row", await evaluate(
  `!!document.querySelector('[data-act="backup-restore"]')`));

await evaluate(`document.querySelector('[data-act="backup-restore"]').click()`);
await sleep(200);
await evaluate(`[...document.querySelectorAll('.modal [data-midx]')].find((b) => b.textContent === '復元')?.click()`);
check("restore", /復元/.test(await status()), await status());

await evaluate(`(() => {
  const inp = document.querySelector('#clone-slot');
  inp.value = "5";
})()`);
await evaluate(`document.querySelector('[data-act="clone"]').click()`);
await sleep(300);
check("clone", /data0005\.bin/.test(await status()), await status());

await evaluate(`document.querySelector('[data-act="dump"]').click()`);
await sleep(300);
check("dump", /書き出しました/.test(await status()), await status());

// compendium toggle mutates count
await evaluate(`document.querySelector('#nav [data-page="comp"]').click()`);
const compToggle = await evaluate(`(() => {
  const before = document.querySelector('#content .progress + div, #content section > div')?.textContent;
  return before;
})()`);
await evaluate(`document.querySelector('[data-sel="comp"][data-idx="1"]').click()`);
await evaluate(`document.querySelector('[data-act="comp-toggle"]').click()`);
const compAfter = await evaluate(
  `document.querySelector('#content section > div')?.textContent`);
check("comp-toggle", compToggle !== compAfter, `${compToggle} -> ${compAfter}`);

// unsaved-changes modal on slot prev/next is N/A (single slot) but open-file modal:
await evaluate(`document.querySelector('#btn-open-file').click()`);
await sleep(300);
const modal = await evaluate(
  `document.querySelector('.modal .msg')?.textContent || ""`);
check("unsaved-modal", modal.includes("保存されていない"), modal);
await evaluate(`[...document.querySelectorAll('.modal [data-midx]')].find((b) => b.textContent === 'キャンセル')?.click()`);

console.log(results.join("\n"));
const fails = results.filter((r) => r.startsWith("FAIL"));
console.log(fails.length ? `\n${fails.length} FAILURES` : "\nALL PASS");
chrome.kill();
process.exit(fails.length ? 1 : 0);
