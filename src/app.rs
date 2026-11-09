use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use chrono::NaiveDate;
use gpui::prelude::*;
use gpui::*;

use crate::game::{self, GameData};
use crate::save::{self, P4GSave};
use crate::text_input::TextInput;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const BG: u32 = 0x0f1014;
const PANEL: u32 = 0x15161c;
const PANEL_2: u32 = 0x1b1c24;
const BORDER: u32 = 0x2a2c36;
const TEXT: u32 = 0xdedee6;
const MUTED: u32 = 0x8b8d9a;
const ACCENT: u32 = 0xd6b45a;
const ACCENT_DARK: u32 = 0x1a1608;
const RED: u32 = 0xd06a6a;
const INPUT_BG: u32 = 0x101116;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Basic,
    Date,
    Party,
    Personas,
    Compendium,
    Social,
    Inventory,
    Tools,
}

impl Page {
    fn label(self) -> &'static str {
        match self {
            Self::Basic => "基本",
            Self::Date => "日付",
            Self::Party => "仲間",
            Self::Personas => "ペルソナ",
            Self::Compendium => "全書",
            Self::Social => "コミュ",
            Self::Inventory => "アイテム",
            Self::Tools => "ツール",
        }
    }

    const ALL: [Page; 8] = [
        Page::Basic,
        Page::Date,
        Page::Party,
        Page::Personas,
        Page::Compendium,
        Page::Social,
        Page::Inventory,
        Page::Tools,
    ];
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum PersonaTarget {
    Mc(usize),
    Party(usize),
    Compendium(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ItemTarget {
    Inventory,
    Equip(usize, usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PickerTarget {
    Persona(PersonaTarget),
    Skill(PersonaTarget, usize),
    Item(ItemTarget),
}

#[derive(Clone, Copy)]
struct PickerState {
    target: PickerTarget,
    category: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Field {
    LastName,
    FirstName,
    Money,
    McLevel,
    McXp,
    McHp,
    McSp,
    Playtime,
    RiseLines,
    SocialStat(usize),
    DayDate,
    NextDate,
    DaySerial,
    NextDay,
    UnitHp(usize),
    UnitSp(usize),
    PersonaLevel(PersonaTarget),
    PersonaXp(PersonaTarget),
    PersonaStat(PersonaTarget, usize),
    SocialRank(usize),
    SocialAffinity(usize),
    ItemCount(u16),
    CopySlot,
}

struct FieldEntry {
    input: Entity<TextInput>,
    initial: String,
    error: bool,
    _sub: Subscription,
}

#[derive(Clone, Copy)]
enum Modal {
    SwitchSlot(usize),
    Reload,
    RestoreBackup(usize),
}

pub struct AppState {
    game: GameData,
    save: Option<P4GSave>,
    slots: Vec<PathBuf>,
    slot_labels: Vec<String>,
    selected_slot: usize,
    page: Page,
    advanced: bool,
    modified: bool,
    status: String,
    search_input: Entity<TextInput>,
    fields: HashMap<Field, FieldEntry>,
    picker: Option<PickerState>,
    modal: Option<Modal>,
    backups: Vec<PathBuf>,
    mc_persona_selected: usize,
    party_selected: usize,
    compendium_selected: usize,
    compendium_missing_only: bool,
    inventory_category: usize,
    inventory_owned_only: bool,
    inventory_selected: Option<u16>,
    copy_slot: u16,
}

impl AppState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| TextInput::new("検索", cx));
        cx.observe(&search_input, |_, _, cx| cx.notify()).detach();

        let game = GameData::load();
        let slots = collect_slots();
        let slot_labels = slots
            .iter()
            .map(|path| save::slot_summary_from_path(path))
            .collect();
        let mut app = Self {
            game,
            save: None,
            slots,
            slot_labels,
            selected_slot: 0,
            page: Page::Basic,
            advanced: false,
            modified: false,
            status: "準備完了".to_owned(),
            search_input,
            fields: HashMap::new(),
            picker: None,
            modal: None,
            backups: Vec::new(),
            mc_persona_selected: 0,
            party_selected: 0,
            compendium_selected: 0,
            compendium_missing_only: false,
            inventory_category: 0,
            inventory_owned_only: false,
            inventory_selected: None,
            copy_slot: 1,
        };
        if !app.slots.is_empty() {
            app.load_slot(0, cx);
        } else {
            app.status = "セーブデータが見つかりません。右上の「フォルダ」から選択してください。"
                .to_owned();
        }
        cx.notify();
        app
    }

    fn load_slot(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(path) = self.slots.get(index).cloned() else {
            self.status = "スロットがありません".to_owned();
            return;
        };
        match P4GSave::load(&path) {
            Ok(save) => {
                self.backups = save.list_backups();
                self.save = Some(save);
                self.selected_slot = index;
                self.modified = false;
                self.fields.clear();
                self.picker = None;
                self.modal = None;
                self.mc_persona_selected = 0;
                self.party_selected = 0;
                self.compendium_selected = 0;
                self.inventory_selected = None;
                let name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("セーブ");
                self.status = format!("読み込みました: {name}");
                self.clear_search(cx);
                self.refresh_slot_labels();
                cx.notify();
            }
            Err(error) => {
                self.status = format!("読み込みに失敗しました: {error}");
                cx.notify();
            }
        }
    }

    fn refresh_slot_labels(&mut self) {
        self.slot_labels = self
            .slots
            .iter()
            .map(|path| save::slot_summary_from_path(path))
            .collect();
    }

    fn has_pending_changes(&self, cx: &App) -> bool {
        self.modified
            || self.fields.iter().any(|(field, entry)| {
                *field != Field::CopySlot
                    && entry.input.read(cx).content() != entry.initial.as_str()
            })
    }

    fn request_slot(&mut self, index: usize, cx: &mut Context<Self>) {
        if index == self.selected_slot {
            return;
        }
        if self.has_pending_changes(cx) {
            self.modal = Some(Modal::SwitchSlot(index));
            cx.notify();
        } else {
            self.load_slot(index, cx);
        }
    }

    fn change_slot(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.slots.is_empty() {
            return;
        }
        let len = self.slots.len() as isize;
        let next = (self.selected_slot as isize + delta).rem_euclid(len) as usize;
        self.request_slot(next, cx);
    }

    fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search_input
            .update(cx, |input, cx| input.set_content("", cx));
    }

    fn search_query(&self, cx: &App) -> String {
        self.search_input.read(cx).content().trim().to_lowercase()
    }

    fn field(&mut self, field: Field, value: impl ToString, cx: &mut Context<Self>) -> Entity<TextInput> {
        if let Some(entry) = self.fields.get(&field) {
            return entry.input.clone();
        }
        let text = value.to_string();
        let input = cx.new(|cx| {
            let mut input = TextInput::new("", cx);
            input.set_content(text.clone(), cx);
            input
        });
        let sub = cx.observe(&input, |_, _, cx| cx.notify());
        self.fields.insert(
            field,
            FieldEntry {
                input: input.clone(),
                initial: text,
                error: false,
                _sub: sub,
            },
        );
        input
    }

    fn input_box(
        &mut self,
        field: Field,
        value: impl ToString,
        width_rems: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let input = self.field(field, value, cx);
        let error = self.fields.get(&field).map(|entry| entry.error).unwrap_or(false);
        div()
            .w(rems(width_rems))
            .h(rems(1.85))
            .flex_none()
            .px(rems(0.2))
            .flex()
            .items_center()
            .rounded_sm()
            .border_1()
            .border_color(if error { rgb(RED) } else { rgb(BORDER) })
            .bg(rgb(INPUT_BG))
            .child(input)
    }

    fn set_field(&mut self, field: Field, value: impl ToString, cx: &mut Context<Self>) {
        let value = value.to_string();
        if let Some(entry) = self.fields.get_mut(&field) {
            entry.initial = value.clone();
            entry.error = false;
            entry
                .input
                .update(cx, |input, cx| input.set_content(value, cx));
        }
    }

    fn remove_fields(&mut self, predicate: impl Fn(&Field) -> bool) {
        self.fields.retain(|field, _| !predicate(field));
    }

    fn remove_persona_fields(&mut self, target: PersonaTarget) {
        self.remove_fields(|field| match field {
            Field::PersonaLevel(value) | Field::PersonaXp(value) => *value == target,
            Field::PersonaStat(value, _) => *value == target,
            _ => false,
        });
    }

    fn commit_fields(&mut self, cx: &mut Context<Self>) -> Result<bool, String> {
        let mut jobs = Vec::new();
        for (field, entry) in &self.fields {
            let text = entry.input.read(cx).content().trim().to_owned();
            if text != entry.initial {
                jobs.push((*field, text));
            }
        }
        jobs.sort_by_key(|(field, _)| field_priority(*field));

        let mut errors = Vec::new();
        let mut applied = false;
        for (field, text) in jobs {
            match self.apply_field(field, &text) {
                Ok(()) => {
                    if let Some(entry) = self.fields.get_mut(&field) {
                        entry.initial = text;
                        entry.error = false;
                    }
                    if field != Field::CopySlot {
                        applied = true;
                    }
                }
                Err(message) => {
                    if let Some(entry) = self.fields.get_mut(&field) {
                        entry.error = true;
                    }
                    errors.push(format!("{}: {message}", field_label(field)));
                }
            }
        }
        if errors.is_empty() {
            Ok(applied)
        } else {
            Err(errors.join(" / "))
        }
    }

    fn apply_field(&mut self, field: Field, text: &str) -> Result<(), String> {
        let range = |value: i64, min: i64, max: i64| -> Result<i64, String> {
            if value < min || value > max {
                Err(format!("{min} から {max} の範囲で入力してください"))
            } else {
                Ok(value)
            }
        };
        let parse = |min: i64, max: i64| -> Result<i64, String> {
            let value = text
                .parse::<i64>()
                .map_err(|_| "数値を入力してください".to_owned())?;
            range(value, min, max)
        };

        let save = self
            .save
            .as_mut()
            .ok_or_else(|| "セーブがありません".to_owned())?;
        match field {
            Field::LastName => {
                save.set_lname(text).map_err(|error| error.to_string())?;
            }
            Field::FirstName => {
                save.set_fname(text).map_err(|error| error.to_string())?;
            }
            Field::Money => save.set_money(parse(0, save::MAX_MONEY as i64)? as u32),
            Field::McLevel => save.set_mc_level(parse(1, 99)? as u16),
            Field::McXp => save.set_mc_xp(parse(0, 9_999_999)? as u32),
            Field::McHp => save.set_mc_hp(parse(0, 9999)? as u16),
            Field::McSp => save.set_mc_sp(parse(0, 9999)? as u16),
            Field::Playtime => {
                let (hours, minutes, seconds) = parse_playtime(text)?;
                save.set_playtime(hours, minutes, seconds);
            }
            Field::RiseLines => save.set_rise_lines(parse(0, 255)? as u8),
            Field::SocialStat(index) => {
                let value = parse(0, 999)? as u16;
                let mut stats = save.social_stats();
                if let Some(stat) = stats.get_mut(index) {
                    *stat = value;
                }
                save.set_social_stats(stats);
            }
            Field::DayDate => {
                let date = parse_date(text)?;
                let serial = (date - save::EPOCH).num_days().clamp(0, save::MAX_DAY as i64) as u16;
                save.set_day_serial(serial);
            }
            Field::NextDate => {
                let date = parse_date(text)?;
                let serial = (date - save::EPOCH).num_days().clamp(0, save::MAX_DAY as i64) as u16;
                save.set_next_day(serial);
            }
            Field::DaySerial => save.set_day_serial(parse(0, save::MAX_DAY as i64)? as u16),
            Field::NextDay => save.set_next_day(parse(0, save::MAX_DAY as i64)? as u16),
            Field::UnitHp(index) => {
                let value = parse(0, 9999)? as u16;
                if index == 0 {
                    save.set_mc_hp(value);
                } else if let Some(unit) = save.units.get_mut(index) {
                    unit.set_hp(value);
                } else {
                    return Err("メンバーがいません".to_owned());
                }
            }
            Field::UnitSp(index) => {
                let value = parse(0, 9999)? as u16;
                if index == 0 {
                    save.set_mc_sp(value);
                } else if let Some(unit) = save.units.get_mut(index) {
                    unit.set_sp(value);
                } else {
                    return Err("メンバーがいません".to_owned());
                }
            }
            Field::PersonaLevel(target) => {
                let value = parse(1, 99)? as u16;
                let mut persona = self
                    .persona(target)
                    .ok_or_else(|| "ペルソナがありません".to_owned())?;
                if persona.id() == 0 {
                    return Err("ペルソナがありません".to_owned());
                }
                persona.set_level(value);
                self.set_persona(target, persona);
            }
            Field::PersonaXp(target) => {
                let value = parse(0, 9_999_999)? as u32;
                let mut persona = self
                    .persona(target)
                    .ok_or_else(|| "ペルソナがありません".to_owned())?;
                if persona.id() == 0 {
                    return Err("ペルソナがありません".to_owned());
                }
                persona.set_xp(value);
                self.set_persona(target, persona);
            }
            Field::PersonaStat(target, index) => {
                let value = parse(1, 99)? as u8;
                let mut persona = self
                    .persona(target)
                    .ok_or_else(|| "ペルソナがありません".to_owned())?;
                if persona.id() == 0 {
                    return Err("ペルソナがありません".to_owned());
                }
                let mut stats = persona.stats();
                if let Some(stat) = stats.get_mut(index) {
                    *stat = value;
                }
                persona.set_stats(&stats);
                self.set_persona(target, persona);
            }
            Field::SocialRank(index) => {
                let value = parse(0, 10)? as u16;
                if let Some(link) = save.social_links.get_mut(index) {
                    if link.id() == 0 {
                        return Err("未使用のリンクです".to_owned());
                    }
                    link.set_rank(value);
                }
            }
            Field::SocialAffinity(index) => {
                let value = parse(0, 999)? as u16;
                if let Some(link) = save.social_links.get_mut(index) {
                    if link.id() == 0 {
                        return Err("未使用のリンクです".to_owned());
                    }
                    link.set_affinity(value);
                }
            }
            Field::ItemCount(item_id) => save.set_item_count(item_id, parse(0, 99)? as u8),
            Field::CopySlot => {
                self.copy_slot = parse(1, 16)? as u16;
            }
        }
        Ok(())
    }

    fn persona(&self, target: PersonaTarget) -> Option<save::Persona> {
        let save = self.save.as_ref()?;
        match target {
            PersonaTarget::Mc(index) => save.mc_personas.get(index).copied(),
            PersonaTarget::Party(index) => save.units.get(index).map(|unit| unit.persona()),
            PersonaTarget::Compendium(index) => save.compendium.get(index).copied(),
        }
    }

    fn set_persona(&mut self, target: PersonaTarget, persona: save::Persona) {
        let Some(save) = self.save.as_mut() else {
            return;
        };
        match target {
            PersonaTarget::Mc(index) => {
                if let Some(entry) = save.mc_personas.get_mut(index) {
                    *entry = persona;
                }
            }
            PersonaTarget::Party(index) => {
                if let Some(unit) = save.units.get_mut(index) {
                    unit.set_persona(&persona);
                }
            }
            PersonaTarget::Compendium(index) => {
                if let Some(entry) = save.compendium.get_mut(index) {
                    *entry = persona;
                }
            }
        }
    }

    fn toggle_compendium(&mut self, index: usize) {
        let Some(save) = self.save.as_mut() else {
            return;
        };
        let Some(entry) = save.compendium.get_mut(index) else {
            return;
        };
        if entry.exists() {
            entry.clear();
        } else if let Some(info) = self.game.persona(index as u16 + 1) {
            entry.fill_base(info);
        }
        self.modified = true;
        self.remove_persona_fields(PersonaTarget::Compendium(index));
    }

    fn select_picker(&mut self, id: u16, cx: &mut Context<Self>) {
        let Some(picker) = self.picker else {
            return;
        };
        match picker.target {
            PickerTarget::Persona(target) => {
                let mut persona = self.persona(target).unwrap_or(save::Persona { raw: [0; 48] });
                if let Some(info) = self.game.persona(id) {
                    let name = self.game.persona_name(id);
                    persona.fill_base(info);
                    self.set_persona(target, persona);
                    self.remove_persona_fields(target);
                    self.modified = true;
                    self.status = format!("ペルソナを設定: {name}");
                }
            }
            PickerTarget::Skill(target, skill_index) => {
                let Some(mut persona) = self.persona(target) else {
                    return;
                };
                let mut skills = persona.skills();
                if let Some(skill) = skills.get_mut(skill_index) {
                    *skill = id;
                }
                persona.set_skills(&skills);
                self.set_persona(target, persona);
                self.modified = true;
                self.status = format!("スキルを設定: {}", self.game.skill_name(id));
            }
            PickerTarget::Item(ItemTarget::Inventory) => {
                if let Some(save) = self.save.as_mut() {
                    if save.item_count(id) == 0 {
                        save.set_item_count(id, 1);
                    }
                    self.inventory_selected = Some(id);
                    self.remove_fields(|field| *field == Field::ItemCount(id));
                    self.modified = true;
                }
            }
            PickerTarget::Item(ItemTarget::Equip(unit_index, slot)) => {
                if let Some(save) = self.save.as_mut() {
                    if unit_index == 0 {
                        save.set_mc_equipment(slot, id);
                    } else if let Some(unit) = save.units.get_mut(unit_index) {
                        unit.set_equipment(slot, id);
                    }
                    self.modified = true;
                }
            }
        }
        self.picker = None;
        self.clear_search(cx);
        cx.notify();
    }

    fn open_picker(
        &mut self,
        target: PickerTarget,
        category: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.picker = Some(PickerState { target, category });
        self.clear_search(cx);
        window.focus(&self.search_input.read(cx).focus_handle(cx));
        cx.notify();
    }

    fn save_current(&mut self, cx: &mut Context<Self>) {
        match self.commit_fields(cx) {
            Ok(applied) => {
                if applied {
                    self.modified = true;
                }
            }
            Err(message) => {
                self.status = format!("入力を確認してください: {message}");
                cx.notify();
                return;
            }
        }
        let result = self.save.as_mut().map(|save| save.save(true));
        match result {
            Some(Ok(backup)) => {
                self.modified = false;
                self.fields.clear();
                self.status = match backup {
                    Some(_) => "保存しました (バックアップ作成)".to_owned(),
                    None => "保存しました".to_owned(),
                };
                if let Some(save) = self.save.as_ref() {
                    self.backups = save.list_backups();
                }
                self.refresh_slot_labels();
            }
            Some(Err(error)) => {
                self.status = format!("保存に失敗しました: {error}");
            }
            None => {
                self.status = "セーブがありません".to_owned();
            }
        }
        cx.notify();
    }

    fn reload_current(&mut self, cx: &mut Context<Self>) {
        let index = self.selected_slot;
        self.load_slot(index, cx);
    }

    fn restore_backup(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(path) = self.backups.get(index).cloned() else {
            self.status = "バックアップがありません".to_owned();
            return;
        };
        let Some(save) = self.save.as_mut() else {
            return;
        };
        match save.restore_backup(&path) {
            Ok(()) => {
                self.modal = None;
                self.load_slot(self.selected_slot, cx);
                self.status = "バックアップから復元しました".to_owned();
            }
            Err(error) => {
                self.status = format!("復元に失敗しました: {error}");
                cx.notify();
            }
        }
    }

    fn export_json(&mut self, cx: &mut Context<Self>) {
        let Some(save) = self.save.as_ref() else {
            self.status = "セーブがありません".to_owned();
            return;
        };
        let default_name = format!("p4g_slot{:04}.json", save.slot_number());
        let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_file_name(default_name)
            .save_file()
        else {
            return;
        };
        let value = save.dump_json(&self.game);
        match serde_json::to_string_pretty(&value)
            .map_err(|error| error.to_string())
            .and_then(|text| std::fs::write(&path, text).map_err(|error| error.to_string()))
        {
            Ok(()) => {
                self.status = "JSON を書き出しました".to_owned();
            }
            Err(error) => {
                self.status = format!("JSON の書き出しに失敗しました: {error}");
            }
        }
        cx.notify();
    }

    fn open_save_folder(&mut self) {
        let Some(save) = self.save.as_ref() else {
            return;
        };
        let folder = save.bin_path.parent().unwrap_or(std::path::Path::new("."));
        let _ = Command::new("explorer").arg(folder).spawn();
    }

    fn open_folder(&mut self, cx: &mut Context<Self>) {
        let Some(folder) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        let mut slots = save::list_slots(&folder);
        if slots.is_empty() {
            let remote = folder.join("remote");
            if remote.is_dir() {
                slots = save::list_slots(&remote);
            }
        }
        if slots.is_empty() {
            self.status = "data*.bin が見つかりません".to_owned();
            cx.notify();
            return;
        }
        self.slots = slots;
        self.refresh_slot_labels();
        self.load_slot(0, cx);
        self.status = "フォルダを読み込みました".to_owned();
        cx.notify();
    }

    fn copy_current_to_slot(&mut self, cx: &mut Context<Self>) {
        let target = self
            .fields
            .get(&Field::CopySlot)
            .and_then(|entry| entry.input.read(cx).content().trim().parse::<u16>().ok())
            .unwrap_or(self.copy_slot);
        if !(1..=16).contains(&target) {
            self.status = "複製先は 1 から 16 で指定してください".to_owned();
            cx.notify();
            return;
        }
        let Some(save) = self.save.as_mut() else {
            return;
        };
        let Some(parent) = save.bin_path.parent() else {
            return;
        };
        if target == save.slot_number() {
            self.status = "複製先が現在のスロットと同じです".to_owned();
            cx.notify();
            return;
        }
        let target_bin = parent.join(format!("data{target:04}.bin"));
        match save.copy_to(target_bin, None, true) {
            Ok(()) => {
                self.copy_slot = target;
                self.set_field(Field::CopySlot, target, cx);
                self.status = format!("data{target:04}.bin に複製しました");
                self.refresh_slot_labels();
                if let Some(save) = self.save.as_ref() {
                    self.backups = save.list_backups();
                }
            }
            Err(error) => {
                self.status = format!("複製に失敗しました: {error}");
            }
        }
        cx.notify();
    }

    fn slot_label(&self) -> String {
        if let Some(save) = self.save.as_ref() {
            let info = save.slot_info();
            return format!(
                "data{:04}  {} Lv{}  {} {}",
                save.slot_number(),
                info.name,
                info.level,
                info.date_text,
                info.phase_text
            );
        }
        self.slot_labels
            .get(self.selected_slot)
            .cloned()
            .unwrap_or_else(|| "セーブ未検出".to_owned())
    }

    fn checksum_text(&self) -> String {
        let Some(save) = self.save.as_ref() else {
            return "セーブ未検出".to_owned();
        };
        let checks = save.check_loaded();
        checksum_line(&checks)
    }

    fn compendium_entries(&self, cx: &App) -> Vec<(u16, String)> {
        let Some(save) = self.save.as_ref() else {
            return Vec::new();
        };
        let query = self.search_query(cx);
        self.game
            .personas
            .iter()
            .filter(|info| {
                let entry = &save.compendium[info.id as usize - 1];
                if self.compendium_missing_only && entry.exists() {
                    return false;
                }
                if query.is_empty() {
                    return true;
                }
                let jp = self.game.persona_name(info.id).to_lowercase();
                info.name.to_lowercase().contains(&query)
                    || jp.contains(&query)
                    || format!("{:03}", info.id).contains(&query)
            })
            .map(|info| {
                let entry = &save.compendium[info.id as usize - 1];
                let state = if entry.exists() {
                    format!("Lv{}", entry.level())
                } else {
                    "未登録".to_owned()
                };
                (
                    info.id,
                    format!(
                        "{:03}  {}  [{}]",
                        info.id,
                        self.game.persona_name(info.id),
                        state
                    ),
                )
            })
            .collect()
    }

    fn inventory_entries(&self, cx: &App) -> Vec<(u16, String)> {
        let Some(save) = self.save.as_ref() else {
            return Vec::new();
        };
        let query = self.search_query(cx);
        let categories = game::category_options();
        let category = categories
            .get(self.inventory_category)
            .map(|(key, _)| *key)
            .unwrap_or("");
        let mut result = Vec::new();
        for item in &self.game.items {
            if item.id == 0 || item.tag == "FLR" {
                continue;
            }
            if !category.is_empty() && item.category_key() != category {
                continue;
            }
            let count = save.item_count(item.id);
            if self.inventory_owned_only && count == 0 {
                continue;
            }
            let jp = self.game.item_name(item.id);
            if !self.advanced && jp == "未使用" && count == 0 {
                continue;
            }
            if !query.is_empty()
                && !item.name.to_lowercase().contains(&query)
                && !jp.to_lowercase().contains(&query)
                && !format!("{:04}", item.id).contains(&query)
            {
                continue;
            }
            result.push((
                item.id,
                format!(
                    "{:04}  {}  [{}]",
                    item.id,
                    jp,
                    item.category_jp()
                ),
            ));
        }
        result
    }

    fn picker_entries(&self, cx: &App) -> Vec<(u16, String)> {
        let Some(picker) = self.picker else {
            return Vec::new();
        };
        let query = self.search_query(cx);
        let mut result = Vec::new();
        match picker.target {
            PickerTarget::Persona(_) => {
                for info in &self.game.personas {
                    let jp = self.game.persona_name(info.id);
                    if !self.advanced && jp == "未使用" {
                        continue;
                    }
                    if query.is_empty()
                        || info.name.to_lowercase().contains(&query)
                        || jp.to_lowercase().contains(&query)
                        || format!("{:03}", info.id).contains(&query)
                    {
                        result.push((
                            info.id,
                            format!("{:03}  {}  Lv{}", info.id, jp, info.level),
                        ));
                    }
                }
            }
            PickerTarget::Skill(_, _) => {
                for skill in &self.game.skills {
                    if !self.game.has_jp_skill(skill.id) {
                        continue;
                    }
                    let jp = self.game.skill_name(skill.id);
                    if query.is_empty()
                        || skill.name.to_lowercase().contains(&query)
                        || jp.to_lowercase().contains(&query)
                        || format!("{:03}", skill.id).contains(&query)
                    {
                        result.push((skill.id, format!("{:03}  {}", skill.id, jp)));
                    }
                }
            }
            PickerTarget::Item(_) => {
                let categories = game::category_options();
                let category = categories
                    .get(picker.category)
                    .map(|(key, _)| *key)
                    .unwrap_or("");
                for item in &self.game.items {
                    if item.id == 0 || item.tag == "FLR" {
                        continue;
                    }
                    if !category.is_empty() && item.category_key() != category {
                        continue;
                    }
                    let jp = self.game.item_name(item.id);
                    if !self.advanced && jp == "未使用" {
                        continue;
                    }
                    if query.is_empty()
                        || item.name.to_lowercase().contains(&query)
                        || jp.to_lowercase().contains(&query)
                        || format!("{:04}", item.id).contains(&query)
                    {
                        result.push((
                            item.id,
                            format!("{:04}  {}  [{}]", item.id, jp, item.category_jp()),
                        ));
                    }
                }
            }
        }
        result
    }

    fn page_content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        if self.picker.is_some() {
            return self.picker_content(window, cx);
        }
        match self.page {
            Page::Basic => self.basic_page(cx),
            Page::Date => self.date_page(cx),
            Page::Party => self.party_page(window, cx),
            Page::Personas => self.personas_page(window, cx),
            Page::Compendium => self.compendium_page(window, cx),
            Page::Social => self.social_page(cx),
            Page::Inventory => self.inventory_page(cx),
            Page::Tools => self.tools_page(cx),
        }
    }

    fn basic_page(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some((
            lname,
            fname,
            money,
            level,
            xp,
            hp,
            sp,
            hours,
            minutes,
            seconds,
            difficulty,
            stats,
            party,
            rise,
            links_max,
            compendium,
            persona_total,
        )) = self.save.as_ref().map(|save| {
            let (hours, minutes, seconds) = save.playtime_hms();
            (
                save.lname(),
                save.fname(),
                save.money(),
                save.mc_level(),
                save.mc_xp(),
                save.mc_hp(),
                save.mc_sp(),
                hours,
                minutes,
                seconds,
                save.difficulty(),
                save.social_stats(),
                save.party(),
                save.rise_lines(),
                save.social_link_maxed(),
                save.compendium_count(&self.game),
                self.game.personas.len(),
            )
        }) else {
            return empty_page("セーブデータがありません");
        };

        let mut content = div().flex().flex_col().gap_3().max_w(rems(72.0));
        content = content.child(page_header("基本"));
        content = content.child(hint(format!(
            "コミュ MAX {links_max} / 23 ・ 全書 {compendium} / {persona_total}"
        )));

        let mut name_row = div().flex().flex_wrap().gap_3();
        name_row = name_row
            .child(form_row(
                "姓",
                self.input_box(Field::LastName, lname, 10.0, cx),
            ))
            .child(form_row(
                "名",
                self.input_box(Field::FirstName, fname, 10.0, cx),
            ));

        let mut values = div().flex().flex_wrap().items_center().gap_3();
        values = values
            .child(form_row(
                "所持金",
                self.input_box(Field::Money, money, 8.0, cx),
            ))
            .child(form_row(
                "レベル",
                self.input_box(Field::McLevel, level, 5.0, cx),
            ))
            .child(form_row(
                "累計 XP",
                self.input_box(Field::McXp, xp, 9.0, cx),
            ))
            .child(form_row("HP", self.input_box(Field::McHp, hp, 5.5, cx)))
            .child(form_row("SP", self.input_box(Field::McSp, sp, 5.5, cx)));

        let mut quick = div().flex().flex_wrap().gap_2();
        quick = quick
            .child(button(
                "money-max",
                "所持金 MAX",
                false,
                cx.listener(|this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        save.set_money(save::MAX_MONEY);
                        this.modified = true;
                        this.set_field(Field::Money, save::MAX_MONEY, cx);
                    }
                    cx.notify();
                }),
            ))
            .child(button(
                "level-99",
                "Lv99",
                false,
                cx.listener(|this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        save.set_mc_level(99);
                        save.set_mc_xp(3_902_496);
                        this.modified = true;
                        this.set_field(Field::McLevel, 99, cx);
                        this.set_field(Field::McXp, 3_902_496, cx);
                    }
                    cx.notify();
                }),
            ))
            .child(button(
                "hp-sp-9999",
                "HP/SP 9999",
                false,
                cx.listener(|this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        save.set_mc_hp(9999);
                        save.set_mc_sp(9999);
                        this.modified = true;
                        this.set_field(Field::McHp, 9999, cx);
                        this.set_field(Field::McSp, 9999, cx);
                    }
                    cx.notify();
                }),
            ));

        content = content.child(section(
            "プロフィール",
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(name_row)
                .child(values)
                .child(quick),
        ));

        let playtime = format_playtime(hours, minutes, seconds);
        let mut play_row = div().flex().flex_wrap().items_center().gap_3();
        play_row = play_row
            .child(form_row(
                "プレイ時間",
                self.input_box(Field::Playtime, playtime, 9.0, cx),
            ))
            .child(hint("H:MM:SS"))
            .child(form_row("難易度", phase_text(game::difficulty_jp(difficulty))));
        play_row = play_row.child(button(
            "difficulty-cycle",
            "変更",
            false,
            cx.listener(|this, _, _, cx| {
                if let Some(save) = this.save.as_mut() {
                    save.set_difficulty((save.difficulty() + 1) % 5);
                    this.modified = true;
                }
                cx.notify();
            }),
        ));
        content = content.child(section("プレイ時間と難易度", play_row));

        let mut stats_row = div().flex().flex_wrap().items_center().gap_3();
        for (index, value) in stats.iter().enumerate() {
            stats_row = stats_row.child(form_row(
                game::SOCIAL_STAT_NAMES[index].1,
                self.input_box(Field::SocialStat(index), value, 5.5, cx),
            ));
        }
        let mut stats_section = div().flex().flex_col().gap_3().child(stats_row);
        stats_section = stats_section.child(button(
            "stats-max",
            "すべて MAX",
            false,
            cx.listener(|this, _, _, _cx| {
                if let Some(save) = this.save.as_mut() {
                    save.max_social_stats();
                    this.modified = true;
                }
                this.remove_fields(|field| matches!(field, Field::SocialStat(_)));
                this.status = "社交ステータスを最大にしました".to_owned();
            }),
        ));
        content = content.child(section("社交ステータス", stats_section));

        let mut party_row = div().flex().gap_3();
        for index in 0..3 {
            let value = party[index];
            let mut member = div().flex().flex_col().gap_1();
            member = member.child(hint(format!("枠 {}", index + 1)));
            member = member.child(button(
                ("party", index),
                self.game.char_name(value),
                false,
                cx.listener(move |this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        let mut party = save.party();
                        party[index] = (party[index] + 1) % 9;
                        save.set_party(party);
                        this.modified = true;
                    }
                    cx.notify();
                }),
            ));
            party_row = party_row.child(member);
        }
        content = content.child(section("現在のパーティ", party_row));

        let rise_row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_3()
            .child(form_row(
                "リセのナビ回数",
                self.input_box(Field::RiseLines, rise, 6.0, cx),
            ))
            .child(hint("0 - 255"));
        content = content.child(section("実績カウンタ", rise_row));

        page_scroll("basic-scroll").child(content)
    }

    fn date_page(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some((serial, date, phase, next_day, next_phase, next_date)) =
            self.save.as_ref().map(|save| {
                let serial = save.day_serial();
                let phase = save.phase();
                let next_day = save.next_day();
                let next_phase = save.next_phase();
                (
                    serial,
                    save.game_date(),
                    phase,
                    next_day,
                    next_phase,
                    save::EPOCH + chrono::Duration::days(next_day as i64),
                )
            })
        else {
            return empty_page("セーブデータがありません");
        };

        let mut content = div().flex().flex_col().gap_3().max_w(rems(72.0));
        content = content.child(page_header("日付"));

        let mut current = div().flex().flex_wrap().items_center().gap_3();
        current = current
            .child(form_row(
                "日付",
                self.input_box(
                    Field::DayDate,
                    date.format("%Y/%m/%d").to_string(),
                    9.0,
                    cx,
                ),
            ))
            .child(form_row("時間帯", phase_text(game::phase_jp(phase))))
            .child(button(
                "phase-cycle",
                "次へ",
                false,
                cx.listener(|this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        save.set_phase((save.phase() + 1) % 6);
                        this.modified = true;
                    }
                    cx.notify();
                }),
            ));
        content = content.child(section("現在のゲーム内日付", current));

        let mut next = div().flex().flex_wrap().items_center().gap_3();
        next = next
            .child(form_row(
                "日付",
                self.input_box(
                    Field::NextDate,
                    next_date.format("%Y/%m/%d").to_string(),
                    9.0,
                    cx,
                ),
            ))
            .child(form_row(
                "時間帯",
                phase_text(game::phase_jp(next_phase)),
            ))
            .child(button(
                "next-phase-cycle",
                "次へ",
                false,
                cx.listener(|this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        save.set_next_phase((save.next_phase() + 1) % 6);
                        this.modified = true;
                    }
                    cx.notify();
                }),
            ))
            .child(button(
                "sync-next",
                "現在に合わせる",
                false,
                cx.listener(|this, _, _, cx| {
                    let date_text = if let Some(save) = this.save.as_mut() {
                        save.set_next_day(save.day_serial());
                        save.set_next_phase(save.phase());
                        this.modified = true;
                        Some(save.game_date().format("%Y/%m/%d").to_string())
                    } else {
                        None
                    };
                    if let Some(text) = date_text {
                        this.set_field(Field::NextDate, text, cx);
                    }
                    cx.notify();
                }),
            ));
        content = content.child(section(
            "次の日付",
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(hint("セーブを読み込んだ直後に進む日付です。"))
                .child(next),
        ));

        if self.advanced {
            let mut raw = div().flex().flex_wrap().items_center().gap_3();
            raw = raw
                .child(form_row(
                    "現在の内部値",
                    self.input_box(Field::DaySerial, serial, 6.0, cx),
                ))
                .child(form_row(
                    "次の内部値",
                    self.input_box(Field::NextDay, next_day, 6.0, cx),
                ));
            content = content.child(section("内部値 (詳細モード)", raw));
        }

        content = content.child(hint(
            "日付は物語の進行に影響します。変更前にバックアップを取ってください。",
        ));
        page_scroll("date-scroll").child(content)
    }

    fn party_page(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let selected = self.party_selected;
        let Some((unit_ids, hp_values, sp_values, selected_id, selected_hp, selected_sp, equipment)) =
            self.save.as_ref().map(|save| {
                let unit_ids = save.units.iter().map(|unit| unit.id()).collect::<Vec<_>>();
                let hp_values = (0..save.units.len())
                    .map(|index| save.unit_hp(index))
                    .collect::<Vec<_>>();
                let sp_values = (0..save.units.len())
                    .map(|index| save.unit_sp(index))
                    .collect::<Vec<_>>();
                let selected = selected.min(save.units.len().saturating_sub(1));
                let selected_id = save.units.get(selected).map(|unit| unit.id()).unwrap_or(0);
                let selected_hp = hp_values.get(selected).copied().unwrap_or(0);
                let selected_sp = sp_values.get(selected).copied().unwrap_or(0);
                let equipment = (0..4)
                    .map(|slot| {
                        let item_id = if selected == 0 {
                            save.mc_equipment(slot)
                        } else {
                            save.units
                                .get(selected)
                                .map(|unit| unit.equipment(slot))
                                .unwrap_or(0)
                        };
                        (item_id, self.game.item_name(item_id))
                    })
                    .collect::<Vec<_>>();
                (
                    unit_ids,
                    hp_values,
                    sp_values,
                    selected_id,
                    selected_hp,
                    selected_sp,
                    equipment,
                )
            })
        else {
            return empty_page("セーブデータがありません");
        };

        let mut left = div().flex().flex_col().gap_1().child(section_title("メンバー"));
        for (index, unit_id) in unit_ids.iter().enumerate() {
            let is_selected = index == selected;
            let mut member = div()
                .id(("party-member", index))
                .px_2()
                .py_1()
                .rounded_sm()
                .cursor_pointer()
                .bg(if is_selected { rgb(PANEL_2) } else { rgb(PANEL) })
                .text_color(if is_selected { rgb(ACCENT) } else { rgb(TEXT) })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.party_selected = index;
                    cx.notify();
                }));
            member = member.child(format!(
                "{}   HP {} / SP {}",
                self.game.char_name(*unit_id),
                hp_values[index],
                sp_values[index]
            ));
            left = left.child(member);
        }

        let mut right = div().flex().flex_col().gap_3();
        right = right.child(
            div()
                .text_lg()
                .text_color(rgb(ACCENT))
                .child(self.game.char_name(selected_id)),
        );
        if selected == 0 {
            right = right.child(hint("HP/SP は「基本」ページで編集します。"));
        } else {
            let mut hp_row = div().flex().flex_wrap().gap_3();
            hp_row = hp_row
                .child(form_row(
                    "HP",
                    self.input_box(Field::UnitHp(selected), selected_hp, 6.0, cx),
                ))
                .child(form_row(
                    "SP",
                    self.input_box(Field::UnitSp(selected), selected_sp, 6.0, cx),
                ));
            right = right.child(hp_row);
        }
        let mut party_actions = div().flex().flex_wrap().gap_2();
        party_actions = party_actions
            .child(button(
                "clear-status",
                "状態異常を解除",
                false,
                cx.listener(move |this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        if let Some(unit) = save.units.get_mut(selected) {
                            unit.clear_status();
                            this.modified = true;
                        }
                    }
                    cx.notify();
                }),
            ))
            .child(button(
                "heal-party",
                "全員回復",
                false,
                cx.listener(|this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        save.heal_party();
                        this.modified = true;
                    }
                    this.remove_fields(|field| {
                        matches!(
                            field,
                            Field::UnitHp(_) | Field::UnitSp(_) | Field::McHp | Field::McSp
                        )
                    });
                    this.status = "全員の HP/SP を回復しました".to_owned();
                    cx.notify();
                }),
            ));
        right = right.child(party_actions);

        if selected == 0 {
            right = right.child(button(
                "goto-personas",
                "ペルソナタブへ",
                false,
                cx.listener(|this, _, _, cx| {
                    this.page = Page::Personas;
                    cx.notify();
                }),
            ));
        } else {
            right = right.child(section_title("ペルソナ"));
            right = right.child(self.persona_editor(PersonaTarget::Party(selected), cx));
        }

        right = right.child(section_title("装備"));
        for (slot, slot_name) in ["武器", "防具", "アクセ", "衣装"].iter().enumerate() {
            let (item_id, item_name) = equipment[slot].clone();
            let category = match slot {
                0 => "Weapon",
                1 => "Armor",
                2 => "Acc",
                _ => "Clothes",
            };
            let category_index = game::category_options()
                .iter()
                .position(|(key, _)| *key == category)
                .unwrap_or(0);
            let mut row = div().flex().items_center().gap_2();
            row = row
                .child(div().w(rems(5.0)).text_sm().text_color(rgb(MUTED)).child(*slot_name))
                .child(
                    div()
                        .w(rems(14.0))
                        .text_sm()
                        .child(if item_id == 0 {
                            "なし".to_owned()
                        } else {
                            item_name
                        }),
                )
                .child(button(
                    ("equip-pick", slot),
                    "変更",
                    false,
                    cx.listener(move |this, _, window, cx| {
                        this.open_picker(
                            PickerTarget::Item(ItemTarget::Equip(selected, slot)),
                            category_index,
                            window,
                            cx,
                        );
                    }),
                ))
                .child(button(
                    ("equip-clear", slot),
                    "なし",
                    false,
                    cx.listener(move |this, _, _, cx| {
                        if let Some(save) = this.save.as_mut() {
                            if selected == 0 {
                                save.set_mc_equipment(slot, 0);
                            } else if let Some(unit) = save.units.get_mut(selected) {
                                unit.set_equipment(slot, 0);
                            }
                            this.modified = true;
                        }
                        cx.notify();
                    }),
                ));
            right = right.child(row);
        }

        page_scroll("party-scroll").child(
            div()
                .flex()
                .gap_3()
                .child(div().w(rems(15.0)).flex_none().child(panel(left)))
                .child(div().flex_1().min_w(px(0.)).child(panel(right))),
        )
    }

    fn personas_page(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let selected = self.mc_persona_selected;
        let Some(slots) = self.save.as_ref().map(|save| {
            save.mc_personas
                .iter()
                .enumerate()
                .map(|(index, persona)| {
                    if persona.exists() {
                        format!(
                            "{:02}  {}  Lv{}",
                            index + 1,
                            self.game.persona_name(persona.id()),
                            persona.level()
                        )
                    } else {
                        format!("{:02}  空き", index + 1)
                    }
                })
                .collect::<Vec<_>>()
        }) else {
            return empty_page("セーブデータがありません");
        };

        let mut left = div().flex().flex_col().gap_1().child(section_title("所持枠 12"));
        for (index, text) in slots.iter().enumerate() {
            let is_selected = index == selected;
            left = left.child(
                div()
                    .id(("mc-persona", index))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(if is_selected { rgb(PANEL_2) } else { rgb(PANEL) })
                    .text_color(if is_selected { rgb(ACCENT) } else { rgb(TEXT) })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.mc_persona_selected = index;
                        cx.notify();
                    }))
                    .child(text.clone()),
            );
        }

        let mut right = div().flex().flex_col().gap_3();
        right = right.child(section_title("ペルソナ編集"));
        right = right.child(self.persona_editor(PersonaTarget::Mc(selected), cx));
        let mut persona_actions = div().flex().flex_wrap().gap_2();
        persona_actions = persona_actions
            .child(button(
                "clear-persona",
                "この枠を空にする",
                false,
                cx.listener(|this, _, _, cx| {
                    let index = this.mc_persona_selected;
                    if let Some(save) = this.save.as_mut() {
                        if let Some(persona) = save.mc_personas.get_mut(index) {
                            persona.clear();
                            this.modified = true;
                            this.remove_persona_fields(PersonaTarget::Mc(index));
                        }
                    }
                    cx.notify();
                }),
            ))
            .child(button(
                "all-personas-99",
                "全枠 Lv99",
                false,
                cx.listener(|this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        save.max_personas(&this.game);
                        this.modified = true;
                    }
                    this.remove_fields(|field| {
                        matches!(
                            field,
                            Field::PersonaLevel(_)
                                | Field::PersonaXp(_)
                                | Field::PersonaStat(_, _)
                        )
                    });
                    this.status = "所持ペルソナを Lv99 にしました".to_owned();
                    cx.notify();
                }),
            ));
        right = right.child(persona_actions);

        page_scroll("personas-scroll").child(
            div()
                .flex()
                .gap_3()
                .child(div().w(rems(15.0)).flex_none().child(panel(left)))
                .child(div().flex_1().min_w(px(0.)).child(panel(right))),
        )
    }

    fn compendium_page(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some((count, total)) = self.save.as_ref().map(|save| {
            (save.compendium_count(&self.game), self.game.personas.len())
        }) else {
            return empty_page("セーブデータがありません");
        };
        let ratio = if total == 0 {
            0.0
        } else {
            count as f32 / total as f32
        };
        let entries = self.compendium_entries(cx);

        let mut top = div().flex().flex_col().gap_2();
        top = top.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().text_base().text_color(rgb(ACCENT)).child(format!("{count} / {total}")))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .h(rems(0.45))
                        .rounded_sm()
                        .bg(rgb(PANEL_2))
                        .child(
                            div()
                                .h_full()
                                .w(relative(ratio.clamp(0.0, 1.0)))
                                .rounded_sm()
                                .bg(rgb(ACCENT)),
                        ),
                ),
        );
        let mut actions = div().flex().flex_wrap().gap_2();
        actions = actions
            .child(button(
                "comp-register-all",
                "未登録をすべて登録",
                true,
                cx.listener(|this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        save.fill_compendium(&this.game);
                        this.modified = true;
                    }
                    this.remove_fields(|field| {
                        matches!(
                            field,
                            Field::PersonaLevel(PersonaTarget::Compendium(_))
                                | Field::PersonaXp(PersonaTarget::Compendium(_))
                                | Field::PersonaStat(PersonaTarget::Compendium(_), _)
                        )
                    });
                    cx.notify();
                }),
            ))
            .child(button(
                "comp-reset-all",
                "全初期化",
                false,
                cx.listener(|this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        save.reset_compendium(&this.game);
                        this.modified = true;
                    }
                    this.remove_fields(|field| {
                        matches!(
                            field,
                            Field::PersonaLevel(PersonaTarget::Compendium(_))
                                | Field::PersonaXp(PersonaTarget::Compendium(_))
                                | Field::PersonaStat(PersonaTarget::Compendium(_), _)
                        )
                    });
                    cx.notify();
                }),
            ))
            .child(button(
                "comp-clear-all",
                "全解除",
                false,
                cx.listener(|this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        save.clear_compendium();
                        this.modified = true;
                    }
                    this.remove_fields(|field| {
                        matches!(
                            field,
                            Field::PersonaLevel(PersonaTarget::Compendium(_))
                                | Field::PersonaXp(PersonaTarget::Compendium(_))
                                | Field::PersonaStat(PersonaTarget::Compendium(_), _)
                        )
                    });
                    cx.notify();
                }),
            ));
        top = top.child(actions);
        top = top.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().w(rems(14.0)).child(self.search_input.clone()))
                .child(button(
                    "comp-missing",
                    if self.compendium_missing_only {
                        "未登録のみ: ON"
                    } else {
                        "未登録のみ: OFF"
                    },
                    self.compendium_missing_only,
                    cx.listener(|this, _, _, cx| {
                        this.compendium_missing_only = !this.compendium_missing_only;
                        cx.notify();
                    }),
                )),
        );

        let count = entries.len();
        let list = div()
            .flex_1()
            .min_h(px(0.))
            .child(
                uniform_list(
                    "compendium-list",
                    count,
                    cx.processor(move |this, range, _window, cx| {
                        let mut rows = Vec::new();
                        for index in range {
                            let Some((id, label)): Option<&(u16, String)> = entries.get(index) else {
                                continue;
                            };
                            let id = *id;
                            let selected = id as usize - 1 == this.compendium_selected;
                            rows.push(
                                div()
                                    .id(("comp-row", index))
                                    .h(rems(1.8))
                                    .w_full()
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .cursor_pointer()
                                    .bg(if selected { rgb(PANEL_2) } else { rgb(PANEL) })
                                    .border_b_1()
                                    .border_color(rgb(BORDER))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.compendium_selected = id as usize - 1;
                                        cx.notify();
                                    }))
                                    .child(label.clone()),
                            );
                        }
                        rows
                    }),
                )
                .h_full(),
            )
            .h_full();

        let right = self.persona_editor(
            PersonaTarget::Compendium(self.compendium_selected),
            cx,
        );
        let left = div()
            .flex()
            .flex_col()
            .gap_2()
            .w(rems(18.0))
            .flex_none()
            .child(top)
            .child(list);
        let content = div()
            .flex()
            .gap_3()
            .h_full()
            .child(left)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .min_h(px(0.))
                    .id("comp-scroll")
                    .overflow_scroll()
                    .child(panel(right)),
            );
        page_fixed("compendium-page").child(content)
    }

    fn social_page(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(links) = self.save.as_ref().map(|save| {
            save.social_links
                .iter()
                .enumerate()
                .map(|(index, link)| {
                    (
                        index,
                        link.id(),
                        link.rank(),
                        link.affinity(),
                    )
                })
                .collect::<Vec<_>>()
        }) else {
            return empty_page("セーブデータがありません");
        };

        let mut content = div().flex().flex_col().gap_1().max_w(rems(72.0));
        content = content.child(page_header("コミュ"));
        for (index, id, rank, affinity) in links {
            if id == 0 {
                content = content.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .py_1()
                        .px_2()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .child(hint(format!("{:02}  {}", index + 1, self.game.social_display(id)))),
                );
                continue;
            }
            let name = self.game.social_display(id);
            let mut row = div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .py_1()
                .px_2()
                .border_b_1()
                .border_color(rgb(BORDER));
            row = row
                .child(hint(format!("{:02}", index + 1)))
                .child(div().w(rems(13.0)).text_sm().child(name))
                .child(form_row(
                    "ランク",
                    self.input_box(Field::SocialRank(index), rank, 4.5, cx),
                ))
                .child(form_row(
                    "好感度",
                    self.input_box(Field::SocialAffinity(index), affinity, 5.0, cx),
                ))
                .child(button(
                    ("social-max", index),
                    "MAX",
                    false,
                    cx.listener(move |this, _, _, cx| {
                        if let Some(save) = this.save.as_mut() {
                            if let Some(link) = save.social_links.get_mut(index) {
                                link.set_rank(10);
                                link.set_affinity(0);
                                this.modified = true;
                                this.set_field(Field::SocialRank(index), 10, cx);
                                this.set_field(Field::SocialAffinity(index), 0, cx);
                            }
                        }
                        cx.notify();
                    }),
                ));
            content = content.child(row);
        }
        content = content.child(div().flex().child(button(
            "social-all-max",
            "すべて MAX",
            true,
            cx.listener(|this, _, _, _cx| {
                if let Some(save) = this.save.as_mut() {
                    save.max_social_links();
                    this.modified = true;
                }
                this.remove_fields(|field| {
                    matches!(field, Field::SocialRank(_) | Field::SocialAffinity(_))
                });
                this.status = "コミュを最大にしました".to_owned();
            }),
        )));
        page_scroll("social-scroll").child(content)
    }

    fn inventory_page(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(save) = self.save.as_ref() else {
            return empty_page("セーブデータがありません");
        };
        let _ = save;
        let entries = self.inventory_entries(cx);
        let categories = game::category_options();
        let category = categories
            .get(self.inventory_category)
            .map(|(_, label)| *label)
            .unwrap_or("すべて");
        let count = entries.len();

        let mut content = div().flex().flex_col().gap_2().h_full();
        content = content.child(page_header("アイテム"));
        content = content.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(div().w(rems(14.0)).child(self.search_input.clone()))
                .child(button(
                    "inv-category",
                    format!("カテゴリ: {category}"),
                    false,
                    cx.listener(|this, _, _, cx| {
                        this.inventory_category =
                            (this.inventory_category + 1) % game::category_options().len();
                        cx.notify();
                    }),
                ))
                .child(button(
                    "inv-owned",
                    if self.inventory_owned_only {
                        "所持のみ: ON"
                    } else {
                        "所持のみ: OFF"
                    },
                    self.inventory_owned_only,
                    cx.listener(|this, _, _, cx| {
                        this.inventory_owned_only = !this.inventory_owned_only;
                        cx.notify();
                    }),
                ))
                .child(button(
                    "inv-add",
                    "アイテムを追加",
                    true,
                    cx.listener(|this, _, window, cx| {
                        this.open_picker(PickerTarget::Item(ItemTarget::Inventory), 0, window, cx);
                    }),
                )),
        );
        content = content.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(button(
                    "inv-all-99",
                    "全アイテム 99",
                    false,
                    cx.listener(|this, _, _, cx| {
                        if let Some(save) = this.save.as_mut() {
                            save.add_all_items(99, false, &this.game);
                            this.modified = true;
                        }
                        this.remove_fields(|field| matches!(field, Field::ItemCount(_)));
                        cx.notify();
                    }),
                ))
                .child(button(
                    "inv-clear",
                    "全削除",
                    false,
                    cx.listener(|this, _, _, cx| {
                        if let Some(save) = this.save.as_mut() {
                            save.clear_inventory();
                            this.modified = true;
                        }
                        this.remove_fields(|field| matches!(field, Field::ItemCount(_)));
                        cx.notify();
                    }),
                ))
                .child(hint(format!("{count} 件"))),
        );

        content = content.child(
            div()
                .flex_1()
                .min_h(px(0.))
                .child(
                    uniform_list(
                        "inventory-list",
                        count,
                        cx.processor(move |this, range, _window, cx| {
                            let mut rows = Vec::new();
                            for index in range {
                                let Some((id, label)): Option<&(u16, String)> = entries.get(index)
                                else {
                                    continue;
                                };
                                let id = *id;
                                let selected = this.inventory_selected == Some(id);
                                let item_count = this
                                    .save
                                    .as_ref()
                                    .map(|save| save.item_count(id))
                                    .unwrap_or(0);
                                let box_div =
                                    this.input_box(Field::ItemCount(id), item_count, 4.5, cx);
                                rows.push(
                                    div()
                                        .id(("inv-row", index))
                                        .h(rems(1.9))
                                        .w_full()
                                        .px_2()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .cursor_pointer()
                                        .bg(if selected { rgb(PANEL_2) } else { rgb(PANEL) })
                                        .border_b_1()
                                        .border_color(rgb(BORDER))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.inventory_selected = Some(id);
                                            cx.notify();
                                        }))
                                        .child(div().flex_1().min_w(px(0.)).child(label.clone()))
                                        .child(box_div),
                                );
                            }
                            rows
                        }),
                    )
                    .h_full(),
                )
                .h_full(),
        );
        page_fixed("inventory-page").child(content)
    }

    fn tools_page(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let (path, checksum) = match self.save.as_ref() {
            Some(save) => (
                format!(
                    "{}\n{}",
                    save.bin_path.display(),
                    save.slot_path.display()
                ),
                self.checksum_text(),
            ),
            None => return empty_page("セーブデータがありません"),
        };
        let backups = self.backups.clone();
        let copy_slot = self
            .fields
            .get(&Field::CopySlot)
            .map(|entry| entry.input.read(cx).content().to_owned())
            .unwrap_or_else(|| self.copy_slot.to_string());

        let mut content = div().flex().flex_col().gap_3().max_w(rems(72.0));
        content = content.child(page_header("ツール"));

        let mut file_section = div().flex().flex_col().gap_2();
        file_section = file_section
            .child(div().text_xs().child(path))
            .child(hint(checksum));
        let mut file_buttons = div().flex().flex_wrap().gap_2();
        file_buttons = file_buttons
            .child(button(
                "open-folder",
                "保存フォルダを開く",
                false,
                cx.listener(|this, _, _, cx| {
                    this.open_save_folder();
                    cx.notify();
                }),
            ))
            .child(button(
                "check",
                "状態を再確認",
                false,
                cx.listener(|this, _, _, cx| {
                    if let Some(save) = this.save.as_mut() {
                        let checks = save.verify();
                        this.status = checksum_line(&checks);
                    }
                    cx.notify();
                }),
            ));
        file_section = file_section.child(file_buttons);
        content = content.child(section("ファイル", file_section));

        let mut backup_rows = div().flex().flex_col().gap_1();
        if backups.is_empty() {
            backup_rows = backup_rows.child(hint("バックアップはありません"));
        } else {
            for (index, backup) in backups.iter().take(8).enumerate() {
                let label = backup
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("backup")
                    .to_owned();
                let mut row = div().flex().items_center().gap_2();
                row = row.child(div().flex_1().text_sm().truncate().child(label));
                row = row.child(button(
                    ("restore", index),
                    "復元",
                    false,
                    cx.listener(move |this, _, _, cx| {
                        this.modal = Some(Modal::RestoreBackup(index));
                        cx.notify();
                    }),
                ));
                backup_rows = backup_rows.child(row);
            }
        }
        let mut backup_section = div().flex().flex_col().gap_2();
        backup_section = backup_section.child(backup_rows);
        backup_section = backup_section.child(div().flex().child(button(
            "create-backup",
            "今すぐバックアップ",
            false,
            cx.listener(|this, _, _, cx| {
                let result = this
                    .save
                    .as_ref()
                    .map(|save| save.create_backup("manual"));
                match result {
                    Some(Ok(Some(_))) => {
                        this.status = "バックアップを作成しました".to_owned();
                        if let Some(save) = this.save.as_ref() {
                            this.backups = save.list_backups();
                        }
                    }
                    Some(Ok(None)) => this.status = "バックアップ対象がありません".to_owned(),
                    Some(Err(error)) => this.status = format!("バックアップ失敗: {error}"),
                    None => {}
                }
                cx.notify();
            }),
        )));
        content = content.child(section("バックアップ", backup_section));

        let mut copy_row = div().flex().flex_wrap().items_center().gap_2();
        copy_row = copy_row
            .child(form_row(
                "複製先",
                self.input_box(Field::CopySlot, copy_slot, 5.0, cx),
            ))
            .child(hint("dataXXXX / 1 - 16"))
            .child(button(
                "copy-slot-run",
                "複製",
                false,
                cx.listener(|this, _, _, cx| this.copy_current_to_slot(cx)),
            ));
        content = content.child(section(
            "別スロットに複製",
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(copy_row)
                .child(hint("既存の複製先は上書きされます。上書き前にバックアップします。")),
        ));

        content = content.child(section(
            "書き出し",
            div().flex().child(button(
                "export-json",
                "JSON に書き出す",
                false,
                cx.listener(|this, _, _, cx| this.export_json(cx)),
            )),
        ));

        page_scroll("tools-scroll").child(content)
    }

    fn picker_content(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(picker) = self.picker else {
            return empty_page("選択画面がありません");
        };
        let title = match picker.target {
            PickerTarget::Persona(_) => "ペルソナを選択",
            PickerTarget::Skill(_, _) => "スキルを選択",
            PickerTarget::Item(_) => "アイテムを選択",
        };
        let entries = self.picker_entries(cx);
        let categories = game::category_options();
        let category = categories
            .get(picker.category)
            .map(|(_, label)| *label)
            .unwrap_or("すべて");
        let count = entries.len();
        let mut header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(div().text_base().text_color(rgb(ACCENT)).child(title))
            .child(div().w(rems(16.0)).child(self.search_input.clone()));
        if matches!(picker.target, PickerTarget::Item(_)) {
            header = header.child(button(
                "picker-category",
                format!("カテゴリ: {category}"),
                false,
                cx.listener(|this, _, _, cx| {
                    if let Some(picker) = this.picker.as_mut() {
                        picker.category =
                            (picker.category + 1) % game::category_options().len();
                    }
                    cx.notify();
                }),
            ));
        }
        if matches!(picker.target, PickerTarget::Skill(_, _)) {
            header = header.child(hint("使用可能スキル"));
        }
        header = header
            .child(hint(format!("{count} 件")))
            .child(button(
                "picker-cancel",
                "閉じる",
                false,
                cx.listener(|this, _, _, cx| {
                    this.picker = None;
                    this.clear_search(cx);
                    cx.notify();
                }),
            ));

        let list = uniform_list(
            "picker-list",
            count,
            cx.processor(move |_this, range, _window, cx| {
                let mut rows = Vec::new();
                for index in range {
                    let Some((id, label)): Option<&(u16, String)> = entries.get(index) else {
                        continue;
                    };
                    let id = *id;
                    rows.push(
                        div()
                            .id(("picker-row", index))
                            .h(rems(1.9))
                            .w_full()
                            .px_2()
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .bg(rgb(PANEL))
                            .border_b_1()
                            .border_color(rgb(BORDER))
                            .hover(|style| style.bg(rgb(PANEL_2)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_picker(id, cx);
                            }))
                            .child(label.clone()),
                    );
                }
                rows
            }),
        )
        .h_full();
        page_fixed("picker-page").child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .h_full()
                .child(header)
                .child(list),
        )
    }

    fn persona_editor(&mut self, target: PersonaTarget, cx: &mut Context<Self>) -> Div {
        let Some(persona) = self.persona(target) else {
            return div().child("ペルソナがありません");
        };
        let fixed = matches!(target, PersonaTarget::Compendium(_));
        let compendium_id = match target {
            PersonaTarget::Compendium(index) => Some(index as u16 + 1),
            _ => None,
        };
        let id = compendium_id.unwrap_or_else(|| persona.id());
        let mut content = div().flex().flex_col().gap_3();

        if fixed {
            content = content.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_base()
                            .text_color(rgb(ACCENT))
                            .child(format!("{:03}  {}", id, self.game.persona_name(id))),
                    )
                    .child(button(
                        "comp-toggle",
                        if persona.exists() {
                            "登録済み (解除)"
                        } else {
                            "未登録 (登録)"
                        },
                        persona.exists(),
                        cx.listener(move |this, _, _, cx| {
                            if let PersonaTarget::Compendium(index) = target {
                                this.toggle_compendium(index);
                                cx.notify();
                            }
                        }),
                    )),
            );
        } else {
            content = content.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_base()
                            .text_color(rgb(ACCENT))
                            .child(self.game.persona_display(id)),
                    )
                    .child(button(
                        "persona-select",
                        if id == 0 {
                            "ペルソナを選択"
                        } else {
                            "変更"
                        },
                        false,
                        cx.listener(move |this, _, window, cx| {
                            this.open_picker(PickerTarget::Persona(target), 0, window, cx);
                        }),
                    )),
            );
        }

        if persona.id() == 0 && !persona.exists() && !fixed {
            content = content.child(hint("空き枠です。「ペルソナを選択」から追加してください。"));
            return content;
        }
        if !persona.exists() && fixed {
            content = content.child(hint("未登録です。上のボタンで登録するとステータスを編集できます。"));
            return content;
        }

        let info = self.game.persona(id);
        let arcana = info
            .map(|info| game::arcana_jp(info.arcana))
            .unwrap_or("");
        content = content
            .child(hint(format!("アルカナ: {arcana}")))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(form_row(
                        "レベル",
                        self.input_box(
                            Field::PersonaLevel(target),
                            persona.level(),
                            5.0,
                            cx,
                        ),
                    ))
                    .child(form_row(
                        "XP",
                        self.input_box(Field::PersonaXp(target), persona.xp(), 8.0, cx),
                    )),
            )
            .child(section_title("ステータス"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(
                        ["力", "魔", "耐", "速", "運"]
                            .iter()
                            .enumerate()
                            .map(|(index, label)| {
                                form_row(
                                    *label,
                                    self.input_box(
                                        Field::PersonaStat(target, index),
                                        persona.stats()[index],
                                        4.5,
                                        cx,
                                    ),
                                )
                            }),
                    ),
            )
            .child(section_title("スキル"));
        let skills = persona.skills();
        for (index, skill_id) in skills.iter().enumerate() {
            content = content.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(rems(2.0)).text_xs().text_color(rgb(MUTED)).child(format!("{:02}", index + 1)))
                    .child(
                        div()
                            .w(rems(16.0))
                            .text_sm()
                            .truncate()
                            .child(self.game.skill_display(*skill_id)),
                    )
                    .child(button(
                        ("skill-pick", index),
                        "選択",
                        false,
                        cx.listener(move |this, _, window, cx| {
                            this.open_picker(PickerTarget::Skill(target, index), 0, window, cx);
                        }),
                    ))
                    .child(button(
                        ("skill-clear", index),
                        "クリア",
                        false,
                        cx.listener(move |this, _, _, cx| {
                            let Some(mut persona) = this.persona(target) else {
                                return;
                            };
                            let mut skills = persona.skills();
                            skills[index] = 0;
                            persona.set_skills(&skills);
                            this.set_persona(target, persona);
                            this.modified = true;
                            cx.notify();
                        }),
                    )),
            );
        }
        content
    }

    fn top_bar(&mut self, cx: &mut Context<Self>) -> Div {
        let slot = self.slot_label();
        let bar = div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .bg(rgb(PANEL))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().text_base().child("P4G Save Editor")),
            )
            .child(div().w(rems(1.0)))
            .child(button(
                "prev-slot",
                "‹",
                false,
                cx.listener(|this, _, _, cx| this.change_slot(-1, cx)),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w(rems(14.0))
                    .max_w(rems(26.0))
                    .child(div().text_sm().truncate().child(slot))
                    .child(hint(format!(
                        "スロット {}/{}",
                        self.selected_slot + 1,
                        self.slots.len()
                    ))),
            )
            .child(button(
                "next-slot",
                "›",
                false,
                cx.listener(|this, _, _, cx| this.change_slot(1, cx)),
            ))
            .child(button(
                "open-folder",
                "フォルダ",
                false,
                cx.listener(|this, _, _, cx| this.open_folder(cx)),
            ))
            .child(button(
                "reload",
                "再読込",
                false,
                cx.listener(|this, _, _, cx| {
                    if this.has_pending_changes(cx) {
                        this.modal = Some(Modal::Reload);
                        cx.notify();
                    } else {
                        this.reload_current(cx);
                    }
                }),
            ))
            .child(div().flex_1())
            .child(button(
                "advanced",
                if self.advanced { "詳細: ON" } else { "詳細: OFF" },
                self.advanced,
                cx.listener(|this, _, _, cx| {
                    this.advanced = !this.advanced;
                    cx.notify();
                }),
            ))
            .child(button(
                "save",
                "保存",
                true,
                cx.listener(|this, _, _, cx| this.save_current(cx)),
            ));
        bar
    }

    fn sidebar(&mut self, cx: &mut Context<Self>) -> Div {
        let mut nav = div()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .w(rems(10.5))
            .h_full()
            .bg(rgb(PANEL))
            .border_r_1()
            .border_color(rgb(BORDER));
        for (index, page) in Page::ALL.iter().enumerate() {
            let selected = self.page == *page;
            nav = nav.child(
                div()
                    .id(("nav", index))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .border_l_2()
                    .border_color(if selected { rgb(ACCENT) } else { rgb(PANEL) })
                    .cursor_pointer()
                    .bg(if selected { rgb(PANEL_2) } else { rgb(PANEL) })
                    .text_color(if selected { rgb(ACCENT) } else { rgb(TEXT) })
                    .hover(|style| style.bg(rgb(PANEL_2)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = *page;
                        this.picker = None;
                        this.modal = None;
                        this.clear_search(cx);
                        cx.notify();
                    }))
                    .child(page.label()),
            );
        }
        nav.child(div().flex_1())
    }

    fn bottom_bar(&mut self, cx: &mut Context<Self>) -> Div {
        let pending = self.has_pending_changes(cx);
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_1()
            .bg(rgb(PANEL))
            .border_t_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .truncate()
                    .child(self.status.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(if pending { rgb(ACCENT) } else { rgb(MUTED) })
                    .child(if pending {
                        "未保存の変更あり"
                    } else {
                        "保存済み"
                    }),
            )
            .child(hint(format!("v{VERSION}")))
    }

    fn modal_view(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Option<Div> {
        let modal = self.modal?;
        let (title, body, primary_label, primary) = match modal {
            Modal::SwitchSlot(index) => (
                "未保存の変更",
                "変更を保存してからスロットを切り替えますか？".to_owned(),
                "保存して移動",
                Modal::SwitchSlot(index),
            ),
            Modal::Reload => (
                "未保存の変更",
                "変更を破棄して再読込しますか？".to_owned(),
                "保存して再読込",
                Modal::Reload,
            ),
            Modal::RestoreBackup(index) => (
                "バックアップの復元",
                "選択したバックアップで現在のセーブを上書きします。".to_owned(),
                "復元する",
                Modal::RestoreBackup(index),
            ),
        };
        let primary = button(
            "modal-primary",
            primary_label,
            true,
            cx.listener(move |this, _, _, cx| match primary {
                Modal::SwitchSlot(index) => {
                    this.save_current(cx);
                    this.modal = None;
                    if !this.has_pending_changes(cx) {
                        this.load_slot(index, cx);
                    }
                    cx.notify();
                }
                Modal::Reload => {
                    this.save_current(cx);
                    this.modal = None;
                    if !this.has_pending_changes(cx) {
                        this.reload_current(cx);
                    }
                    cx.notify();
                }
                Modal::RestoreBackup(index) => this.restore_backup(index, cx),
            }),
        );
        let cancel = button(
            "modal-cancel",
            "キャンセル",
            false,
            cx.listener(|this, _, _, cx| {
                this.modal = None;
                cx.notify();
            }),
        );
        let secondary = match modal {
            Modal::SwitchSlot(index) => Some(button(
                "modal-discard",
                "破棄して移動",
                false,
                cx.listener(move |this, _, _, cx| {
                    this.modal = None;
                    this.load_slot(index, cx);
                }),
            )),
            Modal::Reload => Some(button(
                "modal-discard",
                "破棄して再読込",
                false,
                cx.listener(|this, _, _, cx| {
                    this.modal = None;
                    this.reload_current(cx);
                }),
            )),
            Modal::RestoreBackup(_) => None,
        };
        let mut buttons = div().flex().gap_2().child(primary);
        if let Some(secondary) = secondary {
            buttons = buttons.child(secondary);
        }
        buttons = buttons.child(cancel);
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(hsla(0., 0., 0., 0.68))
                .child(
                    div()
                        .w(rems(34.0))
                        .p_4()
                        .rounded_md()
                        .bg(rgb(PANEL))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().text_base().text_color(rgb(ACCENT)).child(title))
                        .child(div().text_sm().child(body))
                        .child(buttons),
                ),
        )
    }
}

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let scale = (f32::from(viewport.width) / 1180.0)
            .min(f32::from(viewport.height) / 760.0)
            .clamp(0.90, 1.30);
        window.set_rem_size(px(14.0 * scale));

        let page = self.page_content(window, cx);
        let modal = self.modal_view(window, cx);
        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(TEXT));
        root = root
            .child(self.top_bar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(self.sidebar(cx))
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.))
                            .min_w(px(0.))
                            .child(page),
                    ),
            )
            .child(self.bottom_bar(cx));
        if let Some(modal) = modal {
            root = root.child(modal);
        }
        root
    }
}

fn collect_slots() -> Vec<PathBuf> {
    let mut result = Vec::new();
    for folder in save::find_save_folders() {
        result.extend(save::list_slots(folder));
    }
    result.sort_by_key(|path| {
        std::cmp::Reverse(
            path.metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .map(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .flatten(),
        )
    });
    result
}

fn page_scroll(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_col()
        .h_full()
        .w_full()
        .p_3()
        .overflow_scroll()
}

fn page_fixed(id: &'static str) -> Stateful<Div> {
    div().id(id).flex().flex_col().h_full().w_full().p_3()
}

fn empty_page(message: &str) -> Stateful<Div> {
    div()
        .flex()
        .items_center()
        .justify_center()
        .size_full()
        .id("empty-page")
        .child(hint(message.to_owned()))
}

fn panel(content: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_2()
        .w_full()
        .rounded_md()
        .bg(rgb(PANEL))
        .border_1()
        .border_color(rgb(BORDER))
        .child(content)
}

fn page_header(title: &str) -> Div {
    div()
        .pb_2()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(div().text_lg().child(title.to_owned()))
}

fn section(title: impl Into<SharedString>, content: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .py_2()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(section_title(title))
        .child(content)
}

fn section_title(text: impl Into<SharedString>) -> Div {
    div().text_sm().text_color(rgb(ACCENT)).child(text.into())
}

fn hint(text: impl Into<SharedString>) -> Div {
    div().text_xs().text_color(rgb(MUTED)).child(text.into())
}

fn phase_text(text: &str) -> Div {
    div().text_sm().child(text.to_owned())
}

fn form_row(label: impl Into<SharedString>, control: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(rems(7.5))
                .flex_none()
                .text_sm()
                .text_color(rgb(MUTED))
                .child(label.into()),
        )
        .child(control)
}

fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    accent: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(if accent { rgb(ACCENT) } else { rgb(BORDER) })
        .bg(if accent { rgb(ACCENT) } else { rgb(PANEL_2) })
        .text_color(if accent { rgb(ACCENT_DARK) } else { rgb(TEXT) })
        .text_sm()
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if accent {
                rgb(0xe5c76e)
            } else {
                rgb(0x252632)
            })
        })
        .on_click(on_click)
        .child(label)
}

fn field_priority(field: Field) -> u8 {
    match field {
        Field::LastName | Field::FirstName => 0,
        Field::DayDate | Field::NextDate => 1,
        Field::DaySerial | Field::NextDay => 2,
        _ => 3,
    }
}

fn field_label(field: Field) -> &'static str {
    match field {
        Field::LastName => "姓",
        Field::FirstName => "名",
        Field::Money => "所持金",
        Field::McLevel => "レベル",
        Field::McXp => "累計 XP",
        Field::McHp => "HP",
        Field::McSp => "SP",
        Field::Playtime => "プレイ時間",
        Field::RiseLines => "リセのナビ回数",
        Field::SocialStat(_) => "社交ステータス",
        Field::DayDate | Field::DaySerial => "現在の日付",
        Field::NextDate | Field::NextDay => "次の日付",
        Field::UnitHp(_) => "メンバー HP",
        Field::UnitSp(_) => "メンバー SP",
        Field::PersonaLevel(_) => "ペルソナレベル",
        Field::PersonaXp(_) => "ペルソナ XP",
        Field::PersonaStat(_, _) => "ペルソナステータス",
        Field::SocialRank(_) => "コミュランク",
        Field::SocialAffinity(_) => "好感度",
        Field::ItemCount(_) => "所持数",
        Field::CopySlot => "複製先",
    }
}

fn parse_date(text: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(text.trim(), "%Y/%m/%d")
        .or_else(|_| NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d"))
        .map_err(|_| "日付は YYYY/MM/DD で入力してください".to_owned())
}

fn parse_playtime(text: &str) -> Result<(u32, u32, u32), String> {
    let parts = text
        .split(':')
        .map(|part| part.trim().parse::<u32>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "時間は H:MM:SS で入力してください".to_owned())?;
    match parts.as_slice() {
        [hours, minutes, seconds] => {
            if *minutes > 59 || *seconds > 59 {
                Err("分と秒は 0 から 59 の範囲で入力してください".to_owned())
            } else if *hours > 999 {
                Err("時間は 0 から 999 の範囲で入力してください".to_owned())
            } else {
                Ok((*hours, *minutes, *seconds))
            }
        }
        [hours, minutes] => {
            if *minutes > 59 {
                Err("分は 0 から 59 の範囲で入力してください".to_owned())
            } else if *hours > 999 {
                Err("時間は 0 から 999 の範囲で入力してください".to_owned())
            } else {
                Ok((*hours, *minutes, 0))
            }
        }
        [seconds] => {
            if *seconds > 359999 {
                Err("時間は H:MM:SS で入力してください".to_owned())
            } else {
                Ok((seconds / 3600, (seconds / 60) % 60, seconds % 60))
            }
        }
        _ => Err("時間は H:MM:SS で入力してください".to_owned()),
    }
}

fn format_playtime(hours: u32, minutes: u32, seconds: u32) -> String {
    format!("{hours}:{minutes:02}:{seconds:02}")
}

fn checksum_line(checks: &save::LoadedChecks) -> String {
    format!(
        "ヘッダ {} / セーブ MD5 {} / スロット MD5 {}",
        yes_no(checks.header),
        yes_no(checks.save_md5),
        yes_no(checks.slot_md5)
    )
}

fn yes_no(value: bool) -> &'static str {
    if value { "OK" } else { "NG" }
}
