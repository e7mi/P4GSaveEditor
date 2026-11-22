// Unit tests for web/pure.js: run the classic script in a vm context so the
// same file the browser loads is what gets exercised.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import assert from "node:assert/strict";
import vm from "node:vm";

const ctx = vm.createContext({});
vm.runInContext(
  readFileSync(new URL("../web/pure.js", import.meta.url), "utf8"),
  ctx
);
const { stem, pairRecs, dateToSerial, serialToDate } = ctx;

test("stem strips .bin/.binslot case-insensitively, preserving case", () => {
  assert.equal(stem("data0001.bin"), "data0001");
  assert.equal(stem("DATA0002.BIN"), "DATA0002");
  assert.equal(stem("Data0003.BinSlot"), "Data0003");
  assert.equal(stem("data0001.binslot"), "data0001");
});

test("pairRecs pairs .bin with .binslot regardless of case", () => {
  const map = new Map([
    ["DATA0002.BIN", "bin-a"],
    ["data0002.binslot", "slot-a"],
    ["data0003.bin", "bin-b"],
    ["readme.txt", "x"],
  ]);
  const recs = pairRecs(map);
  assert.equal(recs.length, 2);
  assert.equal(recs[0].name, "data0003.bin" < "DATA0002.BIN" ? "data0003.bin" : "DATA0002.BIN");
  const upper = recs.find((r) => r.name === "DATA0002.BIN");
  assert.equal(upper.slotFile, "slot-a");
  const lower = recs.find((r) => r.name === "data0003.bin");
  assert.equal(lower.slotFile, null);
});

test("dateToSerial round-trips and rejects invalid dates", () => {
  assert.equal(dateToSerial("2011/04/01"), 0);
  assert.equal(dateToSerial("2011-04-02"), 1);
  assert.equal(dateToSerial("2012/03/20"), 354);
  assert.equal(dateToSerial("2011/03/31"), -1);
  assert.equal(dateToSerial("2012/03/21"), -1);
  assert.equal(dateToSerial("2011/13/32"), -1);
  assert.equal(dateToSerial("2011/02/30"), -1);
  assert.equal(dateToSerial("x"), -1);
});

test("serialToDate is the inverse", () => {
  assert.equal(serialToDate(0).iso, "2011-04-01");
  assert.equal(serialToDate(354).iso, "2012-03-20");
});
