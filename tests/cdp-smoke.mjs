// CDP smoke test for the P4G web editor.
// Usage: node tests/cdp-smoke.mjs
// Requires the static server on :8741 serving /t/ from P4G_TMP_SAVE.
import { spawn } from "node:child_process";
import { get } from "node:http";

const CHROME = "C:/Hobby/Chrome for Testing/chrome.exe";
const PORT = 9333;
const PAGE = "http://127.0.0.1:8741/";

const chrome = spawn(CHROME, [
  "--headless=new", "--disable-gpu", "--no-sandbox",
  `--remote-debugging-port=${PORT}`, "about:blank",
], { stdio: "ignore" });
process.on("exit", () => chrome.kill());

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function httpJson(url) {
  return new Promise((resolve, reject) => {
    get(url, (res) => {
      let d = "";
      res.on("data", (c) => d += c);
      res.on("end", () => resolve(JSON.parse(d)));
    }).on("error", reject);
  });
}

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
          if (m.id && pending.has(m.id)) {
            pending.get(m.id)(m);
            pending.delete(m.id);
          }
        };
        await new Promise((r) => ws.onopen = r);
        return;
      }
    } catch {}
    await sleep(250);
  }
  throw new Error("could not connect to chrome");
}

function cdp(method, params = {}) {
  const id = ++msgId;
  ws.send(JSON.stringify({ id, method, params }));
  return new Promise((resolve) => pending.set(id, resolve));
}

async function evaluate(expression) {
  const res = await cdp("Runtime.evaluate", {
    expression, awaitPromise: true, returnByValue: true,
  });
  if (res.result?.exceptionDetails) {
    throw new Error(JSON.stringify(res.result.exceptionDetails).slice(0, 500));
  }
  return res.result?.result?.value;
}

await connect();
await cdp("Page.navigate", { url: PAGE });
await sleep(3000); // wasm init

const results = [];
const check = (name, ok, extra = "") => {
  results.push(`${ok ? "PASS" : "FAIL"} ${name}${extra ? " | " + extra : ""}`);
};

// 1. boot state
check("boot", await evaluate(
  `document.querySelector('#status-msg').textContent.includes('開いてください')`));

// 2. load a save via the hidden input fallback
const loadResult = await evaluate(`(async () => {
  const dt = new DataTransfer();
  for (const name of ["data0001.bin", "data0001.binslot"]) {
    const buf = await (await fetch("/t/" + name)).arrayBuffer();
    dt.items.add(new File([buf], name));
  }
  const input = document.querySelector("#file-input");
  input.files = dt.files;
  input.dispatchEvent(new Event("change", { bubbles: true }));
  await new Promise((r) => setTimeout(r, 1500));
  return document.querySelector('#status-msg').textContent
    + " || " + document.querySelector('#slot-desc').textContent;
})()`);
check("load", /読み込みました: data0001\.bin/.test(loadResult), loadResult);

// 3. basic page fields
const basic = await evaluate(`({
  lname: document.querySelector('[data-key="lname"]')?.value,
  fname: document.querySelector('[data-key="fname"]')?.value,
  hint: document.querySelector('.hint')?.textContent,
  partyBtns: [...document.querySelectorAll('[data-act="party-cycle"]')].length,
})`);
check("basic-fields", !!basic.lname && basic.partyBtns === 3, JSON.stringify(basic));

// 4. mutation: money max -> modified flag
await evaluate(`document.querySelector('[data-act="money-max"]').click()`);
check("modified-flag", await evaluate(
  `document.querySelector('#modified-flag').textContent === '未保存の変更あり'`));
const money = await evaluate(
  `document.querySelector('[data-commit="money"]').value`);
check("money-max", money === "9999999", money);

// 5. date page
await evaluate(`document.querySelector('#nav [data-page="date"]').click()`);
const date = await evaluate(`({
  date: document.querySelector('[data-commit="date"][data-key="cur"]')?.value,
  phase: document.querySelector('[data-act="phase-next"]')?.parentElement?.textContent,
})`);
check("date-page", /\d{4}\/\d{2}\/\d{2}/.test(date.date || ""), JSON.stringify(date));

// 6. party page
await evaluate(`document.querySelector('#nav [data-page="party"]').click()`);
const party = await evaluate(`({
  rows: document.querySelectorAll('[data-sel="unit"]').length,
  equip: document.querySelectorAll('[data-act="equip-pick"]').length,
})`);
check("party-page", party.rows === 8 && party.equip === 4, JSON.stringify(party));

// select unit 1 -> persona editor appears
await evaluate(`document.querySelector('[data-sel="unit"][data-idx="1"]').click()`);
const unit1 = await evaluate(`({
  hp: !!document.querySelector('[data-commit="unitHp"]'),
  skills: document.querySelectorAll('[data-act="skill-pick"]').length,
})`);
check("party-persona", unit1.hp && unit1.skills === 8, JSON.stringify(unit1));

// 7. persona page
await evaluate(`document.querySelector('#nav [data-page="persona"]').click()`);
const persona = await evaluate(`document.querySelectorAll('[data-sel="pslot"]').length`);
check("persona-page", persona === 12, String(persona));

// persona picker opens
await evaluate(`document.querySelector('[data-act="persona-pick"]')?.click()`);
await sleep(300);
const pick = await evaluate(`({
  title: document.querySelector('.picker-modal h3')?.textContent,
  count: document.querySelector('#picker-count')?.textContent,
})`);
check("persona-picker", pick.title === "ペルソナを選択", JSON.stringify(pick));

// pick first persona in list
await evaluate(`document.querySelector('#picker-list .virt-row')?.click()`);
check("persona-assigned", await evaluate(
  `document.querySelector('#picker-overlay') === null`));

// 8. compendium page
await evaluate(`document.querySelector('#nav [data-page="comp"]').click()`);
const comp = await evaluate(`({
  rows: document.querySelectorAll('[data-sel="comp"]').length,
  prog: document.querySelector('.progress > div')?.style.width,
})`);
check("comp-page", comp.rows > 200 && !!comp.prog, JSON.stringify(comp));
await evaluate(`document.querySelector('[data-sel="comp"][data-idx="1"]')?.click()`);
const compSel = await evaluate(
  `!!document.querySelector('[data-act="comp-toggle"]')`);
check("comp-select", compSel);

// 9. social page
await evaluate(`document.querySelector('#nav [data-page="social"]').click()`);
const social = await evaluate(`({
  rows: document.querySelectorAll('#content .row').length,
  rankInputs: document.querySelectorAll('[data-commit="socialRank"]').length,
})`);
check("social-page", social.rows === 23, JSON.stringify(social));

// 10. items page virtualization
await evaluate(`document.querySelector('#nav [data-page="items"]').click()`);
await sleep(400);
const items = await evaluate(`({
  rendered: document.querySelectorAll('#item-list .virt-row').length,
  spacer: document.querySelector('#item-list .virt-rows')?.style.height,
  searchVal: document.querySelector('#item-search')?.value,
})`);
check("items-virtual", items.rendered > 0 && items.rendered < 100,
  JSON.stringify(items));

// item count input commit
const itemCommit = await evaluate(`(() => {
  const inp = document.querySelector('#item-list [data-commit="itemCount"]');
  if (!inp) return "no-input";
  inp.value = "7";
  inp.dispatchEvent(new Event("change", { bubbles: true }));
  return inp.dataset.idx;
})()`);
check("item-commit", itemCommit !== "no-input", String(itemCommit));

// 11. tools page
await evaluate(`document.querySelector('#nav [data-page="tools"]').click()`);
const tools = await evaluate(`({
  checks: [...document.querySelectorAll('.val')].map((e) => e.textContent).join(" "),
  dump: !!document.querySelector('[data-act="dump"]'),
})`);
check("tools-page", /ヘッダ OK/.test(tools.checks) && tools.dump,
  tools.checks.slice(0, 80));

// 12. invalid input -> red border, no crash
await evaluate(`document.querySelector('#nav [data-page="basic"]').click()`);
const invalid = await evaluate(`(() => {
  const inp = document.querySelector('[data-commit="mcLevel"]');
  inp.value = "200";
  inp.dispatchEvent(new Event("change", { bubbles: true }));
  return inp.classList.contains("invalid") + "|" +
    document.querySelector('#status-msg').textContent;
})()`);
check("invalid-input", invalid.startsWith("true"), invalid);

console.log(results.join("\n"));
const fails = results.filter((r) => r.startsWith("FAIL"));
console.log(fails.length ? `\n${fails.length} FAILURES` : "\nALL PASS");
chrome.kill();
process.exit(fails.length ? 1 : 0);
