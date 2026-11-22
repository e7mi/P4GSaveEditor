// Pure helpers with no DOM dependency, shared by app.js and node tests.
// Kept as a classic script so it also works from file://.

const EPOCH_UTC = Date.UTC(2011, 3, 1); // serial 0 = 2011/04/01
const DAY_MS = 86400000;
const MAX_SERIAL = 354;

function pad(value, width) {
  return String(value).padStart(width, "0");
}

function stem(name) {
  return name.replace(/\.binslot$/i, "").replace(/\.bin$/i, "");
}

// serial <-> date helpers; UTC everywhere so timezone never shifts a day
function serialToDate(serial) {
  const d = new Date(EPOCH_UTC + serial * DAY_MS);
  const y = d.getUTCFullYear();
  const m = pad(d.getUTCMonth() + 1, 2);
  const day = pad(d.getUTCDate(), 2);
  return { text: `${y}/${m}/${day}`, iso: `${y}-${m}-${day}` };
}

function dateToSerial(text) {
  const m = String(text).trim().match(/^(\d{4})[-/](\d{1,2})[-/](\d{1,2})$/);
  if (!m) return -1;
  const serial = Math.round(
    (Date.UTC(+m[1], +m[2] - 1, +m[3]) - EPOCH_UTC) / DAY_MS
  );
  if (serial < 0 || serial > MAX_SERIAL) return -1;
  const check = serialToDate(serial);
  if (check.iso !== `${m[1]}-${pad(m[2], 2)}-${pad(m[3], 2)}`) return -1;
  return serial;
}

const BIN_RE = /^data\d{4}\.bin$/i;

// Collect [bin, binslot] pairs from a flat name->entry map (File or handle).
// Matching is case-insensitive: the directory may mix DATA0002.BIN and
// data0002.binslot.
function pairRecs(map) {
  const lower = new Map([...map.keys()].map((k) => [k.toLowerCase(), k]));
  const recs = [];
  for (const [name, entry] of map) {
    if (BIN_RE.test(name)) {
      const slotName = lower.get(`${stem(name).toLowerCase()}.binslot`);
      recs.push({
        name,
        file: entry,
        slotFile: slotName ? map.get(slotName) : null,
      });
    }
  }
  recs.sort((a, b) => a.name.localeCompare(b.name));
  return recs;
}
