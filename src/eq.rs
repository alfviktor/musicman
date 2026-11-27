use crate::library::{optional, save};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io};

#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Gains {
    pub low: i8,
    pub mid: i8,
    pub high: i8,
}

#[derive(Serialize, Deserialize)]
pub struct Presets {
    pub selected: String,
    custom: BTreeMap<String, Gains>,
}
impl Default for Presets {
    fn default() -> Self {
        Self {
            selected: "off".into(),
            custom: BTreeMap::new(),
        }
    }
}
impl Presets {
    pub fn load() -> io::Result<Self> {
        Ok(optional("equalizer.json")?.unwrap_or_default())
    }
    pub fn save(&self) -> io::Result<()> {
        save("equalizer.json", self)
    }
    pub fn names(&self) -> impl Iterator<Item = &str> {
        ["off", "focus"]
            .into_iter()
            .chain(self.custom.keys().map(String::as_str))
    }
    pub fn gains(&self) -> Gains {
        match self.selected.as_str() {
            "focus" => Gains {
                high: -3,
                ..Default::default()
            },
            "off" => Gains::default(),
            name => self.custom.get(name).copied().unwrap_or_default(),
        }
    }
    pub fn edit(&mut self, band: usize, delta: i8) {
        let mut g = self.gains();
        let value = match band {
            0 => &mut g.low,
            1 => &mut g.mid,
            _ => &mut g.high,
        };
        *value = (*value + delta).clamp(-6, 6);
        self.selected = "custom".into();
        self.custom.insert(self.selected.clone(), g);
    }
    pub fn save_as(&mut self, name: &str) -> bool {
        if name.is_empty()
            || name.chars().count() > 32
            || matches!(name, "off" | "focus" | "custom")
        {
            return false;
        }
        self.custom.insert(name.into(), self.gains());
        self.selected = name.into();
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edited_focus_and_named_preset_round_trip() {
        let mut p = Presets {
            selected: "focus".into(),
            ..Default::default()
        };
        assert_eq!(p.gains().high, -3);
        p.edit(2, -1);
        assert!(p.save_as("late night"));
        p.edit(0, 1);
        let mut loaded: Presets = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        loaded.selected = "late night".into();
        assert_eq!((loaded.gains().low, loaded.gains().high), (0, -4));
    }
}
