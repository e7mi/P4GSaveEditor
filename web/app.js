// P4G Save Editor - frontend for the wasm-bindgen module in ./pkg/.
// All mutations stay in wasm memory until save.to_bytes() is called.

import init, { P4gSave } from "./pkg/p4g_wasm.js";

// ---------- static tables ----------
const CHAR_JP = {
  1: "主人公", 2: "花村陽介", 3: "里中千枝", 4: "天城雪子",
  5: "Rise Kujikawa", 6: "巽完二", 7: "白鐘直斗", 8: "クマ",
};
const SOCIAL_STAT_NAMES = ["勇気", "知識", "忍耐", "寛容", "表現"];
const PERSONA_STAT_NAMES = ["力", "魔", "耐", "速", "運"];
const ARCANA_JP = [
  "", "愚者", "魔術師", "女教皇", "女帝", "皇帝", "法王", "恋愛", "戦車",
  "正義", "隠者", "運命", "剛毅", "刑死者", "死神", "節制", "悪魔", "塔",
  "星", "月", "太陽", "審判", "世界", "永劫", "異邦人",
];
const EQUIP_SLOTS = [
  { label: "武器", cat: "Weapon" },
  { label: "防具", cat: "Armor" },
  { label: "アクセ", cat: "Acc" },
  { label: "衣装", cat: "Clothes" },
];
const CATEGORY_OPTIONS = [
  ["", "すべて"], ["Weapon", "武器"], ["Armor", "防具"], ["Acc", "アクセサリ"],
  ["Clothes", "衣装"], ["Expend", "消耗品"], ["Material", "素材"],
  ["Card", "スキルカード"], ["Book", "書物"], ["Veg", "野菜"], ["Gems", "宝石"],
  ["Fish", "魚"], ["Bugs", "虫"], ["Key", "重要"], ["Quest", "依頼"],
  ["SLink", "ソーシャルリンク"], ["Shelf", "置物"], ["Hollow Forest", "迷宮"],
  ["Bugged", "不具合"], ["BLN", "未使用"], ["FLR", "空き"],
];

const ROW_H = 30; // fixed pixel height of virtualized rows
const OVERSCAN = 8;

// ---------- app state ----------
const app = {
  save: null,          // P4gSave instance for the current slot
  state: null,         // parsed state_json()
  slots: [],           // [{save, state, name, binHandle, slotHandle, lastBytes, modified, backups}]
  selSlot: -1,
  page: "basic",
  detail: false,
  dirHandle: null,     // FileSystemDirectoryHandle when a writable folder was opened
  selUnit: 0,          // party page selected unit index
  selPSlot: 0,         // persona page selected slot index
  selCompId: 0,        // compendium page selected persona id
  compSearch: "",
  compOnlyMissing: false,
  itemSearch: "",
  itemCat: "",
  itemOwnedOnly: false,
  scroll: {},          // scrollTop per page, restored on re-render
};

// Inventory counts come from state_json().inventory as a flat array.

const $ = (sel) => document.querySelector(sel);
const contentEl = $("#content");

function esc(value) {
  return String(value)
    .replaceAll("&", "&amp;").replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function setStatus(msg, isError = false) {
  const el = $("#status-msg");
  el.textContent = msg;
  el.classList.toggle("error", isError);
}

function gameData() {
  return app.state ? app.state.game : null;
}

function personaInfo(id) {
  const g = gameData();
  return g ? g.personas.find((p) => p.id === id) : null;
}

function personaName(id) {
  const info = personaInfo(id);
  return info ? info.name : `0x${pad(id.toString(16).toUpperCase(), 3)}`;
}

function personaDisplay(id) {
  if (!id) return "000  なし";
  return `${pad(id, 3)}  ${personaName(id)}`;
}

function skillName(id) {
  const g = gameData();
  const skill = g.skills.find((s) => s.id === id);
  return skill ? skill.name : `0x${pad(id.toString(16).toUpperCase(), 3)}`;
}

function itemName(id) {
  if (!id) return "なし";
  const g = gameData();
  const item = g.items.find((i) => i.id === id);
  return item ? item.name : `0x${pad(id.toString(16).toUpperCase(), 4)}`;
}

function charName(id) {
  return CHAR_JP[id] || "0x" + pad(id.toString(16).toUpperCase(), 2);
}

function socialName(id) {
  const g = gameData();
  const info = g.social.find((s) => s.id === id);
  return info ? info.name : `ID ${id}`;
}

function categoryLabel(key) {
  const found = CATEGORY_OPTIONS.find(([k]) => k === key);
  return found ? found[1] : key;
}

// ---------- wasm call helpers ----------
function refreshState() {
  if (!app.save) {
    app.state = null;
    return;
  }
  app.state = JSON.parse(app.save.state_json());
}

// Run one wasm mutation; on success mark the slot dirty and re-render.
// Throws propagate: callers that wrap them must setStatus themselves.
function mutate(fn, statusMsg) {
  saveScroll();
  fn(app.save);
  const slot = app.slots[app.selSlot];
  if (slot) slot.modified = true;
  refreshState();
  render();
  if (statusMsg) setStatus(statusMsg);
}

function invCount(id) {
  return app.state?.inventory?.[id] || 0;
}

// ---------- rendering ----------
function render() {
  renderNav();
  renderTopbar();
  renderModified();
  if (!app.save) {
    contentEl.innerHTML =
      '<div class="empty-state">dataXXXX.bin を開いてください</div>';
    return;
  }
  const pageFn = {
    basic: renderBasic,
    date: renderDate,
    party: renderParty,
    persona: renderPersonaPage,
    comp: renderCompendium,
    social: renderSocial,
    items: renderItems,
    tools: renderTools,
  }[app.page] || renderBasic;
  pageFn();
}

function renderNav() {
  for (const btn of document.querySelectorAll("#nav button")) {
    btn.classList.toggle("active", btn.dataset.page === app.page && !!app.save);
  }
}

function renderTopbar() {
  const hasSave = !!app.save;
  $("#btn-save").disabled = !hasSave;
  $("#btn-prev").disabled = app.selSlot <= 0;
  $("#btn-next").disabled = app.selSlot < 0 || app.selSlot >= app.slots.length - 1;
  if (hasSave) {
    const s = app.state.slot;
    $("#slot-desc").textContent =
      `${stem(app.slots[app.selSlot].name)}  ${s.name} Lv${s.level}  ${s.date_text} ${s.phase_text}`;
    $("#slot-num").textContent =
      `スロット ${app.selSlot + 1}/${app.slots.length}`;
  } else {
    $("#slot-desc").textContent = "-";
    $("#slot-num").textContent = "";
  }
  $("#btn-detail").textContent = `詳細: ${app.detail ? "ON" : "OFF"}`;
  $("#btn-detail").classList.toggle("toggled", app.detail);
}

function renderModified() {
  const flag = $("#modified-flag");
  const dirty = !!app.slots[app.selSlot]?.modified;
  flag.textContent = app.save ? (dirty ? "未保存の変更あり" : "保存済み") : "";
  flag.classList.toggle("dirty", dirty);
  flag.classList.toggle("muted", !dirty);
}

function saveScroll() {
  for (const el of contentEl.querySelectorAll("[data-scrollkey]")) {
    app.scroll[el.dataset.scrollkey] = el.scrollTop;
  }
}

function restoreScroll() {
  for (const el of contentEl.querySelectorAll("[data-scrollkey]")) {
    if (app.scroll[el.dataset.scrollkey] != null) {
      el.scrollTop = app.scroll[el.dataset.scrollkey];
    }
  }
}

// ---------- shared UI fragments ----------
function fieldHtml(label, inner) {
  return `<div class="field"><label>${label}</label>${inner}</div>`;
}

function numInput(commit, key, value, width) {
  return `<input type="number" class="${width || ""}" data-commit="${commit}" ` +
    `data-key="${key}" value="${esc(value)}">`;
}

function textInput(commit, key, value, width) {
  return `<input type="text" class="${width || ""}" data-commit="${commit}" ` +
    `data-key="${key}" value="${esc(value)}">`;
}

// Virtualized row list: renders only the visible window inside a scroll box.
// el must have position:relative children space; rows are absolutely placed.
function virtRender(el, rows, rowHtml) {
  const h = ROW_H;
  const total = rows.length;
  const first = Math.max(0, Math.floor(el.scrollTop / h) - OVERSCAN);
  const last = Math.min(
    total, Math.ceil((el.scrollTop + el.clientHeight) / h) + OVERSCAN
  );
  let html = `<div class="virt-rows" style="height:${total * h}px">`;
  for (let i = first; i < last; i++) {
    html += `<div class="virt-row" style="top:${i * h}px;height:${h}px">` +
      rowHtml(rows[i], i) + `</div>`;
  }
  el.innerHTML = html + "</div>";
}

// ---------- page: 基本 ----------
function renderBasic() {
  const b = app.state.base;
  const compTotal = gameData().personas.length;
  const socialInputs = SOCIAL_STAT_NAMES.map((name, i) =>
    fieldHtml(name, numInput("socialStat", i, b.social_stats[i]))
  ).join("");
  const partyBtns = b.party.map((id, i) =>
    `<button data-act="party-cycle" data-idx="${i}">${esc(charName(id))}</button>`
  ).join("");
  contentEl.innerHTML = `
    <p class="hint">コミュ MAX ${b.links_maxed} / 23 ・ 全書 ${b.compendium_count} / ${compTotal}</p>
    <section>
      <h3>プロフィール</h3>
      <div class="field-grid">
        ${fieldHtml("姓", textInput("name", "lname", b.lname))}
        ${fieldHtml("名", textInput("name", "fname", b.fname))}
        ${fieldHtml("所持金", numInput("money", "", b.money, "wide"))}
        ${fieldHtml("レベル", numInput("mcLevel", "", b.level))}
        ${fieldHtml("累計 XP", numInput("mcXp", "", b.xp, "wide"))}
        ${fieldHtml("HP", numInput("mcHp", "", b.hp))}
        ${fieldHtml("SP", numInput("mcSp", "", b.sp))}
      </div>
      <div class="actions">
        <button data-act="money-max">所持金 MAX</button>
        <button data-act="level-99">Lv99</button>
        <button data-act="hp-sp-max">HP/SP 9999</button>
      </div>
    </section>
    <section>
      <h3>プレイ時間と難易度</h3>
      <div class="field-grid">
        ${fieldHtml("プレイ時間",
          textInput("playtime", "", `${b.playtime[0]}:${pad(b.playtime[1], 2)}:${pad(b.playtime[2], 2)}`, "wide"))}
        ${fieldHtml("難易度",
          `<span class="val">${esc(b.difficulty_jp)}</span><button data-act="diff-cycle">変更</button>`)}
      </div>
    </section>
    <section>
      <h3>社交ステータス</h3>
      <div class="field-grid">${socialInputs}</div>
      <div class="actions">
        <button data-act="social-stats-max">すべて MAX</button>
      </div>
    </section>
    <section>
      <h3>現在のパーティ</h3>
      <div class="btn-row">${partyBtns}</div>
    </section>
    <section>
      <h3>実績カウンタ</h3>
      <div class="field-grid">
        ${fieldHtml("リセのナビ回数", numInput("riseLines", "", b.rise_lines))}
      </div>
    </section>`;
}

// ---------- page: 日付 ----------
function renderDate() {
  const b = app.state.base;
  const nextDate = serialToDate(b.next_day);
  const detailBlock = app.detail ? `
    <section>
      <h3>内部値</h3>
      <div class="field-grid">
        ${fieldHtml("day_serial", numInput("daySerial", "", b.day_serial))}
        ${fieldHtml("next_day", numInput("nextDay", "", b.next_day))}
      </div>
    </section>` : "";
  contentEl.innerHTML = `
    <section>
      <h3>日付と時間帯</h3>
      <div class="field-grid">
        ${fieldHtml("現在のゲーム内日付",
          textInput("date", "cur", b.game_date, "wide"))}
        ${fieldHtml("時間帯",
          `<span class="val">${esc(b.phase_jp)}</span><button data-act="phase-next">次へ</button>`)}
        ${fieldHtml("次の日付",
          textInput("date", "next", nextDate.text, "wide") +
          `<button data-act="sync-next">現在に合わせる</button>`)}
        ${fieldHtml("次の時間帯",
          `<span class="val">${esc(b.next_phase_jp)}</span><button data-act="next-phase-next">次へ</button>`)}
      </div>
    </section>
    ${detailBlock}
    <p class="hint">セーブ前のバックアップは「ツール」ページから復元できます。</p>`;
}

// ---------- persona editor fragment ----------
// kind: "mc" | "party" | "comp". For "comp", index is compendium index (id-1).
function personaEditorHtml(kind, index, p) {
  if (kind === "comp") {
    const info = personaInfo(index + 1);
    const regBtn = p.exists
      ? `<button data-act="comp-toggle" data-idx="${index}">登録済み (解除)</button>`
      : `<button data-act="comp-toggle" data-idx="${index}">未登録 (登録)</button>`;
    let html = `<section><h3>ペルソナ全書</h3>
      <div class="field"><span class="val">${pad(index + 1, 3)}  ${esc(info ? info.name : "?")}</span>${regBtn}</div>`;
    if (!p.exists) {
      return html + `<p class="hint">未登録です。「登録」ボタンで登録できます。</p></section>`;
    }
    html += `<p class="hint">${esc(arcanaHint(info))}</p>`;
    return html + personaFieldsHtml(kind, index, p) + `</section>`;
  }

  const target = kind === "mc" ? "mc" : "party";
  let html = `<section><h3>ペルソナ</h3>
    <div class="field">
      <span class="val">${esc(personaDisplay(p.exists ? p.id : 0))}</span>
      <button data-act="persona-pick" data-target="${target}" data-idx="${index}">ペルソナを選択</button>
    </div>`;
  if (!p.exists) {
    return html +
      `<p class="hint">空き枠です。「ペルソナを選択」から追加してください。</p></section>`;
  }
  const info = personaInfo(p.id);
  html += `<p class="hint">${esc(arcanaHint(info))}</p>`;
  return html + personaFieldsHtml(kind, index, p) + `</section>`;
}

function arcanaHint(info) {
  if (!info) return "";
  const arcana = ARCANA_JP[info.arcana] || "";
  return arcana ? `${arcana} / 初期 Lv${info.level}` : `初期 Lv${info.level}`;
}

// Shared editable fields for a registered persona (level/xp/stats/skills).
// kind is one of "mc" | "party" | "comp"; wasm targets are mc/party/compendium.
function personaFieldsHtml(kind, index, p) {
  const target = { mc: "mc", party: "party", comp: "compendium" }[kind];
  const stats = PERSONA_STAT_NAMES.map((name, i) =>
    fieldHtml(name,
      `<input type="number" data-commit="pStat" data-target="${target}" ` +
      `data-idx="${index}" data-stat="${i}" value="${p.stats[i]}">`)
  ).join("");
  const skills = p.skills.map((sid, i) => `
    <div class="row">
      <span class="badge">${pad(i + 1, 2)}</span>
      <span class="grow">${sid ? esc(`${pad(sid, 3)}  ${skillName(sid)}`) : `${pad(0, 3)}  なし`}</span>
      <button data-act="skill-pick" data-target="${target}" data-idx="${index}" data-pslot="${i}">選択</button>
      <button data-act="skill-clear" data-target="${target}" data-idx="${index}" data-pslot="${i}">クリア</button>
    </div>`).join("");
  return `
    <div class="field-grid">
      ${fieldHtml("レベル",
        `<input type="number" data-commit="pLevel" data-target="${target}" data-idx="${index}" value="${p.level}">`)}
      ${fieldHtml("XP",
        `<input type="number" class="wide" data-commit="pXp" data-target="${target}" data-idx="${index}" value="${p.xp}">`)}
    </div>
    <h3 style="margin-top:14px">ステータス</h3>
    <div class="field-grid">${stats}</div>
    <h3 style="margin-top:14px">スキル</h3>
    <div>${skills}</div>`;
}

// ---------- page: 仲間 ----------
function renderParty() {
  const units = app.state.party_units;
  const sel = Math.min(app.selUnit, units.length - 1);
  app.selUnit = sel;
  const list = units.map((u, i) => `
    <div class="row clickable${i === sel ? " selected" : ""}" data-sel="unit" data-idx="${i}">
      <span class="grow">${esc(charName(u.id))}</span>
      <span class="muted">HP ${u.hp} / SP ${u.sp}</span>
    </div>`).join("");

  const u = units[sel];
  let detail = "";
  if (sel === 0) {
    detail += `<p class="hint">HP/SP は「基本」ページで編集します。</p>
      <div class="actions"><button data-act="goto-persona">ペルソナタブへ</button></div>`;
  } else {
    detail += `
      <div class="field-grid">
        ${fieldHtml("HP", `<input type="number" data-commit="unitHp" data-idx="${sel}" value="${u.hp}">`)}
        ${fieldHtml("SP", `<input type="number" data-commit="unitSp" data-idx="${sel}" value="${u.sp}">`)}
      </div>`;
  }
  detail += `<div class="actions">
      <button data-act="clear-status">状態異常を解除</button>
      <button data-act="heal-all">全員回復</button>
    </div>`;
  detail += `<section><h3>装備</h3>` + u.equipment.map((itemId, slot) => {
    const meta = EQUIP_SLOTS[slot];
    return `<div class="row">
      <span class="badge">${meta.label}</span>
      <span class="grow">${esc(itemName(itemId))}</span>
      <button data-act="equip-pick" data-unit="${sel}" data-slot="${slot}">変更</button>
      <button data-act="equip-none" data-unit="${sel}" data-slot="${slot}">なし</button>
    </div>`;
  }).join("") + `</section>`;
  if (sel > 0) {
    detail += personaEditorHtml("party", sel, u.persona);
  }

  contentEl.innerHTML = `
    <div class="split">
      <div class="list-col"><div class="listbox" data-scrollkey="party-list">${list}</div></div>
      <div class="detail-col"><section><h3>${esc(charName(u.id))}</h3>${detail}</section></div>
    </div>`;
  restoreScroll();
}

// ---------- page: ペルソナ (主人公所持 12 枠) ----------
function renderPersonaPage() {
  const personas = app.state.mc_personas;
  const sel = Math.min(app.selPSlot, personas.length - 1);
  app.selPSlot = sel;
  const list = personas.map((p, i) => {
    const label = p.exists
      ? `${pad(i + 1, 2)}  ${esc(personaName(p.id))} Lv${p.level}`
      : `${pad(i + 1, 2)}  空き`;
    return `<div class="row clickable${i === sel ? " selected" : ""}" ` +
      `data-sel="pslot" data-idx="${i}"><span class="grow">${label}</span></div>`;
  }).join("");
  const p = personas[sel];
  let actions = `<div class="actions">`;
  if (p.exists) {
    actions += `<button data-act="persona-clear" data-idx="${sel}">この枠を空にする</button>`;
  }
  actions += `<button data-act="persona-max-all">全枠 Lv99</button></div>`;
  contentEl.innerHTML = `
    <div class="split">
      <div class="list-col"><div class="listbox" data-scrollkey="persona-list">${list}</div></div>
      <div class="detail-col">
        ${personaEditorHtml("mc", sel, p)}
        ${actions}
      </div>
    </div>`;
  restoreScroll();
}

// ---------- page: 全書 ----------
function renderCompendium() {
  const g = gameData();
  const comp = app.state.compendium;
  const registered = app.state.base.compendium_count;
  const total = g.personas.length;
  const q = app.compSearch.trim().toLowerCase();

  const entries = g.personas.filter((info) => {
    if (!app.detail && info.name === "未使用") return false;
    const entry = comp[info.id - 1];
    if (app.compOnlyMissing && entry && entry.exists) return false;
    if (q) {
      const hay = `${info.name} ${info.name_en} ${pad(info.id, 3)}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  });

  const rows = entries.map((info) => {
    const entry = comp[info.id - 1];
    const right = entry && entry.exists ? `Lv${entry.level}` : "未登録";
    const selCls = info.id === app.selCompId ? " selected" : "";
    const mute = entry && entry.exists ? "" : " mutedrow";
    return `<div class="row clickable${selCls}${mute}" data-sel="comp" data-idx="${info.id}">
      <span class="grow">${pad(info.id, 3)}  ${esc(info.name)}</span>
      <span class="muted">${right}</span>
    </div>`;
  }).join("");

  let detail = "";
  if (app.selCompId > 0 && app.selCompId <= comp.length) {
    detail = personaEditorHtml("comp", app.selCompId - 1, comp[app.selCompId - 1]);
  } else {
    detail = `<section><p class="hint">左の一覧からペルソナを選んでください。</p></section>`;
  }

  contentEl.innerHTML = `
    <section>
      <h3>ペルソナ全書</h3>
      <div>${registered} / ${total}</div>
      <div class="progress"><div style="width:${total ? (registered / total) * 100 : 0}%"></div></div>
      <div class="actions">
        <button data-act="comp-fill">未登録をすべて登録</button>
        <button data-act="comp-reset">全初期化</button>
        <button data-act="comp-clear" class="danger">全解除</button>
      </div>
      <div class="field-grid" style="margin-top:10px">
        ${fieldHtml("検索", `<input type="search" id="comp-search" value="${esc(app.compSearch)}">`)}
        ${fieldHtml("", `<button data-act="comp-only-missing" class="${app.compOnlyMissing ? "toggled" : ""}">未登録のみ</button>`)}
      </div>
    </section>
    <div class="split">
      <div class="list-col"><div class="listbox" data-scrollkey="comp-list">${rows}</div></div>
      <div class="detail-col">${detail}</div>
    </div>`;
  const search = $("#comp-search");
  search.addEventListener("input", () => {
    app.compSearch = search.value;
    rerenderKeepFocus("comp-search");
  });
  restoreScroll();
}

// ---------- page: コミュ ----------
function renderSocial() {
  const rows = app.state.social_links.map((link, i) => {
    if (link.id === 0) {
      return `<div class="row mutedrow">
        <span class="badge">${pad(i, 2)}</span><span class="grow">未使用</span>
      </div>`;
    }
    return `<div class="row">
      <span class="badge">${pad(i, 2)}</span>
      <span class="grow">${esc(socialName(link.id))}</span>
      <label>ランク</label>
      <input type="number" data-commit="socialRank" data-idx="${i}" value="${link.rank}">
      <label>好感度</label>
      <input type="number" data-commit="socialAff" data-idx="${i}" value="${link.affinity}">
      <button data-act="social-max" data-idx="${i}">MAX</button>
    </div>`;
  }).join("");
  contentEl.innerHTML = `
    <div class="listbox" data-scrollkey="social-list">${rows}</div>
    <div class="actions"><button data-act="social-max-all">すべて MAX</button></div>`;
  restoreScroll();
}

// ---------- page: アイテム ----------
function filteredItems() {
  const g = gameData();
  const q = app.itemSearch.trim().toLowerCase();
  return g.items.filter((item) => {
    if (item.id === 0 || item.category === "FLR") return false;
    const count = invCount(item.id);
    if (!app.detail && item.name === "未使用" && count === 0) return false;
    if (app.itemOwnedOnly && count === 0) return false;
    if (app.itemCat && item.category !== app.itemCat) return false;
    if (q) {
      const hay = `${item.name} ${item.name_en} ${pad(item.id, 4)}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  });
}

function renderItems() {
  const items = filteredItems();
  contentEl.innerHTML = `
    <section>
      <div class="field-grid">
        ${fieldHtml("検索", `<input type="search" id="item-search" value="${esc(app.itemSearch)}">`)}
        ${fieldHtml("カテゴリ",
          `<button data-act="item-cat-cycle">${esc(categoryLabel(app.itemCat))}</button>`)}
        ${fieldHtml("", `<button data-act="item-owned" class="${app.itemOwnedOnly ? "toggled" : ""}">所持のみ</button>`)}
        ${fieldHtml("", `<button data-act="item-add">アイテムを追加</button>`)}
      </div>
      <div class="actions">
        <button data-act="items-all-99">全アイテム 99</button>
        <button data-act="items-clear" class="danger">全削除</button>
        <span class="muted" style="align-self:center">${items.length} 件</span>
      </div>
    </section>
    <div class="listbox" id="item-list" data-scrollkey="item-list" style="height:480px"></div>`;
  const list = $("#item-list");
  const renderWindow = () => virtRender(list, items, (item) => `
    <span class="grow">${pad(item.id, 4)}  ${esc(item.name)}  [${esc(item.category_jp)}]</span>
    <input type="number" data-commit="itemCount" data-idx="${item.id}" value="${invCount(item.id)}">
  `);
  list.addEventListener("scroll", renderWindow);
  const search = $("#item-search");
  search.addEventListener("input", () => {
    app.itemSearch = search.value;
    app.scroll["item-list"] = 0;
    rerenderKeepFocus("item-search");
  });
  restoreScroll();
  renderWindow(); // after scroll restore so the visible window matches scrollTop
}

// ---------- page: ツール ----------
function renderTools() {
  const slot = app.slots[app.selSlot];
  const checks = slot ? slot.checks : { header: true, save_md5: true, slot_md5: true };
  const checkText =
    `ヘッダ ${checks.header ? "OK" : "NG"} / ` +
    `セーブ MD5 ${checks.save_md5 ? "OK" : "NG"} / ` +
    `スロット MD5 ${checks.slot_md5 ? "OK" : "NG"}`;
  const allOk = checks.header && checks.save_md5 && checks.slot_md5;
  const backups = (slot ? slot.backups : []).map((b, i) => `
    <div class="row">
      <span class="grow">${esc(b.label)}</span>
      <button data-act="backup-restore" data-idx="${i}">復元</button>
    </div>`).join("");
  contentEl.innerHTML = `
    <section>
      <h3>ファイル</h3>
      <div class="field"><label>ファイル名</label><span class="val">${esc(slot ? slot.name : "-")}</span></div>
      <div class="field"><label>チェック</label>
        <span class="val ${allOk ? "checks-ok" : "checks-ng"}">${checkText}</span></div>
      <div class="actions"><button data-act="verify">状態を再確認</button></div>
    </section>
    <section>
      <h3>バックアップ</h3>
      ${backups || '<p class="hint">まだバックアップがありません。</p>'}
      <div class="actions"><button data-act="backup-now">今すぐバックアップ</button></div>
    </section>
    <section>
      <h3>別スロットに複製</h3>
      <div class="field">
        <label>スロット番号</label>
        <input type="number" id="clone-slot" value="2" min="1" max="16">
        <button data-act="clone">複製</button>
      </div>
    </section>
    <section>
      <h3>書き出し</h3>
      <div class="actions"><button data-act="dump">JSON に書き出す</button></div>
    </section>`;
}

// Re-render while keeping focus on an input identified by id.
function rerenderKeepFocus(focusId) {
  saveScroll();
  render();
  const el = document.getElementById(focusId);
  if (el) el.focus();
}

// ---------- modal ----------
function showModal({ message, buttons }) {
  return new Promise((resolve) => {
    const root = $("#modal-root");
    root.innerHTML = `
      <div class="overlay">
        <div class="modal">
          <div class="msg">${esc(message)}</div>
          <div class="buttons">${buttons.map((b, i) =>
            `<button data-midx="${i}" class="${b.primary ? "primary" : ""}">${esc(b.label)}</button>`
          ).join("")}</div>
        </div>
      </div>`;
    root.querySelectorAll("[data-midx]").forEach((btn) => {
      btn.addEventListener("click", () => {
        root.innerHTML = "";
        resolve(buttons[+btn.dataset.midx].value);
      });
    });
  });
}

// Warn about unsaved changes before a slot/file switch.
// Returns true when the caller may proceed.
async function ensureClean() {
  // Jump to the first dirty slot; other slots may hold unsaved edits too.
  const dirtyIndex = app.slots.findIndex((s) => s.modified);
  if (dirtyIndex < 0) return true;
  if (dirtyIndex !== app.selSlot) {
    app.selSlot = dirtyIndex;
    app.save = app.slots[dirtyIndex].save;
    refreshState();
    render();
  }
  const slot = app.slots[app.selSlot];
  if (!slot || !slot.modified) return true;
  const choice = await showModal({
    message: "保存されていない変更があります。",
    buttons: [
      { label: "保存して移動", value: "save", primary: true },
      { label: "破棄して移動", value: "discard" },
      { label: "キャンセル", value: "cancel" },
    ],
  });
  if (choice === "cancel") return false;
  if (choice === "save") {
    await saveCurrent();
    if (slot.modified) return false; // save failed
  }
  return true;
}

async function confirmDestructive(message) {
  const choice = await showModal({
    message,
    buttons: [
      { label: "実行", value: "run", primary: true },
      { label: "キャンセル", value: "cancel" },
    ],
  });
  return choice === "run";
}

// ---------- file loading ----------
async function readBytes(fileOrHandle) {
  const file = fileOrHandle instanceof File ? fileOrHandle : await fileOrHandle.getFile();
  return new Uint8Array(await file.arrayBuffer());
}

// recs: [{name, file: File|handle, slotFile: File|handle|null}]
async function loadSlotRecs(recs) {
  const entries = [];
  const errors = [];
  for (const rec of recs) {
    try {
      const bin = await readBytes(rec.file);
      const slotBytes = rec.slotFile ? await readBytes(rec.slotFile) : null;
      const save = new P4gSave(bin, slotBytes);
      save.set_file_name(rec.name);
      const [obin, oslot] = save.original_bytes();
      entries.push({
        save,
        name: rec.name,
        binHandle: rec.file instanceof File ? null : rec.file,
        slotHandle: rec.slotFile && !(rec.slotFile instanceof File) ? rec.slotFile : null,
        lastBytes: { bin: new Uint8Array(obin), slot: new Uint8Array(oslot) },
        modified: false,
        backups: [],
        checks: JSON.parse(save.checks_json()),
      });
    } catch (e) {
      errors.push(`${rec.name}: ${e.message || e}`);
    }
  }
  return { entries, errors };
}

async function adoptSlots(entries, sourceLabel) {
  if (!entries.length) return;
  app.slots = entries;
  app.selSlot = 0;
  app.save = entries[0].save;
  app.selUnit = 0;
  app.selPSlot = 0;
  app.selCompId = 0;
  refreshState();
  render();
  const names = entries.map((e) => e.name).join(", ");
  setStatus(`読み込みました: ${names}${sourceLabel || ""}`);
}

async function openFolder() {
  if (window.showDirectoryPicker) {
    let dir;
    try {
      dir = await showDirectoryPicker({ mode: "readwrite" });
    } catch (e) {
      if (e.name === "AbortError") return;
      // Read-write denied; retry read-only so the folder can still be browsed.
      try {
        dir = await showDirectoryPicker();
      } catch (e2) {
        if (e2.name !== "AbortError") setStatus(`エラー: ${e2.message || e2}`, true);
        return;
      }
    }
    const map = new Map();
    for await (const entry of dir.values()) {
      if (entry.kind === "file") map.set(entry.name, entry);
    }
    const recs = pairRecs(map);
    if (!recs.length) {
      setStatus("フォルダ内に dataXXXX.bin が見つかりません", true);
      return;
    }
    app.dirHandle = dir;
    const { entries, errors } = await loadSlotRecs(recs);
    await adoptSlots(entries);
    if (errors.length) setStatus(`エラー: ${errors.join("; ")}`, true);
  } else {
    $("#file-dir-input").click();
  }
}

async function openFiles() {
  if (window.showOpenFilePicker) {
    let handles;
    try {
      handles = await showOpenFilePicker({
        multiple: true,
        types: [{ description: "P4G セーブ", accept: { "application/octet-stream": [".bin", ".binslot"] } }],
      });
    } catch (e) {
      if (e.name !== "AbortError") setStatus(`エラー: ${e.message || e}`, true);
      return;
    }
    const map = new Map();
    for (const h of handles) map.set(h.name, h);
    const recs = pairRecs(map);
    if (!recs.length) {
      setStatus("dataXXXX.bin を選んでください", true);
      return;
    }
    app.dirHandle = null;
    const { entries, errors } = await loadSlotRecs(recs);
    await adoptSlots(entries);
    if (errors.length) setStatus(`エラー: ${errors.join("; ")}`, true);
  } else {
    $("#file-input").click();
  }
}

// ---------- save / download ----------
async function writeOrDownload(dir, name, bytes, fallbackName) {
  if (dir) {
    try {
      const fh = await dir.getFileHandle(name, { create: true });
      const w = await fh.createWritable();
      await w.write(bytes);
      await w.close();
      return true;
    } catch {
      // fall through to download
    }
  }
  download(fallbackName || name, bytes);
  return false;
}

async function writeHandle(handle, bytes) {
  const w = await handle.createWritable();
  await w.write(bytes);
  await w.close();
}

function download(name, bytes) {
  const blob = new Blob([bytes], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10000);
}

async function saveCurrent() {
  const slot = app.slots[app.selSlot];
  if (!slot) return;
  let out;
  try {
    out = app.save.to_bytes();
  } catch (e) {
    setStatus(`エラー: ${e.message || e}`, true);
    return;
  }
  const bin = new Uint8Array(out[0]);
  const slotBytes = new Uint8Array(out[1]);
  try {
    if (slot.binHandle) {
      await writeHandle(slot.binHandle, bin);
      if (slot.slotHandle) await writeHandle(slot.slotHandle, slotBytes);
      else if (app.dirHandle) {
        const fh = await app.dirHandle.getFileHandle(`${stem(slot.name)}.binslot`, { create: true });
        await writeHandle(fh, slotBytes);
        slot.slotHandle = fh;
      } else {
        // .bin opened alone: writing only it would leave a stale .binslot
        // whose md5 no longer matches, so hand the pair to the user.
        download(`${stem(slot.name)}.binslot`, slotBytes);
      }
    } else {
      download(slot.name, bin);
      download(`${stem(slot.name)}.binslot`, slotBytes);
    }
  } catch (e) {
    setStatus(`保存に失敗しました: ${e.message || e}`, true);
    return;
  }
  slot.backups.push({
    label: `${new Date().toLocaleTimeString("ja-JP")} (保存前)`,
    bin: slot.lastBytes.bin,
    slot: slot.lastBytes.slot,
  });
  slot.lastBytes = { bin, slot: slotBytes };
  slot.modified = false;
  refreshState();
  render();
  setStatus(`保存しました: ${slot.name}`);
}

// ---------- picker ----------
const picker = { items: [], cat: "", lockCat: false };

function openPicker(type, title, entries, opts = {}) {
  picker.items = entries;
  picker.cat = opts.lockCat || "";
  picker.lockCat = !!opts.lockCat;
  const root = $("#modal-root");
  root.innerHTML = `
    <div class="overlay" id="picker-overlay">
      <div class="modal picker-modal">
        <h3>${esc(title)}</h3>
        <div class="picker-controls">
          <input type="search" id="picker-search" placeholder="名前または ID で検索">
          ${type === "item" && !picker.lockCat
            ? `<button id="picker-cat">${esc(categoryLabel(picker.cat))}</button>` : ""}
          <span class="picker-count" id="picker-count"></span>
        </div>
        <div class="picker-list" id="picker-list"></div>
        <div class="picker-footer"><button id="picker-close">閉じる</button></div>
      </div>
    </div>`;
  const list = $("#picker-list");
  const search = $("#picker-search");
  const renderList = () => {
    const q = search.value.trim().toLowerCase();
    const filtered = picker.items.filter((it) => {
      if (type === "item" && picker.cat && it.category !== picker.cat) return false;
      if (q && !it.hay.includes(q)) return false;
      return true;
    });
    $("#picker-count").textContent = `${filtered.length} 件`;
    virtRender(list, filtered, (it) => it.row);
    list.querySelectorAll(".virt-row").forEach((row, i) => row.classList.add("clickable"));
  };
  list.addEventListener("scroll", renderList);
  list.addEventListener("click", (e) => {
    const row = e.target.closest(".virt-row");
    if (!row) return;
    const idx = [...list.querySelectorAll(".virt-row")].indexOf(row);
    const q = search.value.trim().toLowerCase();
    const filtered = picker.items.filter((it) => {
      if (type === "item" && picker.cat && it.category !== picker.cat) return false;
      if (q && !it.hay.includes(q)) return false;
      return true;
    });
    // map visible index back to filtered list
    const first = Math.max(0, Math.floor(list.scrollTop / ROW_H) - OVERSCAN);
    const item = filtered[first + idx];
    closePicker();
    if (item) item.onPick();
  });
  search.addEventListener("input", () => {
    list.scrollTop = 0;
    renderList();
  });
  const catBtn = $("#picker-cat");
  if (catBtn) {
    catBtn.addEventListener("click", () => {
      const i = CATEGORY_OPTIONS.findIndex(([k]) => k === picker.cat);
      picker.cat = CATEGORY_OPTIONS[(i + 1) % CATEGORY_OPTIONS.length][0];
      catBtn.textContent = categoryLabel(picker.cat);
      list.scrollTop = 0;
      renderList();
    });
  }
  $("#picker-close").addEventListener("click", closePicker);
  $("#picker-overlay").addEventListener("click", (e) => {
    if (e.target.id === "picker-overlay") closePicker();
  });
  renderList();
  search.focus();
}

function closePicker() {
  $("#modal-root").innerHTML = "";
}

function openPersonaPicker(target, index) {
  const g = gameData();
  const entries = g.personas
    .filter((p) => p.id > 0 && (app.detail || p.name !== "未使用"))
    .map((p) => ({
      hay: `${p.name} ${p.name_en} ${pad(p.id, 3)}`.toLowerCase(),
      row: `<span class="grow" data-pid="${p.id}">${pad(p.id, 3)}  ${esc(p.name)}  Lv${p.level}</span>`,
      onPick: () => {
        try {
          mutate((s) => s.persona_assign(target, index, p.id));
        } catch (e) {
          setStatus(`エラー: ${e.message || e}`, true);
        }
      },
    }));
  openPicker("persona", "ペルソナを選択", entries);
}

function openSkillPicker(target, index, pslot) {
  const g = gameData();
  const entries = g.skills
    .filter((s) => s.has_jp)
    .map((s) => ({
      hay: `${s.name} ${s.name_en} ${pad(s.id, 3)}`.toLowerCase(),
      row: `<span class="grow" data-sid="${s.id}">${pad(s.id, 3)}  ${esc(s.name)}</span>`,
      onPick: () => {
        try {
          mutate((s2) => s2.persona_set_skill(target, index, pslot, s.id));
        } catch (e) {
          setStatus(`エラー: ${e.message || e}`, true);
        }
      },
    }));
  openPicker("skill", "スキルを選択", entries);
}

function openItemPicker(lockCat, onPick) {
  const g = gameData();
  const entries = g.items
    .filter((i) => i.id !== 0 && i.category !== "FLR")
    .filter((i) => app.detail || i.name !== "未使用")
    .map((i) => ({
      category: i.category,
      hay: `${i.name} ${i.name_en} ${pad(i.id, 4)}`.toLowerCase(),
      row: `<span class="grow" data-iid="${i.id}">${pad(i.id, 4)}  ${esc(i.name)}  [${esc(i.category_jp)}]</span>`,
      onPick: () => onPick(i.id),
    }));
  openPicker("item", "アイテムを選択", entries, { lockCat });
}

// ---------- input commits ----------
function commitInput(el) {
  if (!app.save) return;
  const kind = el.dataset.commit;
  const raw = el.value.trim();
  const fail = (msg) => {
    el.classList.add("invalid");
    setStatus(msg, true);
  };
  const num = (min, max, msg) => {
    const v = raw === "" ? NaN : Number(raw);
    if (!Number.isInteger(v) || v < min || v > max) {
      fail(msg);
      return null;
    }
    return v;
  };
  const apply = (fn, msg) => {
    try {
      mutate(fn, msg);
    } catch (e) {
      fail(`エラー: ${e.message || e}`);
    }
  };
  const idx = () => +el.dataset.idx;
  const target = () => el.dataset.target;

  switch (kind) {
    case "name": {
      const lname = $("#content").querySelector('[data-key="lname"]').value;
      const fname = $("#content").querySelector('[data-key="fname"]').value;
      try {
        mutate((s) => s.set_name(lname, fname));
      } catch {
        fail("名前に使用できない文字が含まれています");
      }
      return;
    }
    case "money": {
      const v = num(0, 9999999, "所持金は 0-9999999 で入力してください");
      if (v == null) return;
      apply((s) => s.set_money(v));
      return;
    }
    case "mcLevel": {
      const v = num(1, 99, "レベルは 1-99 で入力してください");
      if (v == null) return;
      apply((s) => s.set_mc_level(v));
      return;
    }
    case "mcXp": {
      const v = num(0, 9999999, "XP は 0-9999999 で入力してください");
      if (v == null) return;
      apply((s) => s.set_mc_xp(v));
      return;
    }
    case "mcHp": {
      const v = num(0, 9999, "HP は 0-9999 で入力してください");
      if (v == null) return;
      apply((s) => s.set_mc_hp(v));
      return;
    }
    case "mcSp": {
      const v = num(0, 9999, "SP は 0-9999 で入力してください");
      if (v == null) return;
      apply((s) => s.set_mc_sp(v));
      return;
    }
    case "playtime": {
      const m = raw.match(/^(\d+):(\d{1,2}):(\d{1,2})$/);
      if (!m || +m[1] > 999 || +m[2] > 59 || +m[3] > 59) {
        fail("プレイ時間は H:MM:SS (0-999 時間) で入力してください");
        return;
      }
      apply((s) => s.set_playtime(+m[1], +m[2], +m[3]));
      return;
    }
    case "socialStat": {
      const v = num(0, 999, "0-999 で入力してください");
      if (v == null) return;
      apply((s) => s.set_social_stat(+el.dataset.key, v));
      return;
    }
    case "riseLines": {
      const v = num(0, 255, "0-255 で入力してください");
      if (v == null) return;
      apply((s) => s.set_rise_lines(v));
      return;
    }
    case "date": {
      const serial = dateToSerial(raw);
      if (serial < 0) {
        fail("日付は 2011/04/01 - 2012/03/20 の範囲で入力してください");
        return;
      }
      if (el.dataset.key === "next") apply((s) => s.set_next_day(serial));
      else apply((s) => s.set_day_serial(serial));
      return;
    }
    case "daySerial": {
      const v = num(0, MAX_SERIAL, `0-${MAX_SERIAL} で入力してください`);
      if (v == null) return;
      apply((s) => s.set_day_serial(v));
      return;
    }
    case "nextDay": {
      const v = num(0, MAX_SERIAL, `0-${MAX_SERIAL} で入力してください`);
      if (v == null) return;
      apply((s) => s.set_next_day(v));
      return;
    }
    case "unitHp": {
      const v = num(0, 9999, "HP は 0-9999 で入力してください");
      if (v == null) return;
      apply((s) => s.unit_set_hp(idx(), v));
      return;
    }
    case "unitSp": {
      const v = num(0, 9999, "SP は 0-9999 で入力してください");
      if (v == null) return;
      apply((s) => s.unit_set_sp(idx(), v));
      return;
    }
    case "pLevel": {
      const v = num(1, 99, "レベルは 1-99 で入力してください");
      if (v == null) return;
      apply((s) => s.persona_set_level(target(), idx(), v));
      return;
    }
    case "pXp": {
      const v = num(0, 9999999, "XP は 0-9999999 で入力してください");
      if (v == null) return;
      apply((s) => s.persona_set_xp(target(), idx(), v));
      return;
    }
    case "pStat": {
      const v = num(1, 99, "1-99 で入力してください");
      if (v == null) return;
      apply((s) => s.persona_set_stat(target(), idx(), +el.dataset.stat, v));
      return;
    }
    case "socialRank": {
      const v = num(0, 10, "ランクは 0-10 で入力してください");
      if (v == null) return;
      apply((s) => s.social_set_rank(idx(), v));
      return;
    }
    case "socialAff": {
      const v = num(0, 999, "好感度は 0-999 で入力してください");
      if (v == null) return;
      apply((s) => s.social_set_affinity(idx(), v));
      return;
    }
    case "itemCount": {
      const v = num(0, 99, "個数は 0-99 で入力してください");
      if (v == null) return;
      apply((s) => s.set_item_count(+el.dataset.idx, v));
      return;
    }
  }
}

// ---------- action dispatch ----------
async function dispatch(act, el) {
  const idx = () => +el.dataset.idx;
  switch (act) {
    // top bar
    case "open-dir":
      if (await ensureClean()) await openFolder();
      break;
    case "open-file":
      if (await ensureClean()) await openFiles();
      break;
    case "toggle-detail":
      app.detail = !app.detail;
      render();
      break;
    case "save":
      await saveCurrent();
      break;
    case "prev-slot":
    case "next-slot": {
      const next = app.selSlot + (act === "prev-slot" ? -1 : 1);
      if (next < 0 || next >= app.slots.length) break;
      if (!(await ensureClean())) break;
      app.selSlot = next;
      app.save = app.slots[next].save;
      refreshState();
      render();
      setStatus(`スロット: ${app.slots[next].name}`);
      break;
    }

    // 基本
    case "money-max": mutate((s) => s.money_max()); break;
    case "level-99": mutate((s) => s.mc_level_99()); break;
    case "hp-sp-max": mutate((s) => s.mc_hp_sp_max()); break;
    case "diff-cycle": mutate((s) => s.cycle_difficulty()); break;
    case "social-stats-max": mutate((s) => s.max_social_stats()); break;
    case "party-cycle":
      try { mutate((s) => s.cycle_party_member(idx())); }
      catch (e) { setStatus(`エラー: ${e.message || e}`, true); }
      break;

    // 日付
    case "phase-next": mutate((s) => s.cycle_phase()); break;
    case "next-phase-next": mutate((s) => s.cycle_next_phase()); break;
    case "sync-next": mutate((s) => s.sync_next_date()); break;

    // 仲間
    case "goto-persona":
      app.page = "persona";
      app.selPSlot = 0;
      render();
      break;
    case "clear-status":
      try { mutate((s) => s.unit_clear_status(app.selUnit)); }
      catch (e) { setStatus(`エラー: ${e.message || e}`, true); }
      break;
    case "heal-all": mutate((s) => s.heal_party()); break;
    case "equip-pick": {
      const meta = EQUIP_SLOTS[+el.dataset.slot];
      openItemPicker(meta.cat, (id) => {
        try {
          mutate((s) => s.set_equipment(+el.dataset.unit, +el.dataset.slot, id));
        } catch (e) { setStatus(`エラー: ${e.message || e}`, true); }
      });
      break;
    }
    case "equip-none":
      try { mutate((s) => s.set_equipment(+el.dataset.unit, +el.dataset.slot, 0)); }
      catch (e) { setStatus(`エラー: ${e.message || e}`, true); }
      break;

    // persona editor
    case "persona-pick":
      openPersonaPicker(el.dataset.target, idx());
      break;
    case "persona-clear":
      try {
        mutate((s) => s.persona_clear(el.dataset.target || "mc", idx()));
      } catch (e) { setStatus(`エラー: ${e.message || e}`, true); }
      break;
    case "persona-max-all": mutate((s) => s.persona_max_all()); break;
    case "skill-pick":
      openSkillPicker(el.dataset.target, idx(), +el.dataset.pslot);
      break;
    case "skill-clear":
      try {
        mutate((s) => s.persona_set_skill(el.dataset.target, idx(), +el.dataset.pslot, 0));
      } catch (e) { setStatus(`エラー: ${e.message || e}`, true); }
      break;

    // 全書
    case "comp-fill":
      if (await confirmDestructive("全書をすべて登録します。実行しますか？"))
        mutate((s) => s.compendium_fill());
      break;
    case "comp-reset":
      if (await confirmDestructive("全書を未登録に戻します。実行しますか？"))
        mutate((s) => s.compendium_reset());
      break;
    case "comp-clear":
      if (await confirmDestructive("全書の登録をすべて解除します。実行しますか？"))
        mutate((s) => s.compendium_clear());
      break;
    case "comp-toggle":
      try { mutate((s) => s.compendium_toggle(idx())); }
      catch (e) { setStatus(`エラー: ${e.message || e}`, true); }
      break;
    case "comp-only-missing":
      app.compOnlyMissing = !app.compOnlyMissing;
      app.scroll["comp-list"] = 0;
      render();
      break;

    // コミュ
    case "social-max":
      try { mutate((s) => s.social_max(idx())); }
      catch (e) { setStatus(`エラー: ${e.message || e}`, true); }
      break;
    case "social-max-all": mutate((s) => s.social_max_all()); break;

    // アイテム
    case "item-cat-cycle": {
      const i = CATEGORY_OPTIONS.findIndex(([k]) => k === app.itemCat);
      app.itemCat = CATEGORY_OPTIONS[(i + 1) % CATEGORY_OPTIONS.length][0];
      app.scroll["item-list"] = 0;
      render();
      break;
    }
    case "item-owned":
      app.itemOwnedOnly = !app.itemOwnedOnly;
      app.scroll["item-list"] = 0;
      render();
      break;
    case "item-add":
      openItemPicker(null, (id) => {
        const count = invCount(id);
        if (count === 0) {
          mutate((s) => s.set_item_count(id, 1), `追加しました: ${itemName(id)}`);
        } else {
          setStatus(`所持しています: ${itemName(id)} x${count}`);
        }
      });
      break;
    case "items-all-99":
      if (await confirmDestructive("全アイテムを 99 個所持にします。実行しますか？"))
        mutate((s) => s.add_all_items(99));
      break;
    case "items-clear":
      if (await confirmDestructive("所持アイテムをすべて削除します。実行しますか？"))
        mutate((s) => s.clear_inventory());
      break;

    // ツール
    case "verify": {
      try {
        const checks = JSON.parse(app.save.verify_json());
        const ok = checks.header && checks.save_md5 && checks.slot_md5;
        const slot = app.slots[app.selSlot];
        if (slot) slot.checks = checks;
        render();
        setStatus(ok ? "チェックサム: すべて OK" : "チェックサムに問題があります", !ok);
      } catch (e) {
        setStatus(`エラー: ${e.message || e}`, true);
      }
      break;
    }
    case "backup-now": {
      const slot = app.slots[app.selSlot];
      if (!slot) break;
      const out = app.save.to_bytes();
      slot.backups.push({
        label: `${new Date().toLocaleTimeString("ja-JP")} (手動)`,
        bin: new Uint8Array(out[0]),
        slot: new Uint8Array(out[1]),
      });
      render();
      setStatus("バックアップを作成しました");
      break;
    }
    case "backup-restore": {
      const slot = app.slots[app.selSlot];
      const backup = slot && slot.backups[idx()];
      if (!backup) break;
      const ok = await showModal({
        message: "このバックアップに復元しますか? 現在の変更は失われます。",
        buttons: [
          { label: "復元", value: true, primary: true },
          { label: "キャンセル", value: false },
        ],
      });
      if (!ok) break;
      try {
        slot.save = new P4gSave(backup.bin, backup.slot);
        slot.save.set_file_name(slot.name);
        app.save = slot.save;
        slot.modified = true;
        refreshState();
        render();
        setStatus("バックアップから復元しました");
      } catch (e) {
        setStatus(`復元に失敗しました: ${e.message || e}`, true);
      }
      break;
    }
    case "clone": {
      const num = +$("#clone-slot").value;
      if (!Number.isInteger(num) || num < 1 || num > 16) {
        setStatus("スロット番号は 1-16 で入力してください", true);
        break;
      }
      const binName = `data${pad(num, 4)}.bin`;
      const slotName = `data${pad(num, 4)}.binslot`;
      try {
        // Work on a clone so the active save's slot metadata is untouched.
        const cur = app.save.to_bytes();
        const copy = new P4gSave(new Uint8Array(cur[0]), new Uint8Array(cur[1]));
        copy.set_file_name(binName);
        const out = copy.to_bytes();
        let wrote = false;
        if (app.dirHandle) {
          wrote = await writeOrDownload(app.dirHandle, binName, new Uint8Array(out[0]));
          await writeOrDownload(app.dirHandle, slotName, new Uint8Array(out[1]));
        } else {
          download(binName, new Uint8Array(out[0]));
          download(slotName, new Uint8Array(out[1]));
        }
        // If the target slot is loaded, swap in the cloned bytes so a later
        // save cannot overwrite the clone with stale in-memory data.
        const existing = app.slots.find((s) => stem(s.name) === stem(binName));
        if (existing && existing !== slot) {
          existing.save = new P4gSave(new Uint8Array(out[0]), new Uint8Array(out[1]));
          existing.save.set_file_name(binName);
          existing.lastBytes = { bin: new Uint8Array(out[0]), slot: new Uint8Array(out[1]) };
          existing.modified = false;
          existing.checks = JSON.parse(existing.save.checks_json());
        }
        refreshState();
        render();
        setStatus(wrote ? `書き込みました: ${binName}` : `複製しました: ${binName}`);
      } catch (e) {
        setStatus(`複製に失敗しました: ${e.message || e}`, true);
      }
      break;
    }
    case "dump": {
      try {
        const json = app.save.dump_json();
        const slotNum = pad(app.state.slot.number, 4);
        download(`p4g_slot${slotNum}.json`, new TextEncoder().encode(json));
        setStatus(`書き出しました: p4g_slot${slotNum}.json`);
      } catch (e) {
        setStatus(`エラー: ${e.message || e}`, true);
      }
      break;
    }
  }
}

// ---------- drag & drop ----------
// Returns { map, dir }: dir is set only when a directory was dropped, so a
// failed drop does not clear app.dirHandle and silently switch saves to
// download mode.
async function entriesFromDataTransfer(dt) {
  const map = new Map();
  let dir = null;
  const items = dt.items ? [...dt.items] : [];
  const handles = [];
  for (const item of items) {
    if (item.kind !== "file") continue;
    if (item.getAsFileSystemHandle) {
      try {
        handles.push(await item.getAsFileSystemHandle());
        continue;
      } catch { /* fall through to File */ }
    }
    const f = item.getAsFile();
    if (f) map.set(f.name, f);
  }
  for (const h of handles) {
    if (h.kind === "file") {
      map.set(h.name, h);
    } else if (h.kind === "directory") {
      dir = h;
      for await (const entry of h.values()) {
        if (entry.kind === "file") map.set(entry.name, entry);
      }
    }
  }
  return { map, dir };
}

// ---------- boot ----------
async function boot() {
  // hidden file inputs for the non-FSAA fallbacks
  const fileInput = document.createElement("input");
  fileInput.type = "file";
  fileInput.id = "file-input";
  fileInput.multiple = true;
  fileInput.accept = ".bin,.binslot";
  fileInput.style.display = "none";
  document.body.appendChild(fileInput);
  const dirInput = document.createElement("input");
  dirInput.type = "file";
  dirInput.id = "file-dir-input";
  dirInput.setAttribute("webkitdirectory", "");
  dirInput.style.display = "none";
  document.body.appendChild(dirInput);

  fileInput.addEventListener("change", async () => {
    const map = new Map();
    for (const f of fileInput.files) map.set(f.name, f);
    fileInput.value = "";
    const recs = pairRecs(map);
    if (!recs.length) { setStatus("dataXXXX.bin を選んでください", true); return; }
    app.dirHandle = null;
    const { entries, errors } = await loadSlotRecs(recs);
    await adoptSlots(entries);
    if (errors.length) setStatus(`エラー: ${errors.join("; ")}`, true);
  });
  dirInput.addEventListener("change", async () => {
    const map = new Map();
    for (const f of dirInput.files) {
      const name = f.name; // webkitRelativePath would include the folder prefix
      map.set(name, f);
    }
    dirInput.value = "";
    const recs = pairRecs(map);
    if (!recs.length) { setStatus("フォルダ内に dataXXXX.bin が見つかりません", true); return; }
    app.dirHandle = null; // read-only mode: writes become downloads
    const { entries, errors } = await loadSlotRecs(recs);
    await adoptSlots(entries, " (読み取り専用)");
    if (errors.length) setStatus(`エラー: ${errors.join("; ")}`, true);
  });

  // delegated clicks for data-act buttons
  document.addEventListener("click", (e) => {
    const el = e.target.closest("[data-act]");
    if (!el) return;
    dispatch(el.dataset.act, el);
  });
  // delegated row selection
  document.addEventListener("click", (e) => {
    const el = e.target.closest("[data-sel]");
    if (!el || e.target.closest("[data-act]") || e.target.closest("input")) return;
    const i = +el.dataset.idx;
    if (el.dataset.sel === "unit") app.selUnit = i;
    else if (el.dataset.sel === "pslot") app.selPSlot = i;
    else if (el.dataset.sel === "comp") app.selCompId = i;
    saveScroll();
    render();
  });
  // delegated input commits
  document.addEventListener("change", (e) => {
    const el = e.target.closest("[data-commit]");
    if (el) commitInput(el);
  });
  // nav
  document.querySelectorAll("#nav button").forEach((btn) => {
    btn.addEventListener("click", () => {
      if (!app.save) return;
      if (app.page === btn.dataset.page) return;
      saveScroll();
      app.page = btn.dataset.page;
      render();
    });
  });


  window.addEventListener("dragover", (e) => e.preventDefault());
  window.addEventListener("drop", async (e) => {
    e.preventDefault();
    if (!(await ensureClean())) return;
    try {
      const { map, dir } = await entriesFromDataTransfer(e.dataTransfer);
      const recs = pairRecs(map);
      if (!recs.length) { setStatus("dataXXXX.bin が見つかりません", true); return; }
      app.dirHandle = dir;
      const { entries, errors } = await loadSlotRecs(recs);
      await adoptSlots(entries);
      if (errors.length) setStatus(`エラー: ${errors.join("; ")}`, true);
    } catch (e2) {
      setStatus(`エラー: ${e2.message || e2}`, true);
    }
  });
  window.addEventListener("beforeunload", (e) => {
    if (app.slots.some((s) => s.modified)) e.preventDefault();
  });

  try {
    await init("./pkg/p4g_wasm_bg.wasm");
  } catch (e) {
    setStatus(`wasm の読み込みに失敗しました: ${e.message || e}`, true);
    contentEl.innerHTML =
      '<div class="empty-state">wasm の読み込みに失敗しました</div>';
    return;
  }
  setStatus("dataXXXX.bin を開いてください");
  render();
}

boot();
