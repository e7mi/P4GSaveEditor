use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, NaiveDate, Timelike};
use encoding_rs::SHIFT_JIS;
use serde_json::{Value, json};

use crate::game::{self, GameData, PersonaInfo};

pub const SAVE_SIZE: usize = 0x35351;
pub const SLOT_SIZE: usize = 884;
pub const SLOT_DATA_OFFSET: usize = 0x28;
pub const SLOT_DATA_SIZE: usize = 0x34C;
pub const SLOT_COMMENT_SIZE: usize = 0x228;
pub const SLOT_MAGIC: &[u8; 8] = b"SAVE0001";
pub const SLOT_SUFFIX: &[u8; 8] = b"P4GOLDEN";

pub const OFF_DAY: usize = 0x04;
pub const OFF_PHASE: usize = 0x06;
pub const OFF_MONEY: usize = 0x58;
pub const OFF_PARTY: usize = 0x5C;
pub const OFF_LNAME: usize = 0x64;
pub const OFF_FNAME: usize = 0x76;
pub const OFF_INVENTORY: usize = 0x88;
pub const OFF_MC_PERSONAS: usize = 0xA8C;
pub const OFF_PARTY_UNITS: usize = 0xCD4;
pub const OFF_MC_LEVEL: usize = 0xCDA;
pub const OFF_MC_HP: usize = 0xCDC;
pub const OFF_MC_SP: usize = 0xCDE;
pub const OFF_SOCIAL_STATS: usize = 0xD08;
pub const OFF_MC_XP: usize = 0xD14;
pub const OFF_MC_EQUIP: usize = 0xD20;
pub const OFF_PLAYTIME: usize = 0x173C;
pub const OFF_DIFFICULTY: usize = 0x1748;
pub const OFF_SOCIAL_LINKS: usize = 0x1970;
pub const OFF_DAY_BODY: usize = 0x1954;
pub const OFF_PHASE_BODY: usize = 0x1956;
pub const OFF_NEXT_DAY: usize = 0x195C;
pub const OFF_NEXT_PHASE: usize = 0x195E;
pub const OFF_COMPENDIUM: usize = 0x25D8;
pub const OFF_RISE_LINES: usize = 0x55B7;
pub const OFF_HEADER_CHECKSUM: usize = 0x36;

pub const OFF_NAME_PC_HEADER_LAST: usize = 0x10;
pub const OFF_NAME_PC_HEADER_FIRST: usize = 0x22;
pub const OFF_NAME_PC_LAST: usize = 0x15130;
pub const OFF_NAME_PC_FIRST: usize = 0x15142;
pub const OFF_NAME_PC_LAST_ALT: usize = 0x19CEC;
pub const OFF_NAME_PC_FIRST_ALT: usize = 0x19CFE;
pub const OFF_NAME_PC_FLAG: usize = 0x19CE8;

pub const INVENTORY_SIZE: usize = 2559;
pub const MAX_MONEY: u32 = 9_999_999;
pub const MC_PERSONA_COUNT: usize = 12;
pub const PARTY_UNIT_COUNT: usize = 8;
pub const PERSONA_STRIDE: usize = 48;
pub const UNIT_STRIDE: usize = 132;
pub const UNIT_PERSONA_OFFSET: usize = 0x54;
pub const EQUIP_OFFSETS: [usize; 4] = [0x4C, 0x4E, 0x50, 0x52];
pub const SOCIAL_LINK_COUNT: usize = 23;
pub const SOCIAL_LINK_STRIDE: usize = 16;
pub const COMPENDIUM_COUNT: usize = 249;
pub const MAX_DAY: u16 = 354;

pub const EPOCH: NaiveDate = match NaiveDate::from_ymd_opt(2011, 4, 1) {
    Some(date) => date,
    None => panic!("invalid epoch"),
};

#[derive(Debug)]
pub enum SaveError {
    Io(io::Error),
    Invalid(String),
    TooLong(String),
    Encode(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Invalid(message) => write!(f, "{message}"),
            Self::TooLong(message) => write!(f, "{message}"),
            Self::Encode(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<io::Error> for SaveError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub type Result<T> = std::result::Result<T, SaveError>;

fn read_u16(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

fn read_u32(buf: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn decode_utf8(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).trim().to_owned()
}

fn decode_cp932(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    SHIFT_JIS.decode(&raw[..end]).0.trim().to_owned()
}

fn encode_cp932(text: &str) -> Result<Vec<u8>> {
    let (raw, _, had_errors) = SHIFT_JIS.encode(text);
    if had_errors {
        return Err(SaveError::Encode(format!(
            "この文字は保存できません: {text}"
        )));
    }
    Ok(raw.into_owned())
}

fn utf8_prefix(text: &str, max: usize) -> &[u8] {
    let mut end = 0;
    for (index, character) in text.char_indices() {
        let next = index + character.len_utf8();
        if next > max {
            break;
        }
        end = next;
    }
    &text.as_bytes()[..end]
}

fn write_fixed_utf8(buf: &mut [u8], offset: usize, text: &str, size: usize) {
    let raw = utf8_prefix(text, size);
    buf[offset..offset + size].fill(0);
    buf[offset..offset + raw.len()].copy_from_slice(raw);
}

fn write_fixed_cp932(buf: &mut [u8], offset: usize, text: &str, size: usize) -> Result<()> {
    let raw = encode_cp932(text)?;
    if raw.len() > size {
        return Err(SaveError::TooLong(format!(
            "文字列が長すぎます (最大 {size} バイト): {text}"
        )));
    }
    buf[offset..offset + size].fill(0);
    buf[offset..offset + raw.len()].copy_from_slice(&raw);
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct SlotInfo {
    pub name: String,
    pub level: u16,
    pub date_text: String,
    pub phase_text: String,
    pub location: String,
    pub playtime: String,
}

pub struct SlotBlock<'a> {
    pub name: &'a str,
    pub level: u16,
    pub date_text: &'a str,
    pub phase_text: &'a str,
    pub location: &'a str,
    pub difficulty: &'a str,
    pub playtime: &'a str,
}

#[derive(Debug, Clone)]
pub struct SlotMeta {
    pub raw: Vec<u8>,
}

impl SlotMeta {
    pub fn new(raw: &[u8]) -> Result<Self> {
        if raw.len() != SLOT_DATA_SIZE {
            return Err(SaveError::Invalid(format!(
                "binslot のスロット情報サイズが不正です ({})",
                raw.len()
            )));
        }
        Ok(Self { raw: raw.to_vec() })
    }

    pub fn name_line(&self) -> String {
        decode_utf8(&self.raw[0x04..0x44])
    }

    pub fn detail_line(&self) -> String {
        decode_utf8(&self.raw[0x44..0xC4])
    }

    pub fn info(&self) -> SlotInfo {
        let mut name = self.name_line();
        let mut level = 0;
        if let Some(index) = name.rfind(" Lv:") {
            if let Ok(parsed) = name[index + 4..].trim().parse::<u16>() {
                level = parsed;
            }
            name.truncate(index);
        }
        let detail = self.detail_line();
        let mut parts = detail.splitn(3, ':');
        let date_text = parts.next().unwrap_or("").to_owned();
        let phase_text = parts.next().unwrap_or("").to_owned();
        let location = parts.next().unwrap_or("").to_owned();
        let mut playtime = String::new();
        for line in self.block_lines() {
            let text = String::from_utf8(line.clone())
                .unwrap_or_else(|_| SHIFT_JIS.decode(&line).0.into_owned());
            if let Some(value) = text.strip_prefix("プレイ時間") {
                playtime = value.to_owned();
            }
        }
        SlotInfo {
            name: name.trim().to_owned(),
            level,
            date_text,
            phase_text,
            location,
            playtime,
        }
    }

    pub fn set_name(&mut self, name: &str, level: u16) {
        let text = format!("{name} Lv:{level}");
        write_fixed_utf8(&mut self.raw, 0x04, text.trim(), 0x40);
    }

    pub fn set_detail(&mut self, date_text: &str, phase_text: &str, location: &str) {
        let text = format!("{date_text}:{phase_text}:{location}");
        write_fixed_utf8(&mut self.raw, 0x44, &text, 0x80);
    }

    pub fn set_timestamp(&mut self, value: chrono::DateTime<Local>) {
        let values = [
            value.year() as u16,
            value.month() as u16,
            value.day() as u16,
            value.hour() as u16,
            value.minute() as u16,
            value.second() as u16,
        ];
        for (index, value) in values.iter().enumerate() {
            write_u16(&mut self.raw, 0x30C + index * 2, *value);
        }
    }

    fn block_lines(&self) -> Vec<Vec<u8>> {
        let block = &self.raw[0xC4..0xC4 + SLOT_COMMENT_SIZE];
        let end = block.iter().position(|&b| b == 0).unwrap_or(block.len());
        let block = &block[..end];
        if block.is_empty() {
            return Vec::new();
        }
        block
            .split(|&b| b == b'\n')
            .map(|line| line.to_vec())
            .collect()
    }

    pub fn update_block(&mut self, block: SlotBlock<'_>) {
        let SlotBlock {
            name,
            level,
            date_text,
            phase_text,
            location,
            difficulty,
            playtime,
        } = block;
        let mut lines = self.block_lines();
        if lines.is_empty() {
            lines = vec![
                SHIFT_JIS.encode("日付:").0.into_owned(),
                SHIFT_JIS.encode("名前:").0.into_owned(),
                SHIFT_JIS.encode("難易度:").0.into_owned(),
                SHIFT_JIS.encode("場所:").0.into_owned(),
                SHIFT_JIS.encode("プレイ時間").0.into_owned(),
                b"LANG0".to_vec(),
            ];
        }
        let replacements = [
            (
                SHIFT_JIS.encode("日付:").0.into_owned(),
                format!("日付:{date_text}:{phase_text}").into_bytes(),
            ),
            (
                SHIFT_JIS.encode("名前:").0.into_owned(),
                format!("名前:{name} Lv:{level}").into_bytes(),
            ),
            (
                SHIFT_JIS.encode("難易度:").0.into_owned(),
                format!("難易度:{difficulty} ").into_bytes(),
            ),
            (
                SHIFT_JIS.encode("場所:").0.into_owned(),
                format!("場所:{location}").into_bytes(),
            ),
            (
                SHIFT_JIS.encode("プレイ時間").0.into_owned(),
                format!("プレイ時間{playtime}").into_bytes(),
            ),
        ];
        let mut seen = [false; 5];
        let mut built: Vec<Vec<u8>> = Vec::with_capacity(lines.len() + 5);
        for line in lines {
            let mut replaced = false;
            for (index, (prefix, replacement)) in replacements.iter().enumerate() {
                if line.starts_with(prefix) {
                    built.push(replacement.clone());
                    seen[index] = true;
                    replaced = true;
                    break;
                }
            }
            if !replaced {
                built.push(line);
            }
        }
        for (index, (_, replacement)) in replacements.iter().enumerate() {
            if !seen[index] {
                built.push(replacement.clone());
            }
        }
        let mut block = Vec::new();
        for (index, line) in built.iter().enumerate() {
            if index != 0 {
                block.push(b'\n');
            }
            block.extend_from_slice(line);
        }
        if block.len() > SLOT_COMMENT_SIZE {
            block.truncate(SLOT_COMMENT_SIZE);
        }
        self.raw[0xC4..0xC4 + SLOT_COMMENT_SIZE].fill(0);
        self.raw[0xC4..0xC4 + block.len()].copy_from_slice(&block);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Persona {
    pub raw: [u8; PERSONA_STRIDE],
}

impl Persona {
    pub fn from_slice(raw: &[u8]) -> Self {
        let mut data = [0u8; PERSONA_STRIDE];
        data.copy_from_slice(&raw[..PERSONA_STRIDE]);
        Self { raw: data }
    }

    pub fn flags(&self) -> u16 {
        read_u16(&self.raw, 0)
    }

    pub fn set_flags(&mut self, value: u16) {
        write_u16(&mut self.raw, 0, value);
    }

    pub fn exists(&self) -> bool {
        self.flags() & 1 != 0
    }

    pub fn id(&self) -> u16 {
        read_u16(&self.raw, 0x02)
    }

    pub fn set_id(&mut self, value: u16) {
        write_u16(&mut self.raw, 0x02, value);
    }

    pub fn level(&self) -> u16 {
        read_u16(&self.raw, 0x04)
    }

    pub fn set_level(&mut self, value: u16) {
        write_u16(&mut self.raw, 0x04, value.min(99));
    }

    pub fn xp(&self) -> u32 {
        read_u32(&self.raw, 0x08)
    }

    pub fn set_xp(&mut self, value: u32) {
        write_u32(&mut self.raw, 0x08, value);
    }

    pub fn skills(&self) -> [u16; 8] {
        let mut result = [0u16; 8];
        for (index, value) in result.iter_mut().enumerate() {
            *value = read_u16(&self.raw, 0x0C + index * 2);
        }
        result
    }

    pub fn set_skills(&mut self, values: &[u16; 8]) {
        for (index, value) in values.iter().enumerate() {
            write_u16(&mut self.raw, 0x0C + index * 2, *value);
        }
    }

    pub fn stats(&self) -> [u8; 5] {
        let mut result = [0u8; 5];
        result.copy_from_slice(&self.raw[0x1C..0x21]);
        result
    }

    pub fn set_stats(&mut self, values: &[u8; 5]) {
        self.raw[0x1C..0x21].copy_from_slice(values);
    }

    pub fn clear(&mut self) {
        self.raw.fill(0);
    }

    pub fn fill_base(&mut self, info: &PersonaInfo) {
        self.raw.fill(0);
        self.set_flags(1);
        self.set_id(info.id);
        self.set_level(info.level);
        self.set_xp(info.exp);
        let mut skills = [0u16; 8];
        for (index, value) in info.skills.iter().enumerate() {
            skills[index] = *value;
        }
        self.set_skills(&skills);
        self.set_stats(&info.stats);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Unit {
    pub raw: [u8; UNIT_STRIDE],
}

impl Unit {
    pub fn from_slice(raw: &[u8]) -> Self {
        let mut data = [0u8; UNIT_STRIDE];
        data.copy_from_slice(&raw[..UNIT_STRIDE]);
        Self { raw: data }
    }

    pub fn id(&self) -> u16 {
        read_u16(&self.raw, 0x02)
    }

    pub fn hp(&self) -> u16 {
        read_u16(&self.raw, 0x08)
    }

    pub fn set_hp(&mut self, value: u16) {
        write_u16(&mut self.raw, 0x08, value.min(9999));
    }

    pub fn sp(&self) -> u16 {
        read_u16(&self.raw, 0x0A)
    }

    pub fn set_sp(&mut self, value: u16) {
        write_u16(&mut self.raw, 0x0A, value.min(9999));
    }

    pub fn equipment(&self, slot: usize) -> u16 {
        read_u16(&self.raw, EQUIP_OFFSETS[slot])
    }

    pub fn set_equipment(&mut self, slot: usize, value: u16) {
        write_u16(&mut self.raw, EQUIP_OFFSETS[slot], value);
    }

    pub fn persona(&self) -> Persona {
        Persona::from_slice(&self.raw[UNIT_PERSONA_OFFSET..])
    }

    pub fn set_persona(&mut self, persona: &Persona) {
        self.raw[UNIT_PERSONA_OFFSET..UNIT_PERSONA_OFFSET + PERSONA_STRIDE]
            .copy_from_slice(&persona.raw);
    }

    pub fn clear_status(&mut self) {
        self.raw[0x0C..0x2D].fill(0);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SocialLink {
    pub raw: [u8; SOCIAL_LINK_STRIDE],
}

impl SocialLink {
    pub fn from_slice(raw: &[u8]) -> Self {
        let mut data = [0u8; SOCIAL_LINK_STRIDE];
        data.copy_from_slice(&raw[..SOCIAL_LINK_STRIDE]);
        Self { raw: data }
    }

    pub fn id(&self) -> u16 {
        read_u16(&self.raw, 0)
    }

    pub fn rank(&self) -> u16 {
        read_u16(&self.raw, 0x02)
    }

    pub fn set_rank(&mut self, value: u16) {
        write_u16(&mut self.raw, 0x02, value.min(10));
    }

    pub fn affinity(&self) -> u16 {
        read_u16(&self.raw, 0x04)
    }

    pub fn set_affinity(&mut self, value: u16) {
        write_u16(&mut self.raw, 0x04, value.min(999));
    }
}

#[derive(Debug)]
pub struct P4GSave {
    pub data: Vec<u8>,
    pub slot: Vec<u8>,
    pub bin_path: PathBuf,
    pub slot_path: PathBuf,
    pub original_data: Vec<u8>,
    pub original_slot: Vec<u8>,
    pub inventory: Vec<u8>,
    pub slot_meta: SlotMeta,
    pub mc_personas: Vec<Persona>,
    pub units: Vec<Unit>,
    pub social_links: Vec<SocialLink>,
    pub compendium: Vec<Persona>,
    names_changed: bool,
}

impl P4GSave {
    pub fn load(bin_path: impl AsRef<Path>) -> Result<Self> {
        let bin_path = bin_path.as_ref().to_path_buf();
        let slot_path = bin_path.with_extension("binslot");
        Self::load_pair(bin_path, slot_path)
    }

    pub fn load_pair(bin_path: PathBuf, slot_path: PathBuf) -> Result<Self> {
        if !bin_path.exists() {
            return Err(SaveError::Invalid(format!(
                "ファイルがありません: {}",
                bin_path.display()
            )));
        }
        if !slot_path.exists() {
            return Err(SaveError::Invalid(format!(
                "ファイルがありません: {}",
                slot_path.display()
            )));
        }
        let data = fs::read(&bin_path)?;
        let slot = fs::read(&slot_path)?;
        Self::from_bytes(data, slot, bin_path, slot_path)
    }

    pub fn from_bytes(
        data: Vec<u8>,
        slot: Vec<u8>,
        bin_path: PathBuf,
        slot_path: PathBuf,
    ) -> Result<Self> {
        if data.len() < SAVE_SIZE {
            return Err(SaveError::Invalid(format!(
                "セーブデータが短すぎます ({} bytes)",
                data.len()
            )));
        }
        if slot.len() != SLOT_SIZE {
            return Err(SaveError::Invalid(format!(
                "binslot のサイズが不正です ({} bytes)",
                slot.len()
            )));
        }
        if &slot[..8] != SLOT_MAGIC {
            return Err(SaveError::Invalid(
                "binslot のマジックが不正です (SAVE0001)".to_owned(),
            ));
        }
        let data = data[..SAVE_SIZE].to_vec();
        let inventory = data[OFF_INVENTORY..OFF_INVENTORY + INVENTORY_SIZE].to_vec();
        let slot_meta = SlotMeta::new(&slot[SLOT_DATA_OFFSET..SLOT_DATA_OFFSET + SLOT_DATA_SIZE])?;
        let mc_personas = (0..MC_PERSONA_COUNT)
            .map(|index| {
                Persona::from_slice(
                    &data[OFF_MC_PERSONAS + index * PERSONA_STRIDE
                        ..OFF_MC_PERSONAS + (index + 1) * PERSONA_STRIDE],
                )
            })
            .collect();
        let units = (0..PARTY_UNIT_COUNT)
            .map(|index| {
                Unit::from_slice(
                    &data[OFF_PARTY_UNITS + index * UNIT_STRIDE
                        ..OFF_PARTY_UNITS + (index + 1) * UNIT_STRIDE],
                )
            })
            .collect();
        let social_links = (0..SOCIAL_LINK_COUNT)
            .map(|index| {
                SocialLink::from_slice(
                    &data[OFF_SOCIAL_LINKS + index * SOCIAL_LINK_STRIDE
                        ..OFF_SOCIAL_LINKS + (index + 1) * SOCIAL_LINK_STRIDE],
                )
            })
            .collect();
        let compendium = (0..COMPENDIUM_COUNT)
            .map(|index| {
                Persona::from_slice(
                    &data[OFF_COMPENDIUM + index * PERSONA_STRIDE
                        ..OFF_COMPENDIUM + (index + 1) * PERSONA_STRIDE],
                )
            })
            .collect();
        Ok(Self {
            original_data: data.clone(),
            original_slot: slot.clone(),
            data,
            slot,
            bin_path,
            slot_path,
            inventory,
            slot_meta,
            mc_personas,
            units,
            social_links,
            compendium,
            names_changed: false,
        })
    }

    pub fn slot_number(&self) -> u16 {
        slot_number_from_path(&self.bin_path)
    }

    pub fn money(&self) -> u32 {
        read_u32(&self.data, OFF_MONEY)
    }

    pub fn set_money(&mut self, value: u32) {
        write_u32(&mut self.data, OFF_MONEY, value.min(MAX_MONEY));
    }

    pub fn rise_lines(&self) -> u8 {
        self.data.get(OFF_RISE_LINES).copied().unwrap_or(0)
    }

    pub fn set_rise_lines(&mut self, value: u8) {
        if let Some(entry) = self.data.get_mut(OFF_RISE_LINES) {
            *entry = value;
        }
    }

    pub fn party(&self) -> [u16; 3] {
        [
            read_u16(&self.data, OFF_PARTY),
            read_u16(&self.data, OFF_PARTY + 2),
            read_u16(&self.data, OFF_PARTY + 4),
        ]
    }

    pub fn set_party(&mut self, values: [u16; 3]) {
        for (index, value) in values.iter().enumerate() {
            write_u16(&mut self.data, OFF_PARTY + index * 2, *value);
        }
    }

    pub fn lname(&self) -> String {
        decode_cp932(&self.data[OFF_LNAME..OFF_LNAME + 0x12])
    }

    pub fn set_lname(&mut self, value: &str) -> Result<()> {
        write_fixed_cp932(&mut self.data, OFF_LNAME, value, 0x12)?;
        self.names_changed = true;
        Ok(())
    }

    pub fn fname(&self) -> String {
        decode_cp932(&self.data[OFF_FNAME..OFF_FNAME + 0x12])
    }

    pub fn set_fname(&mut self, value: &str) -> Result<()> {
        write_fixed_cp932(&mut self.data, OFF_FNAME, value, 0x12)?;
        self.names_changed = true;
        Ok(())
    }

    pub fn full_name(&self) -> String {
        format!("{} {}", self.lname(), self.fname())
            .trim()
            .to_owned()
    }

    fn sync_pc_names(&mut self) {
        let last = utf8_prefix(&self.lname(), 16).to_vec();
        let first = utf8_prefix(&self.fname(), 16).to_vec();
        for (offset, value) in [
            (OFF_NAME_PC_HEADER_LAST, &last),
            (OFF_NAME_PC_HEADER_FIRST, &first),
            (OFF_NAME_PC_LAST, &last),
            (OFF_NAME_PC_FIRST, &first),
            (OFF_NAME_PC_LAST_ALT, &last),
            (OFF_NAME_PC_FIRST_ALT, &first),
        ] {
            self.data[offset..offset + 16].fill(0);
            self.data[offset..offset + value.len()].copy_from_slice(value);
        }
        self.data[OFF_NAME_PC_FLAG] = 1;
    }

    pub fn mc_level(&self) -> u16 {
        read_u16(&self.data, OFF_MC_LEVEL)
    }

    pub fn set_mc_level(&mut self, value: u16) {
        write_u16(&mut self.data, OFF_MC_LEVEL, value.clamp(1, 99));
    }

    pub fn mc_hp(&self) -> u16 {
        read_u16(&self.data, OFF_MC_HP)
    }

    pub fn set_mc_hp(&mut self, value: u16) {
        write_u16(&mut self.data, OFF_MC_HP, value.min(9999));
    }

    pub fn mc_sp(&self) -> u16 {
        read_u16(&self.data, OFF_MC_SP)
    }

    pub fn set_mc_sp(&mut self, value: u16) {
        write_u16(&mut self.data, OFF_MC_SP, value.min(9999));
    }

    pub fn mc_xp(&self) -> u32 {
        read_u32(&self.data, OFF_MC_XP)
    }

    pub fn set_mc_xp(&mut self, value: u32) {
        write_u32(&mut self.data, OFF_MC_XP, value.min(9_999_999));
    }

    pub fn social_stats(&self) -> [u16; 5] {
        let mut result = [0u16; 5];
        for (index, value) in result.iter_mut().enumerate() {
            *value = read_u16(&self.data, OFF_SOCIAL_STATS + index * 2);
        }
        result
    }

    pub fn set_social_stats(&mut self, values: [u16; 5]) {
        for (index, value) in values.iter().enumerate() {
            write_u16(
                &mut self.data,
                OFF_SOCIAL_STATS + index * 2,
                (*value).min(999),
            );
        }
    }

    pub fn mc_equipment(&self, slot: usize) -> u16 {
        read_u16(&self.data, OFF_MC_EQUIP + slot * 2)
    }

    pub fn set_mc_equipment(&mut self, slot: usize, value: u16) {
        write_u16(&mut self.data, OFF_MC_EQUIP + slot * 2, value);
    }

    pub fn playtime_frames(&self) -> u32 {
        read_u32(&self.data, OFF_PLAYTIME)
    }

    pub fn set_playtime_frames(&mut self, value: u32) {
        write_u32(&mut self.data, OFF_PLAYTIME, value);
    }

    pub fn playtime_hms(&self) -> (u32, u32, u32) {
        let seconds = self.playtime_frames() / 30;
        (seconds / 3600, (seconds / 60) % 60, seconds % 60)
    }

    pub fn set_playtime(&mut self, hours: u32, minutes: u32, seconds: u32) {
        let total = hours.saturating_mul(3600) + minutes * 60 + seconds;
        self.set_playtime_frames(total.saturating_mul(30));
    }

    pub fn difficulty(&self) -> u32 {
        read_u32(&self.data, OFF_DIFFICULTY)
    }

    pub fn set_difficulty(&mut self, value: u32) {
        write_u32(&mut self.data, OFF_DIFFICULTY, value.min(4));
    }

    pub fn day_serial(&self) -> u16 {
        read_u16(&self.data, OFF_DAY_BODY)
    }

    pub fn set_day_serial(&mut self, value: u16) {
        let value = value.min(MAX_DAY);
        write_u16(&mut self.data, OFF_DAY, value);
        write_u16(&mut self.data, OFF_DAY_BODY, value);
    }

    pub fn phase(&self) -> u16 {
        read_u16(&self.data, OFF_PHASE_BODY)
    }

    pub fn set_phase(&mut self, value: u16) {
        let value = value.min(5);
        write_u16(&mut self.data, OFF_PHASE, value);
        write_u16(&mut self.data, OFF_PHASE_BODY, value);
    }

    pub fn next_day(&self) -> u16 {
        read_u16(&self.data, OFF_NEXT_DAY)
    }

    pub fn set_next_day(&mut self, value: u16) {
        write_u16(&mut self.data, OFF_NEXT_DAY, value.min(MAX_DAY));
    }

    pub fn next_phase(&self) -> u16 {
        read_u16(&self.data, OFF_NEXT_PHASE)
    }

    pub fn set_next_phase(&mut self, value: u16) {
        write_u16(&mut self.data, OFF_NEXT_PHASE, value.min(5));
    }

    pub fn game_date(&self) -> NaiveDate {
        EPOCH + chrono::Duration::days(self.day_serial() as i64)
    }

    pub fn unit_hp(&self, index: usize) -> u16 {
        if index == 0 {
            self.mc_hp()
        } else {
            self.units[index].hp()
        }
    }

    pub fn unit_sp(&self, index: usize) -> u16 {
        if index == 0 {
            self.mc_sp()
        } else {
            self.units[index].sp()
        }
    }

    pub fn social_link_maxed(&self) -> usize {
        self.social_links
            .iter()
            .filter(|entry| entry.id() != 0 && entry.rank() >= 10)
            .count()
    }

    pub fn compendium_count(&self, game: &GameData) -> usize {
        game.personas
            .iter()
            .filter(|persona| self.compendium[persona.id as usize - 1].exists())
            .count()
    }

    pub fn slot_info(&self) -> SlotInfo {
        self.slot_meta.info()
    }

    pub fn item_count(&self, item_id: u16) -> u8 {
        self.inventory.get(item_id as usize).copied().unwrap_or(0)
    }

    pub fn set_item_count(&mut self, item_id: u16, count: u8) {
        let index = item_id as usize;
        if index < self.inventory.len() {
            self.inventory[index] = count.min(99);
        }
    }

    pub fn clear_inventory(&mut self) {
        self.inventory.fill(0);
    }

    pub fn add_all_items(&mut self, count: u8, include_key: bool, game: &GameData) {
        for item in &game.items {
            if item.id as usize >= INVENTORY_SIZE {
                continue;
            }
            let base = item.base_tag();
            if matches!(
                base,
                "Expend" | "Material" | "Card" | "Book" | "Veg" | "Gems" | "Fish" | "Bugs"
            ) {
                self.set_item_count(item.id, count);
            } else if matches!(base, "Weapon" | "Armor" | "Acc" | "Clothes")
                || (include_key && matches!(base, "Key" | "Quest" | "SLink" | "Shelf"))
            {
                self.set_item_count(item.id, 1);
            }
        }
    }

    pub fn fill_compendium(&mut self, game: &GameData) {
        for info in &game.personas {
            if info.id == 0 {
                continue;
            }
            let Some(entry) = self.compendium.get_mut(info.id as usize - 1) else {
                continue;
            };
            if !entry.exists() {
                entry.fill_base(info);
            }
        }
    }

    pub fn reset_compendium(&mut self, game: &GameData) {
        for info in &game.personas {
            if info.id == 0 {
                continue;
            }
            let Some(entry) = self.compendium.get_mut(info.id as usize - 1) else {
                continue;
            };
            entry.fill_base(info);
        }
        for (index, entry) in self.compendium.iter_mut().enumerate() {
            if !game.personas_by_id.contains_key(&(index as u16 + 1)) {
                entry.clear();
            }
        }
    }

    pub fn clear_compendium(&mut self) {
        for entry in &mut self.compendium {
            entry.clear();
        }
    }

    pub fn max_social_stats(&mut self) {
        self.set_social_stats([999; 5]);
    }

    pub fn max_social_links(&mut self) {
        for entry in &mut self.social_links {
            if entry.id() != 0 {
                entry.set_rank(10);
                entry.set_affinity(0);
            }
        }
    }

    pub fn heal_party(&mut self) {
        for unit in &mut self.units {
            unit.set_hp(9999);
            unit.set_sp(9999);
            unit.clear_status();
        }
    }

    pub fn max_personas(&mut self, game: &GameData) {
        for persona in &mut self.mc_personas {
            if let Some(info) = game.persona(persona.id()) {
                persona.set_level(99);
                persona.set_xp(info.exp99);
                persona.set_stats(&[99; 5]);
            }
        }
        for unit in &mut self.units {
            let mut persona = unit.persona();
            if let Some(info) = game.persona(persona.id()) {
                persona.set_level(99);
                persona.set_xp(info.exp99);
                persona.set_stats(&[99; 5]);
                unit.set_persona(&persona);
            }
        }
    }

    pub fn slot_summary(&self) -> String {
        let info = self.slot_info();
        format!(
            "data{:04}  {} Lv{}  {}  {}",
            self.slot_number(),
            info.name,
            info.level,
            info.date_text,
            info.playtime
        )
    }

    pub fn apply(&mut self, update_slot: bool) -> (Vec<u8>, Vec<u8>) {
        self.data[OFF_INVENTORY..OFF_INVENTORY + INVENTORY_SIZE].copy_from_slice(&self.inventory);
        let day = self.day_serial();
        let phase = self.phase();
        write_u16(&mut self.data, OFF_DAY, day);
        write_u16(&mut self.data, OFF_DAY_BODY, day);
        write_u16(&mut self.data, OFF_PHASE, phase);
        write_u16(&mut self.data, OFF_PHASE_BODY, phase);
        let playtime = self.playtime_frames();
        let difficulty = self.difficulty();
        let next_day = self.next_day();
        let next_phase = self.next_phase();
        let mc_level = self.mc_level();
        let mc_hp = self.mc_hp();
        let mc_sp = self.mc_sp();
        let mc_xp = self.mc_xp();
        let social_stats = self.social_stats();
        let mc_equipment = [
            self.mc_equipment(0),
            self.mc_equipment(1),
            self.mc_equipment(2),
            self.mc_equipment(3),
        ];
        write_u32(&mut self.data, OFF_PLAYTIME, playtime);
        write_u32(&mut self.data, OFF_DIFFICULTY, difficulty);
        write_u16(&mut self.data, OFF_NEXT_DAY, next_day);
        write_u16(&mut self.data, OFF_NEXT_PHASE, next_phase);

        for (index, persona) in self.mc_personas.iter().enumerate() {
            let offset = OFF_MC_PERSONAS + index * PERSONA_STRIDE;
            self.data[offset..offset + PERSONA_STRIDE].copy_from_slice(&persona.raw);
        }
        for (index, unit) in self.units.iter().enumerate() {
            let offset = OFF_PARTY_UNITS + index * UNIT_STRIDE;
            self.data[offset..offset + UNIT_STRIDE].copy_from_slice(&unit.raw);
        }
        write_u16(&mut self.data, OFF_MC_LEVEL, mc_level);
        write_u16(&mut self.data, OFF_MC_HP, mc_hp);
        write_u16(&mut self.data, OFF_MC_SP, mc_sp);
        write_u32(&mut self.data, OFF_MC_XP, mc_xp);
        for (index, value) in social_stats.iter().enumerate() {
            write_u16(&mut self.data, OFF_SOCIAL_STATS + index * 2, *value);
        }
        for (index, value) in mc_equipment.iter().enumerate() {
            write_u16(&mut self.data, OFF_MC_EQUIP + index * 2, *value);
        }
        for (index, link) in self.social_links.iter().enumerate() {
            let offset = OFF_SOCIAL_LINKS + index * SOCIAL_LINK_STRIDE;
            self.data[offset..offset + SOCIAL_LINK_STRIDE].copy_from_slice(&link.raw);
        }
        for (index, persona) in self.compendium.iter().enumerate() {
            let offset = OFF_COMPENDIUM + index * PERSONA_STRIDE;
            self.data[offset..offset + PERSONA_STRIDE].copy_from_slice(&persona.raw);
        }

        if self.names_changed {
            self.sync_pc_names();
        }
        self.data[OFF_HEADER_CHECKSUM] =
            (self.data[..54].iter().map(|&b| b as u32).sum::<u32>() & 0xFF) as u8;

        if update_slot {
            let name = self.full_name();
            let level = self.mc_level();
            let date_text = self.game_date().format("%m/%d").to_string();
            let phase_text = game::phase_jp(self.phase()).to_owned();
            let location = self.slot_meta.info().location;
            let difficulty = game::difficulty_save(self.difficulty()).to_owned();
            let (hours, minutes, seconds) = self.playtime_hms();
            let playtime = format!("{hours}:{minutes:02}:{seconds:02}");
            self.slot_meta.set_name(&name, level);
            self.slot_meta
                .set_detail(&date_text, &phase_text, &location);
            self.slot_meta.update_block(SlotBlock {
                name: &name,
                level,
                date_text: &date_text,
                phase_text: &phase_text,
                location: &location,
                difficulty: &difficulty,
                playtime: &playtime,
            });
            self.slot_meta.set_timestamp(Local::now());
        }

        self.slot[SLOT_DATA_OFFSET..SLOT_DATA_OFFSET + SLOT_DATA_SIZE]
            .copy_from_slice(&self.slot_meta.raw);
        let bin_md5 = md5::compute(&self.data);
        self.slot[0x18..0x28].copy_from_slice(&bin_md5.0);
        let mut slot_payload = self.slot[SLOT_DATA_OFFSET..].to_vec();
        slot_payload.extend_from_slice(SLOT_SUFFIX);
        let slot_md5 = md5::compute(&slot_payload);
        self.slot[0x08..0x18].copy_from_slice(&slot_md5.0);

        (self.data.clone(), self.slot.clone())
    }

    pub fn check_loaded(&self) -> LoadedChecks {
        let data = &self.original_data;
        let slot = &self.original_slot;
        let header = (data[..54].iter().map(|&b| b as u32).sum::<u32>() & 0xFF) as u8;
        let mut slot_payload = slot[SLOT_DATA_OFFSET..].to_vec();
        slot_payload.extend_from_slice(SLOT_SUFFIX);
        LoadedChecks {
            header: header == data[OFF_HEADER_CHECKSUM],
            save_md5: md5::compute(data).0 == slot[0x18..0x28],
            slot_md5: md5::compute(&slot_payload).0 == slot[0x08..0x18],
        }
    }

    pub fn verify(&mut self) -> LoadedChecks {
        let (data, slot) = self.apply(false);
        let header = (data[..54].iter().map(|&b| b as u32).sum::<u32>() & 0xFF) as u8;
        let mut slot_payload = slot[SLOT_DATA_OFFSET..].to_vec();
        slot_payload.extend_from_slice(SLOT_SUFFIX);
        LoadedChecks {
            header: header == data[OFF_HEADER_CHECKSUM],
            save_md5: md5::compute(&data).0 == slot[0x18..0x28],
            slot_md5: md5::compute(&slot_payload).0 == slot[0x08..0x18],
        }
    }

    pub fn save(&mut self, backup: bool) -> Result<Option<PathBuf>> {
        let (data, slot) = self.apply(true);
        let backup_path = if backup {
            self.create_backup("save")?
        } else {
            None
        };
        write_atomic(&self.bin_path, &data)?;
        write_atomic(&self.slot_path, &slot)?;
        self.original_data = data;
        self.original_slot = slot;
        self.names_changed = false;
        Ok(backup_path)
    }

    pub fn copy_to(
        &mut self,
        target_bin: impl AsRef<Path>,
        target_slot: Option<PathBuf>,
        backup: bool,
    ) -> Result<()> {
        let target_bin = target_bin.as_ref().to_path_buf();
        let target_slot = target_slot.unwrap_or_else(|| target_bin.with_extension("binslot"));
        let (data, slot) = self.apply(true);
        if backup {
            for path in [&target_bin, &target_slot] {
                if path.exists() {
                    let folder = path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join("backup")
                        .join(format!(
                            "{}_{}_copy",
                            path.file_stem()
                                .and_then(|value| value.to_str())
                                .unwrap_or("data"),
                            Local::now().format("%Y%m%d_%H%M%S_%f")
                        ));
                    fs::create_dir_all(&folder)?;
                    fs::copy(path, folder.join(path.file_name().unwrap_or_default()))?;
                }
            }
        }
        write_atomic(&target_bin, &data)?;
        write_atomic(&target_slot, &slot)?;
        Ok(())
    }

    pub fn create_backup(&self, tag: &str) -> Result<Option<PathBuf>> {
        let parent = self.bin_path.parent().unwrap_or_else(|| Path::new("."));
        let stem = self
            .bin_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("data");
        let folder = parent.join("backup").join(format!(
            "{}_{}_{}",
            stem,
            Local::now().format("%Y%m%d_%H%M%S_%f"),
            tag
        ));
        if !self.bin_path.exists() && !self.slot_path.exists() {
            return Ok(None);
        }
        fs::create_dir_all(&folder)?;
        if self.bin_path.exists() {
            fs::copy(
                &self.bin_path,
                folder.join(self.bin_path.file_name().unwrap_or_default()),
            )?;
        }
        if self.slot_path.exists() {
            fs::copy(
                &self.slot_path,
                folder.join(self.slot_path.file_name().unwrap_or_default()),
            )?;
        }
        Ok(Some(folder))
    }

    pub fn list_backups(&self) -> Vec<PathBuf> {
        let parent = self.bin_path.parent().unwrap_or_else(|| Path::new("."));
        let root = parent.join("backup");
        let stem = self
            .bin_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("data");
        let mut result = Vec::new();
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .is_some_and(|name| name.starts_with(stem))
                {
                    result.push(path);
                }
            }
        }
        result.sort_by_key(|path| {
            path.metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .map(std::cmp::Reverse)
        });
        result
    }

    pub fn restore_backup(&mut self, folder: &Path) -> Result<()> {
        let bin_file = folder.join(self.bin_path.file_name().unwrap_or_default());
        let slot_file = folder.join(self.slot_path.file_name().unwrap_or_default());
        if !bin_file.exists() || !slot_file.exists() {
            return Err(SaveError::Invalid(
                "バックアップに必要なファイルがありません".to_owned(),
            ));
        }
        self.create_backup("before_restore")?;
        fs::copy(&bin_file, &self.bin_path)?;
        fs::copy(&slot_file, &self.slot_path)?;
        Ok(())
    }

    pub fn dump_json(&self, game: &GameData) -> Value {
        let (hours, minutes, seconds) = self.playtime_hms();
        let personas: Vec<Value> = self
            .mc_personas
            .iter()
            .enumerate()
            .map(|(index, persona)| {
                json!({
                    "slot": index + 1,
                    "exists": persona.exists(),
                    "id": persona.id(),
                    "name": game.persona_name(persona.id()),
                    "level": persona.level(),
                    "xp": persona.xp(),
                    "skills": persona.skills(),
                    "stats": persona.stats(),
                })
            })
            .collect();
        let units: Vec<Value> = self
            .units
            .iter()
            .enumerate()
            .map(|(index, unit)| {
                let persona = unit.persona();
                let equipment = if index == 0 {
                    [
                        self.mc_equipment(0),
                        self.mc_equipment(1),
                        self.mc_equipment(2),
                        self.mc_equipment(3),
                    ]
                } else {
                    [
                        unit.equipment(0),
                        unit.equipment(1),
                        unit.equipment(2),
                        unit.equipment(3),
                    ]
                };
                json!({
                    "id": unit.id(),
                    "name": game.char_name(unit.id()),
                    "hp": self.unit_hp(index),
                    "sp": self.unit_sp(index),
                    "equipment": equipment,
                    "persona": {
                        "exists": persona.exists(),
                        "id": persona.id(),
                        "name": game.persona_name(persona.id()),
                        "level": persona.level(),
                        "xp": persona.xp(),
                        "skills": persona.skills(),
                        "stats": persona.stats(),
                    },
                })
            })
            .collect();
        let links: Vec<Value> = self
            .social_links
            .iter()
            .map(|link| {
                json!({
                    "id": link.id(),
                    "name": game.social_name(link.id()),
                    "rank": link.rank(),
                    "affinity": link.affinity(),
                })
            })
            .collect();
        let inventory: Vec<Value> = self
            .inventory
            .iter()
            .enumerate()
            .filter(|(_, count)| **count != 0)
            .map(|(id, count)| json!({"id": id, "name": game.item_name(id as u16), "count": count}))
            .collect();
        json!({
            "file": self
                .bin_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(""),
            "slot": self.slot_number(),
            "lname": self.lname(),
            "fname": self.fname(),
            "money": self.money(),
            "level": self.mc_level(),
            "xp": self.mc_xp(),
            "hp": self.mc_hp(),
            "sp": self.mc_sp(),
            "playtime_hms": [hours, minutes, seconds],
            "difficulty": self.difficulty(),
            "day_serial": self.day_serial(),
            "phase": self.phase(),
            "next_day": self.next_day(),
            "next_phase": self.next_phase(),
            "social_stats": self.social_stats(),
            "party": self.party(),
            "mc_personas": personas,
            "units": units,
            "social_links": links,
            "compendium_registered": self.compendium_count(game),
            "compendium_total": game.personas.len(),
            "inventory": inventory,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LoadedChecks {
    pub header: bool,
    pub save_md5: bool,
    pub slot_md5: bool,
}

pub fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!(
        "{}tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| format!("{value}."))
            .unwrap_or_default()
    ));
    fs::write(&temp, data)?;
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(SaveError::Io(error));
    }
    Ok(())
}

pub fn slot_number_from_path(path: &Path) -> u16 {
    path.file_stem()
        .and_then(|value| value.to_str())
        .and_then(|value| value.strip_prefix("data"))
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0)
}

pub fn list_slots(folder: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("bin")
                && path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| {
                        name.strip_prefix("data")
                            .is_some_and(|value| value.chars().all(|c| c.is_ascii_digit()))
                    })
            {
                result.push(path);
            }
        }
    }
    result.sort_by_key(|path| slot_number_from_path(path));
    result
}

pub fn find_save_folders() -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    for key in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Ok(base) = std::env::var(key) {
            roots.insert(PathBuf::from(base).join("Steam"));
        }
    }
    for path in [
        r"C:\Steam",
        r"D:\Steam",
        r"E:\Steam",
        r"H:\Steam",
        r"H:\SteamLibrary",
    ] {
        roots.insert(PathBuf::from(path));
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        roots.insert(
            PathBuf::from(profile)
                .join("Documents")
                .join("My Games")
                .join("Persona 4 Golden"),
        );
    }
    let mut result = BTreeSet::new();
    for root in roots {
        if root.file_name().and_then(|value| value.to_str()) == Some("Persona 4 Golden") {
            if root.is_dir() {
                result.insert(root);
            }
            continue;
        }
        let userdata = root.join("userdata");
        if let Ok(users) = fs::read_dir(userdata) {
            for user in users.flatten() {
                let remote = user.path().join("1113000").join("remote");
                if remote.is_dir() {
                    result.insert(remote);
                }
            }
        }
    }
    result.into_iter().collect()
}

pub fn slot_summary_from_path(path: &Path) -> String {
    match P4GSave::load(path) {
        Ok(save) => save.slot_summary(),
        Err(_) => path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("data????.bin")
            .to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameData;

    fn first_save() -> Option<PathBuf> {
        for folder in find_save_folders() {
            if let Some(path) = list_slots(&folder).into_iter().next() {
                return Some(path);
            }
        }
        None
    }

    fn copy_save(source: &Path) -> (PathBuf, PathBuf) {
        let folder = std::env::temp_dir().join(format!(
            "p4g-rust-test-{}-{}",
            std::process::id(),
            Local::now().format("%Y%m%d%H%M%S%f")
        ));
        fs::create_dir_all(&folder).unwrap();
        let bin = folder.join(source.file_name().unwrap());
        let slot = bin.with_extension("binslot");
        fs::copy(source, &bin).unwrap();
        fs::copy(source.with_extension("binslot"), &slot).unwrap();
        (bin, folder)
    }

    #[test]
    fn roundtrip_and_checksums() {
        let Some(source) = first_save() else {
            return;
        };
        let game = GameData::load();
        let save = P4GSave::load(&source).unwrap();
        let checks = save.check_loaded();
        assert!(checks.header && checks.save_md5 && checks.slot_md5);
        assert!(save.compendium_count(&game) > 0);
        let mut copy = P4GSave::load(&source).unwrap();
        let (data, slot) = copy.apply(false);
        assert_eq!(data, copy.original_data);
        assert_eq!(slot, copy.original_slot);
        let checks = copy.apply(false);
        let _ = checks;
    }

    #[test]
    fn edits_reload_with_valid_checksums() {
        let Some(source) = first_save() else {
            return;
        };
        let (bin, folder) = copy_save(&source);
        let mut save = P4GSave::load(&bin).unwrap();
        save.set_money(1_234_567);
        save.set_lname("テスト").unwrap();
        save.set_fname("太郎").unwrap();
        save.set_mc_level(55);
        let date = NaiveDate::from_ymd_opt(2011, 5, 5).unwrap();
        save.set_day_serial((date - EPOCH).num_days() as u16);
        save.set_phase(1);
        save.set_playtime(12, 34, 56);
        save.set_rise_lines(200);
        save.save(false).unwrap();
        let loaded = P4GSave::load(&bin).unwrap();
        assert_eq!(loaded.money(), 1_234_567);
        assert_eq!(loaded.full_name(), "テスト 太郎");
        assert_eq!(loaded.mc_level(), 55);
        assert_eq!(
            loaded.game_date(),
            NaiveDate::from_ymd_opt(2011, 5, 5).unwrap()
        );
        assert_eq!(loaded.phase(), 1);
        assert_eq!(loaded.playtime_hms(), (12, 34, 56));
        assert_eq!(loaded.rise_lines(), 200);
        let checks = loaded.check_loaded();
        assert!(checks.header && checks.save_md5 && checks.slot_md5);
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn synthetic_roundtrip_without_game_files() {
        let folder = std::env::temp_dir().join(format!(
            "p4g-rust-synthetic-{}-{}",
            std::process::id(),
            Local::now().format("%Y%m%d%H%M%S%f")
        ));
        fs::create_dir_all(&folder).unwrap();
        let bin = folder.join("data0001.bin");
        let slot = folder.join("data0001.binslot");
        let data = vec![0u8; SAVE_SIZE];
        let mut slot_data = vec![0u8; SLOT_SIZE];
        slot_data[..8].copy_from_slice(SLOT_MAGIC);
        let mut save = P4GSave::from_bytes(data, slot_data, bin.clone(), slot.clone()).unwrap();
        save.set_money(1_234_567);
        save.set_lname("テスト").unwrap();
        save.set_fname("太郎").unwrap();
        save.set_mc_level(55);
        let date = NaiveDate::from_ymd_opt(2011, 5, 5).unwrap();
        save.set_day_serial((date - EPOCH).num_days() as u16);
        save.set_phase(1);
        save.set_playtime(12, 34, 56);
        save.set_rise_lines(200);
        save.save(false).unwrap();

        let loaded = P4GSave::load(&bin).unwrap();
        assert_eq!(loaded.money(), 1_234_567);
        assert_eq!(loaded.full_name(), "テスト 太郎");
        assert_eq!(loaded.mc_level(), 55);
        assert_eq!(loaded.game_date(), date);
        assert_eq!(loaded.phase(), 1);
        assert_eq!(loaded.playtime_hms(), (12, 34, 56));
        assert_eq!(loaded.rise_lines(), 200);
        let checks = loaded.check_loaded();
        assert!(checks.header && checks.save_md5 && checks.slot_md5);
        let _ = fs::remove_dir_all(folder);
    }
}
