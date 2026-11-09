use std::collections::HashMap;

const ITEMS_TSV: &str = include_str!("../assets/data/items.tsv");
const PERSONAS_TSV: &str = include_str!("../assets/data/personas.tsv");
const SKILLS_TSV: &str = include_str!("../assets/data/skills.tsv");
const SOCIAL_TSV: &str = include_str!("../assets/data/social.tsv");
const CHARS_TSV: &str = include_str!("../assets/data/chars.tsv");
const JP_NAMES_TSV: &str = include_str!("../assets/data/jp_names.tsv");

#[derive(Debug, Clone)]
pub struct Item {
    pub id: u16,
    pub name: String,
    pub tag: String,
}

impl Item {
    pub fn base_tag(&self) -> &str {
        self.tag.split('|').next().unwrap_or("")
    }

    pub fn category_key(&self) -> &str {
        if self.tag.contains("HF") {
            "Hollow Forest"
        } else if self.tag.contains("Bugged") {
            "Bugged"
        } else {
            self.base_tag()
        }
    }

    pub fn category_jp(&self) -> &'static str {
        category_jp(self.category_key())
    }

    pub fn display(&self) -> String {
        format!("{:04}  {}", self.id, self.name)
    }
}

#[derive(Debug, Clone)]
pub struct PersonaInfo {
    pub id: u16,
    pub name: String,
    pub arcana: u8,
    pub level: u16,
    pub stats: [u8; 5],
    pub skills: [u16; 4],
    pub exp99: u32,
    pub exp: u32,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub id: u16,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct SocialInfo {
    pub id: u16,
    pub name: String,
    pub jp_name: String,
}

#[derive(Debug, Clone)]
pub struct GameData {
    pub items: Vec<Item>,
    pub personas: Vec<PersonaInfo>,
    pub skills: Vec<Skill>,
    pub social: Vec<SocialInfo>,
    pub chars: HashMap<u16, String>,
    pub items_by_id: HashMap<u16, usize>,
    pub personas_by_id: HashMap<u16, usize>,
    pub skills_by_id: HashMap<u16, usize>,
    pub social_by_id: HashMap<u16, usize>,
    pub jp_items: HashMap<u16, String>,
    pub jp_personas: HashMap<u16, String>,
    pub jp_skills: HashMap<u16, String>,
}

impl GameData {
    pub fn load() -> Self {
        let items = parse_items();
        let personas = parse_personas();
        let skills = parse_skills();
        let social = parse_social();
        let chars = parse_chars();
        let items_by_id = items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id, index))
            .collect();
        let personas_by_id = personas
            .iter()
            .enumerate()
            .map(|(index, persona)| (persona.id, index))
            .collect();
        let skills_by_id = skills
            .iter()
            .enumerate()
            .map(|(index, skill)| (skill.id, index))
            .collect();
        let social_by_id = social
            .iter()
            .enumerate()
            .map(|(index, link)| (link.id, index))
            .collect();
        let (jp_items, jp_personas, jp_skills) = parse_jp_names();
        Self {
            items,
            personas,
            skills,
            social,
            chars,
            items_by_id,
            personas_by_id,
            skills_by_id,
            social_by_id,
            jp_items,
            jp_personas,
            jp_skills,
        }
    }

    pub fn item_name_en(&self, id: u16) -> String {
        self.items_by_id
            .get(&id)
            .map(|index| self.items[*index].name.clone())
            .unwrap_or_else(|| format!("0x{id:03X}"))
    }

    pub fn item_name(&self, id: u16) -> String {
        if let Some(name) = self.jp_items.get(&id) {
            if !is_placeholder_name(name) {
                return name.clone();
            }
        }
        let english = self.item_name_en(id);
        if is_placeholder_name(&english) {
            "未使用".to_owned()
        } else {
            english
        }
    }

    pub fn item(&self, id: u16) -> Option<&Item> {
        self.items_by_id.get(&id).map(|index| &self.items[*index])
    }

    pub fn persona_name_en(&self, id: u16) -> String {
        if id == 0 {
            return "なし".to_owned();
        }
        self.personas_by_id
            .get(&id)
            .map(|index| self.personas[*index].name.clone())
            .unwrap_or_else(|| format!("0x{id:03X}"))
    }

    pub fn persona_name(&self, id: u16) -> String {
        if id == 0 {
            return "なし".to_owned();
        }
        if let Some(name) = self.jp_personas.get(&id) {
            return name.clone();
        }
        let english = self.persona_name_en(id);
        if is_placeholder_name(&english) {
            "未使用".to_owned()
        } else {
            english
        }
    }

    pub fn persona(&self, id: u16) -> Option<&PersonaInfo> {
        self.personas_by_id
            .get(&id)
            .map(|index| &self.personas[*index])
    }

    pub fn persona_display(&self, id: u16) -> String {
        if id == 0 {
            return "000  なし".to_owned();
        }
        format!("{:03}  {}", id, self.persona_name(id))
    }

    pub fn skill_name_en(&self, id: u16) -> String {
        if id == 0 {
            return "なし".to_owned();
        }
        self.skills_by_id
            .get(&id)
            .map(|index| self.skills[*index].name.clone())
            .unwrap_or_else(|| format!("0x{id:03X}"))
    }

    pub fn skill_name(&self, id: u16) -> String {
        if id == 0 {
            return "なし".to_owned();
        }
        if let Some(name) = self.jp_skills.get(&id) {
            return name.clone();
        }
        let english = self.skill_name_en(id);
        if is_placeholder_name(&english) {
            "未使用".to_owned()
        } else {
            english
        }
    }

    pub fn has_jp_skill(&self, id: u16) -> bool {
        self.jp_skills.contains_key(&id)
    }

    pub fn skill_display(&self, id: u16) -> String {
        if id == 0 {
            return "000  なし".to_owned();
        }
        format!("{:03}  {}", id, self.skill_name(id))
    }

    pub fn social_name(&self, id: u16) -> String {
        if id == 0 {
            return "未使用".to_owned();
        }
        if let Some(name) = social_jp(id) {
            return format!("{name}");
        }
        self.social_by_id
            .get(&id)
            .map(|index| self.social[*index].name.clone())
            .unwrap_or_else(|| format!("0x{id:02X}"))
    }

    pub fn social_display(&self, id: u16) -> String {
        self.social_name(id)
    }

    pub fn char_name(&self, id: u16) -> String {
        char_jp(id)
            .map(|name| name.to_owned())
            .or_else(|| self.chars.get(&id).cloned())
            .unwrap_or_else(|| format!("0x{id:02X}"))
    }
}

fn parse_items() -> Vec<Item> {
    rows(ITEMS_TSV)
        .into_iter()
        .filter_map(|fields| {
            if fields.len() < 4 {
                return None;
            }
            Some(Item {
                id: fields[0].parse().ok()?,
                name: fields[2].to_owned(),
                tag: fields[3].to_owned(),
            })
        })
        .collect()
}

fn parse_personas() -> Vec<PersonaInfo> {
    rows(PERSONAS_TSV)
        .into_iter()
        .filter_map(|fields| {
            if fields.len() < 19 {
                return None;
            }
            let id = fields[0].parse::<u16>().ok()?;
            if fields[3] == "FLR" || id == 0 || id > 249 {
                return None;
            }
            Some(PersonaInfo {
                id,
                name: fields[2].to_owned(),
                arcana: fields[5].parse().ok()?,
                level: fields[6].parse().ok()?,
                stats: [
                    fields[7].parse().ok()?,
                    fields[8].parse().ok()?,
                    fields[9].parse().ok()?,
                    fields[10].parse().ok()?,
                    fields[11].parse().ok()?,
                ],
                skills: [
                    fields[14].parse().ok()?,
                    fields[15].parse().ok()?,
                    fields[16].parse().ok()?,
                    fields[17].parse().ok()?,
                ],
                exp99: fields[18].parse().ok()?,
                exp: fields[19].parse().ok()?,
            })
        })
        .collect()
}

fn parse_skills() -> Vec<Skill> {
    rows(SKILLS_TSV)
        .into_iter()
        .filter_map(|fields| {
            if fields.len() < 3 {
                return None;
            }
            Some(Skill {
                id: fields[0].parse().ok()?,
                name: fields[2].to_owned(),
            })
        })
        .collect()
}

fn parse_social() -> Vec<SocialInfo> {
    rows(SOCIAL_TSV)
        .into_iter()
        .filter_map(|fields| {
            if fields.len() < 3 {
                return None;
            }
            let id = fields[0].parse().ok()?;
            Some(SocialInfo {
                id,
                name: fields[2].to_owned(),
                jp_name: social_jp(id).unwrap_or("").to_owned(),
            })
        })
        .collect()
}

fn parse_chars() -> HashMap<u16, String> {
    let mut result = HashMap::new();
    for fields in rows(CHARS_TSV) {
        if fields.len() < 3 {
            continue;
        }
        if let Ok(id) = fields[0].parse::<u16>() {
            result.insert(id, fields[2].to_owned());
        }
    }
    result
}

fn parse_jp_names() -> (
    HashMap<u16, String>,
    HashMap<u16, String>,
    HashMap<u16, String>,
) {
    let mut items = HashMap::new();
    let mut personas = HashMap::new();
    let mut skills = HashMap::new();
    for fields in rows(JP_NAMES_TSV) {
        if fields.len() < 3 {
            continue;
        }
        let Ok(id) = fields[1].parse::<u16>() else {
            continue;
        };
        let name = fields[2].to_owned();
        match fields[0] {
            "item" => {
                items.insert(id, name);
            }
            "persona" => {
                personas.insert(id, name);
            }
            "skill" => {
                skills.insert(id, name);
            }
            _ => {}
        }
    }
    (items, personas, skills)
}

fn is_placeholder_name(name: &str) -> bool {
    let name = name.trim();
    name.is_empty()
        || name.eq_ignore_ascii_case("blank")
        || name.starts_with("0x")
        || name.starts_with("DBG")
        || (name.len() <= 4 && name.chars().all(|c| c.is_ascii_hexdigit()))
}

fn rows(source: &str) -> Vec<Vec<&str>> {
    source
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').collect())
        .collect()
}

pub fn arcana_jp(value: u8) -> &'static str {
    ARCANA_JP.get(value as usize).copied().unwrap_or("")
}

pub fn phase_jp(value: u16) -> &'static str {
    PHASE_JP.get(value as usize).copied().unwrap_or("")
}

pub fn difficulty_jp(value: u32) -> &'static str {
    DIFFICULTY_JP.get(value as usize).copied().unwrap_or("カスタム")
}

pub fn difficulty_save(value: u32) -> &'static str {
    DIFFICULTY_SAVE.get(value as usize).copied().unwrap_or("CUSTOM")
}

pub fn category_jp(key: &str) -> &'static str {
    match key {
        "Weapon" => "武器",
        "Armor" => "防具",
        "Acc" => "アクセサリ",
        "Clothes" => "衣装",
        "Expend" => "消耗品",
        "Material" => "素材",
        "Card" => "スキルカード",
        "Book" => "書物",
        "Veg" => "野菜",
        "Gems" => "宝石",
        "SLink" => "ソーシャルリンク",
        "Shelf" => "置物",
        "Fish" => "魚",
        "Bugs" => "虫",
        "Key" => "重要",
        "Quest" => "依頼",
        "BLN" => "未使用",
        "FLR" => "空き",
        "Hollow Forest" => "迷宮",
        "Bugged" => "不具合",
        _ => "その他",
    }
}

pub fn category_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("", "すべて"),
        ("Weapon", "武器"),
        ("Armor", "防具"),
        ("Acc", "アクセサリ"),
        ("Clothes", "衣装"),
        ("Expend", "消耗品"),
        ("Material", "素材"),
        ("Card", "スキルカード"),
        ("Book", "書物"),
        ("Veg", "野菜"),
        ("Gems", "宝石"),
        ("Fish", "魚"),
        ("Bugs", "虫"),
        ("Key", "重要"),
        ("Quest", "依頼"),
        ("SLink", "ソーシャルリンク"),
        ("Shelf", "置物"),
        ("Hollow Forest", "迷宮"),
        ("Bugged", "不具合"),
        ("BLN", "未使用"),
        ("FLR", "空き"),
    ]
}

pub fn social_jp(id: u16) -> Option<&'static str> {
    match id {
        1 => Some("特別捜査隊"),
        2 => Some("堂島菜々子"),
        3 => Some("久慈川りせ"),
        4 => Some("久慈川りせ (恋人)"),
        5 => Some("天城雪子"),
        6 => Some("天城雪子 (恋人)"),
        7 => Some("花村陽介"),
        8 => Some("堂島遼太郎"),
        9 => Some("上原小夜子"),
        10 => Some("巽完二"),
        11 => Some("里中千枝"),
        12 => Some("里中千枝 (恋人)"),
        13 => Some("キツネ"),
        14 => Some("白鐘直斗"),
        15 => Some("白鐘直斗 (恋人)"),
        16 => Some("運動部 (一条康)"),
        17 => Some("運動部 (長瀬大輔)"),
        18 => Some("小西尚紀"),
        19 => Some("黒田ひさ乃"),
        20 => Some("マーガレット"),
        21 => Some("相沢愛"),
        22 => Some("相沢愛 (恋人)"),
        23 => Some("中島秀"),
        24 => Some("クマ"),
        25 => Some("小沢結美"),
        26 => Some("小沢結美 (恋人)"),
        27 => Some("松永綾音"),
        28 => Some("松永綾音 (恋人)"),
        29 => Some("南絵里"),
        30 => Some("真実を追う者たち"),
        31 => Some("足立透"),
        32 => Some("足立透 (ハンガー)"),
        33 => Some("マリー"),
        34 => Some("マリー (恋人)"),
        _ => None,
    }
}

pub fn char_jp(id: u16) -> Option<&'static str> {
    match id {
        1 => Some("主人公"),
        2 => Some("花村陽介"),
        3 => Some("里中千枝"),
        4 => Some("天城雪子"),
        5 => Some("久慈川りせ"),
        6 => Some("巽完二"),
        7 => Some("白鐘直斗"),
        8 => Some("クマ"),
        _ => None,
    }
}

pub const PHASE_JP: [&str; 6] = ["早朝", "朝", "昼", "午後", "放課後", "夜"];
pub const DIFFICULTY_JP: [&str; 5] = ["カスタム", "SAFETY", "EASY", "NORMAL", "HARD"];
pub const DIFFICULTY_SAVE: [&str; 5] = ["CUSTOM", "SAFETY", "EASY", "NORMAL", "HARD"];
pub const SOCIAL_STAT_NAMES: [(&str, &str); 5] = [
    ("courage", "勇気"),
    ("knowledge", "知識"),
    ("diligence", "忍耐"),
    ("understanding", "寛容"),
    ("expression", "表現"),
];
pub const ARCANA_JP: [&str; 25] = [
    "",
    "愚者",
    "魔術師",
    "女教皇",
    "女帝",
    "皇帝",
    "法王",
    "恋愛",
    "戦車",
    "正義",
    "隠者",
    "運命",
    "剛毅",
    "刑死者",
    "死神",
    "節制",
    "悪魔",
    "塔",
    "星",
    "月",
    "太陽",
    "審判",
    "世界",
    "永劫",
    "異邦人",
];
