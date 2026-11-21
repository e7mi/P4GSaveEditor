use js_sys::Array;
use p4g_core::game::GameData;
use p4g_core::save::{self, P4GSave, SLOT_MAGIC, SLOT_SIZE};
use serde_json::{Value, json};
use std::path::PathBuf;
use wasm_bindgen::prelude::*;

const MAX_MONEY: u32 = save::MAX_MONEY;
const LEVEL99_XP: u32 = 3_902_496;

#[derive(Clone, Copy)]
enum PersonaTarget {
    Mc(usize),
    Party(usize),
    Compendium(usize),
}

impl PersonaTarget {
    fn parse(target: &str, index: usize) -> Result<Self, JsError> {
        match target {
            "mc" if index < save::MC_PERSONA_COUNT => Ok(Self::Mc(index)),
            "party" if index < save::PARTY_UNIT_COUNT => Ok(Self::Party(index)),
            "compendium" if index < save::COMPENDIUM_COUNT => Ok(Self::Compendium(index)),
            _ => Err(JsError::new("persona target out of range")),
        }
    }
}

#[wasm_bindgen]
pub struct P4gSave {
    save: P4GSave,
    game: GameData,
}

#[wasm_bindgen]
impl P4gSave {
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8], slot: Option<Vec<u8>>) -> Result<P4gSave, JsError> {
        let slot = match slot {
            Some(slot) => slot,
            None => {
                let mut slot = vec![0u8; SLOT_SIZE];
                slot[..8].copy_from_slice(SLOT_MAGIC);
                slot
            }
        };
        let save = P4GSave::from_bytes(
            data.to_vec(),
            slot,
            PathBuf::from("data0001.bin"),
            PathBuf::from("data0001.binslot"),
        )
        .map_err(js_err)?;
        Ok(Self {
            save,
            game: GameData::load(),
        })
    }

    pub fn state_json(&self) -> Result<String, JsError> {
        let state = json!({
            "slot": self.slot_json(),
            "base": self.base_json(),
            "party_units": self.party_units_json(),
            "mc_personas": self.personas_json(&self.save.mc_personas),
            "social_links": self.social_links_json(),
            "compendium": self.compendium_json(),
            "inventory": self.save.inventory,
            "game": self.game_json(),
        });
        serde_json::to_string(&state).map_err(js_err)
    }

    pub fn checks_json(&self) -> Result<String, JsError> {
        let checks = self.save.check_loaded();
        serde_json::to_string(&json!({
            "header": checks.header,
            "save_md5": checks.save_md5,
            "slot_md5": checks.slot_md5,
        }))
        .map_err(js_err)
    }

    pub fn verify_json(&mut self) -> Result<String, JsError> {
        let checks = self.save.verify();
        serde_json::to_string(&json!({
            "header": checks.header,
            "save_md5": checks.save_md5,
            "slot_md5": checks.slot_md5,
        }))
        .map_err(js_err)
    }

    // Recompute checksums and return [bin bytes, binslot bytes].
    pub fn to_bytes(&mut self) -> Array {
        let (data, slot) = self.save.apply(true);
        let out = Array::new();
        out.push(&js_sys::Uint8Array::from(data.as_slice()));
        out.push(&js_sys::Uint8Array::from(slot.as_slice()));
        out
    }

    // Keep the dataXXXX name so slot_number() and slot labels match the file.
    pub fn set_file_name(&mut self, name: &str) {
        let stem = name.trim_end_matches(".bin").trim_end_matches(".binslot");
        self.save.bin_path = PathBuf::from(format!("{stem}.bin"));
        self.save.slot_path = PathBuf::from(format!("{stem}.binslot"));
    }

    // Original loaded bytes, for reverting edits in memory.
    pub fn original_bytes(&self) -> Array {
        let out = Array::new();
        out.push(&js_sys::Uint8Array::from(
            self.save.original_data.as_slice(),
        ));
        out.push(&js_sys::Uint8Array::from(
            self.save.original_slot.as_slice(),
        ));
        out
    }

    pub fn set_name(&mut self, last: &str, first: &str) -> Result<(), JsError> {
        self.save.set_lname(last).map_err(js_err)?;
        self.save.set_fname(first).map_err(js_err)
    }

    pub fn set_money(&mut self, value: u32) {
        self.save.set_money(value);
    }

    pub fn money_max(&mut self) {
        self.save.set_money(MAX_MONEY);
    }

    pub fn set_mc_level(&mut self, value: u16) {
        self.save.set_mc_level(value);
    }

    pub fn mc_level_99(&mut self) {
        self.save.set_mc_level(99);
        self.save.set_mc_xp(LEVEL99_XP);
    }

    pub fn set_mc_xp(&mut self, value: u32) {
        self.save.set_mc_xp(value);
    }

    pub fn set_mc_hp(&mut self, value: u16) {
        self.save.set_mc_hp(value);
    }

    pub fn set_mc_sp(&mut self, value: u16) {
        self.save.set_mc_sp(value);
    }

    pub fn mc_hp_sp_max(&mut self) {
        self.save.set_mc_hp(9999);
        self.save.set_mc_sp(9999);
    }

    pub fn set_playtime(&mut self, hours: u32, minutes: u32, seconds: u32) {
        self.save.set_playtime(hours, minutes, seconds);
    }

    pub fn set_difficulty(&mut self, value: u32) {
        self.save.set_difficulty(value);
    }

    pub fn cycle_difficulty(&mut self) {
        let value = (self.save.difficulty() + 1) % 5;
        self.save.set_difficulty(value);
    }

    pub fn set_social_stat(&mut self, index: usize, value: u16) -> Result<(), JsError> {
        let mut stats = self.save.social_stats();
        let Some(stat) = stats.get_mut(index) else {
            return Err(JsError::new("social stat index out of range"));
        };
        *stat = value;
        self.save.set_social_stats(stats);
        Ok(())
    }

    pub fn max_social_stats(&mut self) {
        self.save.max_social_stats();
    }

    pub fn set_party_member(&mut self, slot: usize, value: u16) -> Result<(), JsError> {
        let mut party = self.save.party();
        let Some(entry) = party.get_mut(slot) else {
            return Err(JsError::new("party slot out of range"));
        };
        *entry = value;
        self.save.set_party(party);
        Ok(())
    }

    pub fn cycle_party_member(&mut self, slot: usize) -> Result<(), JsError> {
        let mut party = self.save.party();
        let Some(entry) = party.get_mut(slot) else {
            return Err(JsError::new("party slot out of range"));
        };
        *entry = (*entry + 1) % 9;
        self.save.set_party(party);
        Ok(())
    }

    pub fn set_rise_lines(&mut self, value: u8) {
        self.save.set_rise_lines(value);
    }

    pub fn set_day_serial(&mut self, value: u16) {
        self.save.set_day_serial(value);
    }

    pub fn set_phase(&mut self, value: u16) {
        self.save.set_phase(value);
    }

    pub fn cycle_phase(&mut self) {
        let value = (self.save.phase() + 1) % 6;
        self.save.set_phase(value);
    }

    pub fn set_next_day(&mut self, value: u16) {
        self.save.set_next_day(value);
    }

    pub fn set_next_phase(&mut self, value: u16) {
        self.save.set_next_phase(value);
    }

    pub fn cycle_next_phase(&mut self) {
        let value = (self.save.next_phase() + 1) % 6;
        self.save.set_next_phase(value);
    }

    pub fn sync_next_date(&mut self) {
        let day = self.save.day_serial();
        let phase = self.save.phase();
        self.save.set_next_day(day);
        self.save.set_next_phase(phase);
    }

    pub fn unit_set_hp(&mut self, index: usize, value: u16) -> Result<(), JsError> {
        if index == 0 {
            self.save.set_mc_hp(value);
            return Ok(());
        }
        let Some(unit) = self.save.units.get_mut(index) else {
            return Err(JsError::new("unit index out of range"));
        };
        unit.set_hp(value);
        Ok(())
    }

    pub fn unit_set_sp(&mut self, index: usize, value: u16) -> Result<(), JsError> {
        if index == 0 {
            self.save.set_mc_sp(value);
            return Ok(());
        }
        let Some(unit) = self.save.units.get_mut(index) else {
            return Err(JsError::new("unit index out of range"));
        };
        unit.set_sp(value);
        Ok(())
    }

    pub fn unit_clear_status(&mut self, index: usize) -> Result<(), JsError> {
        let Some(unit) = self.save.units.get_mut(index) else {
            return Err(JsError::new("unit index out of range"));
        };
        unit.clear_status();
        Ok(())
    }

    pub fn heal_party(&mut self) {
        self.save.heal_party();
    }

    pub fn set_equipment(&mut self, unit: usize, slot: usize, item_id: u16) -> Result<(), JsError> {
        if slot >= 4 {
            return Err(JsError::new("equipment slot out of range"));
        }
        if unit == 0 {
            self.save.set_mc_equipment(slot, item_id);
            return Ok(());
        }
        let Some(entry) = self.save.units.get_mut(unit) else {
            return Err(JsError::new("unit index out of range"));
        };
        entry.set_equipment(slot, item_id);
        Ok(())
    }

    fn persona(&self, target: PersonaTarget) -> Option<save::Persona> {
        match target {
            PersonaTarget::Mc(index) => self.save.mc_personas.get(index).copied(),
            PersonaTarget::Party(index) => self.save.units.get(index).map(|unit| unit.persona()),
            PersonaTarget::Compendium(index) => self.save.compendium.get(index).copied(),
        }
    }

    fn set_persona(&mut self, target: PersonaTarget, persona: save::Persona) {
        match target {
            PersonaTarget::Mc(index) => {
                if let Some(entry) = self.save.mc_personas.get_mut(index) {
                    *entry = persona;
                }
            }
            PersonaTarget::Party(index) => {
                if let Some(unit) = self.save.units.get_mut(index) {
                    unit.set_persona(&persona);
                }
            }
            PersonaTarget::Compendium(index) => {
                if let Some(entry) = self.save.compendium.get_mut(index) {
                    *entry = persona;
                }
            }
        }
    }

    fn persona_mut(&mut self, target: PersonaTarget) -> Result<save::Persona, JsError> {
        let persona = self
            .persona(target)
            .ok_or_else(|| JsError::new("persona slot out of range"))?;
        if persona.id() == 0 {
            return Err(JsError::new("no persona in this slot"));
        }
        Ok(persona)
    }

    pub fn persona_assign(&mut self, target: &str, index: usize, id: u16) -> Result<(), JsError> {
        let target = PersonaTarget::parse(target, index)?;
        let info = self
            .game
            .persona(id)
            .ok_or_else(|| JsError::new("unknown persona id"))?;
        let mut persona = self
            .persona(target)
            .unwrap_or(save::Persona { raw: [0; 48] });
        persona.fill_base(info);
        self.set_persona(target, persona);
        Ok(())
    }

    pub fn persona_clear(&mut self, target: &str, index: usize) -> Result<(), JsError> {
        let target = PersonaTarget::parse(target, index)?;
        let mut persona = self
            .persona(target)
            .ok_or_else(|| JsError::new("persona slot out of range"))?;
        persona.clear();
        self.set_persona(target, persona);
        Ok(())
    }

    pub fn persona_set_level(
        &mut self,
        target: &str,
        index: usize,
        value: u16,
    ) -> Result<(), JsError> {
        let target = PersonaTarget::parse(target, index)?;
        let mut persona = self.persona_mut(target)?;
        persona.set_level(value);
        self.set_persona(target, persona);
        Ok(())
    }

    pub fn persona_set_xp(
        &mut self,
        target: &str,
        index: usize,
        value: u32,
    ) -> Result<(), JsError> {
        let target = PersonaTarget::parse(target, index)?;
        let mut persona = self.persona_mut(target)?;
        persona.set_xp(value);
        self.set_persona(target, persona);
        Ok(())
    }

    pub fn persona_set_stat(
        &mut self,
        target: &str,
        index: usize,
        stat: usize,
        value: u8,
    ) -> Result<(), JsError> {
        if stat >= 5 {
            return Err(JsError::new("stat index out of range"));
        }
        let target = PersonaTarget::parse(target, index)?;
        let mut persona = self.persona_mut(target)?;
        let mut stats = persona.stats();
        stats[stat] = value;
        persona.set_stats(&stats);
        self.set_persona(target, persona);
        Ok(())
    }

    pub fn persona_set_skill(
        &mut self,
        target: &str,
        index: usize,
        slot: usize,
        skill_id: u16,
    ) -> Result<(), JsError> {
        if slot >= 8 {
            return Err(JsError::new("skill slot out of range"));
        }
        let target = PersonaTarget::parse(target, index)?;
        let mut persona = self
            .persona(target)
            .ok_or_else(|| JsError::new("persona slot out of range"))?;
        let mut skills = persona.skills();
        skills[slot] = skill_id;
        persona.set_skills(&skills);
        self.set_persona(target, persona);
        Ok(())
    }

    pub fn persona_max_all(&mut self) {
        self.save.max_personas(&self.game);
    }

    pub fn compendium_toggle(&mut self, index: usize) -> Result<(), JsError> {
        let Some(entry) = self.save.compendium.get_mut(index) else {
            return Err(JsError::new("compendium index out of range"));
        };
        if entry.exists() {
            entry.clear();
        } else if let Some(info) = self.game.persona(index as u16 + 1) {
            entry.fill_base(info);
        }
        Ok(())
    }

    pub fn compendium_fill(&mut self) {
        self.save.fill_compendium(&self.game);
    }

    pub fn compendium_reset(&mut self) {
        self.save.reset_compendium(&self.game);
    }

    pub fn compendium_clear(&mut self) {
        self.save.clear_compendium();
    }

    pub fn social_set_rank(&mut self, index: usize, value: u16) -> Result<(), JsError> {
        let Some(link) = self.save.social_links.get_mut(index) else {
            return Err(JsError::new("social link index out of range"));
        };
        if link.id() == 0 {
            return Err(JsError::new("unused social link"));
        }
        link.set_rank(value);
        Ok(())
    }

    pub fn social_set_affinity(&mut self, index: usize, value: u16) -> Result<(), JsError> {
        let Some(link) = self.save.social_links.get_mut(index) else {
            return Err(JsError::new("social link index out of range"));
        };
        if link.id() == 0 {
            return Err(JsError::new("unused social link"));
        }
        link.set_affinity(value);
        Ok(())
    }

    pub fn social_max(&mut self, index: usize) -> Result<(), JsError> {
        let Some(link) = self.save.social_links.get_mut(index) else {
            return Err(JsError::new("social link index out of range"));
        };
        if link.id() == 0 {
            return Err(JsError::new("unused social link"));
        }
        link.set_rank(10);
        link.set_affinity(0);
        Ok(())
    }

    pub fn social_max_all(&mut self) {
        self.save.max_social_links();
    }

    pub fn set_item_count(&mut self, item_id: u16, count: u8) {
        self.save.set_item_count(item_id, count);
    }

    pub fn clear_inventory(&mut self) {
        self.save.clear_inventory();
    }

    pub fn add_all_items(&mut self, count: u8) {
        self.save.add_all_items(count, false, &self.game);
    }

    pub fn dump_json(&self) -> Result<String, JsError> {
        serde_json::to_string_pretty(&self.save.dump_json(&self.game)).map_err(js_err)
    }
}

fn js_err(error: impl std::fmt::Display) -> JsError {
    JsError::new(&error.to_string())
}

impl P4gSave {
    fn slot_json(&self) -> Value {
        let info = self.save.slot_info();
        json!({
            "number": self.save.slot_number(),
            "name": info.name,
            "level": info.level,
            "date_text": info.date_text,
            "phase_text": info.phase_text,
            "location": info.location,
            "playtime": info.playtime,
        })
    }

    fn base_json(&self) -> Value {
        use p4g_core::game;
        let (hours, minutes, seconds) = self.save.playtime_hms();
        json!({
            "game_date": self.save.game_date().format("%Y/%m/%d").to_string(),
            "difficulty_jp": game::difficulty_jp(self.save.difficulty()),
            "phase_jp": game::phase_jp(self.save.phase()),
            "next_phase_jp": game::phase_jp(self.save.next_phase()),
            "lname": self.save.lname(),
            "fname": self.save.fname(),
            "money": self.save.money(),
            "level": self.save.mc_level(),
            "xp": self.save.mc_xp(),
            "hp": self.save.mc_hp(),
            "sp": self.save.mc_sp(),
            "playtime": [hours, minutes, seconds],
            "difficulty": self.save.difficulty(),
            "day_serial": self.save.day_serial(),
            "phase": self.save.phase(),
            "next_day": self.save.next_day(),
            "next_phase": self.save.next_phase(),
            "social_stats": self.save.social_stats(),
            "party": self.save.party(),
            "rise_lines": self.save.rise_lines(),
            "links_maxed": self.save.social_link_maxed(),
            "compendium_count": self.save.compendium_count(&self.game),
        })
    }

    fn persona_json(&self, persona: &save::Persona) -> Value {
        json!({
            "exists": persona.exists(),
            "id": persona.id(),
            "level": persona.level(),
            "xp": persona.xp(),
            "skills": persona.skills(),
            "stats": persona.stats(),
        })
    }

    fn personas_json(&self, personas: &[save::Persona]) -> Value {
        json!(
            personas
                .iter()
                .map(|persona| self.persona_json(persona))
                .collect::<Vec<_>>()
        )
    }

    fn party_units_json(&self) -> Value {
        json!(
            self.save
                .units
                .iter()
                .enumerate()
                .map(|(index, unit)| {
                    json!({
                        "id": unit.id(),
                        "hp": self.save.unit_hp(index),
                        "sp": self.save.unit_sp(index),
                        "equipment": if index == 0 {
                            [
                                self.save.mc_equipment(0),
                                self.save.mc_equipment(1),
                                self.save.mc_equipment(2),
                                self.save.mc_equipment(3),
                            ]
                        } else {
                            [
                                unit.equipment(0),
                                unit.equipment(1),
                                unit.equipment(2),
                                unit.equipment(3),
                            ]
                        },
                        "persona": self.persona_json(&unit.persona()),
                    })
                })
                .collect::<Vec<_>>()
        )
    }

    fn social_links_json(&self) -> Value {
        json!(
            self.save
                .social_links
                .iter()
                .map(|link| {
                    json!({
                        "id": link.id(),
                        "rank": link.rank(),
                        "affinity": link.affinity(),
                    })
                })
                .collect::<Vec<_>>()
        )
    }

    fn compendium_json(&self) -> Value {
        json!(
            self.save
                .compendium
                .iter()
                .map(|entry| {
                    json!({
                        "exists": entry.exists(),
                        "id": entry.id(),
                        "level": entry.level(),
                        "xp": entry.xp(),
                        "skills": entry.skills(),
                        "stats": entry.stats(),
                    })
                })
                .collect::<Vec<_>>()
        )
    }

    fn game_json(&self) -> Value {
        json!({
            "items": self
                .game
                .items
                .iter()
                .map(|item| {
                    json!({
                        "id": item.id,
                        "name": self.game.item_name(item.id),
                        "name_en": item.name,
                        "category": item.category_key(),
                        "category_jp": item.category_jp(),
                    })
                })
                .collect::<Vec<_>>(),
            "personas": self
                .game
                .personas
                .iter()
                .map(|info| {
                    json!({
                        "id": info.id,
                        "name": self.game.persona_name(info.id),
                        "name_en": info.name,
                        "arcana": info.arcana,
                        "level": info.level,
                    })
                })
                .collect::<Vec<_>>(),
            "skills": self
                .game
                .skills
                .iter()
                .map(|skill| {
                    json!({
                        "id": skill.id,
                        "name": self.game.skill_name(skill.id),
                        "name_en": skill.name,
                        "has_jp": self.game.has_jp_skill(skill.id),
                    })
                })
                .collect::<Vec<_>>(),
            "social": self
                .game
                .social
                .iter()
                .map(|info| {
                    json!({
                        "id": info.id,
                        "name": self.game.social_name(info.id),
                    })
                })
                .collect::<Vec<_>>(),
            "chars": self
                .game
                .chars
                .iter()
                .map(|(id, name)| json!({"id": id, "name": name}))
                .collect::<Vec<_>>(),
        })
    }
}
