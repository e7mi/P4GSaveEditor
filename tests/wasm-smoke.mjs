// Smoke test for the wasm build using a synthetic save (no game files needed).
// Usage: node tests/wasm-smoke.mjs [path/to/dataXXXX.bin]
// With a real .bin path it also validates checksums against the matching .binslot.
import { readFileSync } from "node:fs";
import init, { P4gSave } from "../web/pkg/p4g_wasm.js";

const wasmPath = new URL("../web/pkg/p4g_wasm_bg.wasm", import.meta.url);
await init({ module_or_path: readFileSync(wasmPath) });

const SAVE_SIZE = 0x35351;
const SLOT_SIZE = 884;

let bin = new Uint8Array(SAVE_SIZE);
let slot = new Uint8Array(SLOT_SIZE);
slot.set([...Buffer.from("SAVE0001")], 0);

const arg = process.argv[2];
if (arg) {
  bin = new Uint8Array(readFileSync(arg));
  try {
    slot = new Uint8Array(readFileSync(arg.replace(/\.bin$/, ".binslot")));
  } catch {
    slot = null;
  }
}

const save = new P4gSave(bin, slot ?? undefined);
if (arg) save.set_file_name(arg.split(/[\\/]/).pop());

if (arg && slot) {
  const checks = JSON.parse(save.checks_json());
  console.log("loaded checks:", checks);
  if (!(checks.header && checks.save_md5 && checks.slot_md5)) {
    throw new Error("checksums invalid on load");
  }
}

save.set_money(7654321);
save.set_name("テスト", "次郎");
save.set_mc_level(88);
save.max_social_stats();
save.compendium_fill();
save.add_all_items(99);
save.persona_assign("mc", 0, 1);
save.persona_set_level("mc", 0, 99);
save.set_day_serial(100);
save.set_playtime(10, 20, 30);
save.unit_set_hp(1, 1234);
save.set_equipment(1, 0, 5);
save.set_item_count(2559, 7); // max item id; INVENTORY_SIZE boundary
save.heal_party(); // must heal unit 0 (MC) too

const [outBin, outSlot] = save.to_bytes();
const reloaded = new P4gSave(new Uint8Array(outBin), new Uint8Array(outSlot));
const state = JSON.parse(reloaded.state_json());
const checks = JSON.parse(reloaded.verify_json());

if (!(checks.header && checks.save_md5 && checks.slot_md5)) {
  throw new Error("checksums invalid after edit");
}
if (state.base.money !== 7654321) throw new Error("money mismatch");
if (state.base.level !== 88) throw new Error("level mismatch");
if (state.base.lname !== "テスト") throw new Error("name mismatch");
if (state.base.day_serial !== 100) throw new Error("day_serial mismatch");
if (state.base.playtime.join(",") !== "10,20,30")
  throw new Error("playtime mismatch");
if (state.party_units[0].hp !== 9999)
  throw new Error("heal_party did not heal MC (unit 0)");
if (state.party_units[1].hp !== 9999)
  throw new Error("heal_party did not heal unit 1");
if (state.party_units[1].equipment[0] !== 5)
  throw new Error("equipment mismatch");
if (state.inventory[2559] !== 7) throw new Error("item 2559 not stored");
console.log("wasm smoke test OK");
