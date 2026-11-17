//! Commander upgrades: run-long boons for one of a hero's commanders, offered only where they do something for it (a
//! creature's power and keywords, a planeswalker's loyalty, generic mana to take off its cost). Each is an emblem in the
//! hero's command zone at every fight, so the engine applies it as a continuous, cost or replacement effect and never
//! changes the card.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::CampaignError;

/// What a hero's commander is, as far as its upgrades go; read from the card when the deck is chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CommanderFacts {
    /// Its name.
    pub name: String,
    /// Whether it is a creature.
    pub creature: bool,
    /// Whether it is a planeswalker.
    pub planeswalker: bool,
    /// Its printed power, if it has one.
    pub power: Option<i32>,
    /// The generic mana of its cost.
    pub generic: u32,
    /// Its printed keywords by name ("Flying", "First strike").
    pub keywords: Vec<String>,
}

/// A keyword an upgrade may give a creature commander.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Gift {
    /// Flying.
    Flying,
    /// Trample.
    Trample,
    /// Vigilance.
    Vigilance,
    /// Lifelink.
    Lifelink,
    /// Haste.
    Haste,
    /// Menace.
    Menace,
    /// First strike.
    FirstStrike,
    /// Deathtouch.
    Deathtouch,
    /// Hexproof.
    Hexproof,
}

impl Gift {
    /// Every gift, in offer order.
    pub const ALL: [Gift; 9] = [
        Gift::Flying,
        Gift::Trample,
        Gift::Vigilance,
        Gift::Lifelink,
        Gift::Haste,
        Gift::Menace,
        Gift::FirstStrike,
        Gift::Deathtouch,
        Gift::Hexproof,
    ];

    /// The engine's keyword.
    pub fn keyword(self) -> Keyword {
        match self {
            Gift::Flying => Keyword::Flying,
            Gift::Trample => Keyword::Trample,
            Gift::Vigilance => Keyword::Vigilance,
            Gift::Lifelink => Keyword::Lifelink,
            Gift::Haste => Keyword::Haste,
            Gift::Menace => Keyword::Menace,
            Gift::FirstStrike => Keyword::FirstStrike,
            Gift::Deathtouch => Keyword::Deathtouch,
            Gift::Hexproof => Keyword::Hexproof,
        }
    }

    /// Whether it helps only a creature that deals combat damage, so a commander with no power has no use for it.
    pub fn for_combat(self) -> bool {
        !matches!(self, Gift::Haste | Gift::Hexproof)
    }
}

/// What an upgrade does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Boon {
    /// Its cost has this much less generic mana; its coloured mana stays.
    Cheaper(u32),
    /// It has the keyword.
    Keyword(Gift),
    /// It gets +N/+N.
    Pump(i32),
    /// It enters with an additional +1/+1 counter.
    Counter,
    /// It enters with an additional loyalty counter.
    Loyalty,
    /// A bane: its cost has this much more generic mana.
    Tax(u32),
    /// A bane: it gets −N/−N.
    Shrink(i32),
}

/// A commander bane an event's option costs: the inverse of an upgrade, kept for the rest of the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Bane {
    /// The commander costs this much more generic mana.
    Tax(u32),
    /// The commander gets −N/−N.
    Shrink(i32),
}

impl Bane {
    /// The upgrade's boon that carries it.
    pub fn boon(self) -> Boon {
        match self {
            Bane::Tax(n) => Boon::Tax(n),
            Bane::Shrink(n) => Boon::Shrink(n),
        }
    }

    /// Whether only a creature commander can carry it.
    pub fn for_creature(self) -> bool {
        matches!(self, Bane::Shrink(_))
    }

    /// The bane in words about `name`: "your commander costs {1} more to cast", "Atraxa gets −1/−1".
    pub fn words(self, name: &str) -> String {
        match self {
            Bane::Tax(n) => format!("{name} costs {{{n}}} more to cast"),
            Bane::Shrink(n) => format!("{name} gets −{n}/−{n}"),
        }
    }
}

/// One upgrade of one commander.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Upgrade {
    /// The commander's name.
    pub commander: String,
    /// What it does.
    pub boon: Boon,
}

impl Upgrade {
    /// The upgrade in words: "Atraxa costs {1} less", "Atraxa has flying", "Atraxa gets +1/+1".
    pub fn words(&self) -> String {
        let name = &self.commander;
        match &self.boon {
            Boon::Cheaper(n) => format!("{name} costs {{{n}}} less to cast"),
            Boon::Keyword(gift) => {
                format!("{name} has {}", gift.keyword().shown_name().to_lowercase())
            }
            Boon::Pump(n) => format!("{name} gets +{n}/+{n}"),
            Boon::Counter => format!("{name} enters with an additional +1/+1 counter"),
            Boon::Loyalty => format!("{name} enters with an additional loyalty counter"),
            Boon::Tax(n) => Bane::Tax(*n).words(name),
            Boon::Shrink(n) => Bane::Shrink(*n).words(name),
        }
    }

    /// Whether it is a bane an event cost rather than a boon.
    pub fn is_bane(&self) -> bool {
        matches!(self.boon, Boon::Tax(_) | Boon::Shrink(_))
    }

    /// The emblem that carries it in the hero's command zone.
    pub fn definition(&self) -> Result<Arc<CardDefinition>, CampaignError> {
        let own = json!([
            "Commander",
            { "Named": self.commander },
            { "OwnedBy": "You" }
        ]);
        let mut creature = own.clone();
        if let Value::Array(preds) = &mut creature {
            preds.insert(0, json!({ "Type": "Creature" }));
        }
        let ability = match &self.boon {
            Boon::Cheaper(n) => json!({ "CostModifier": {
                "applies_to": [
                    "Spell",
                    "Commander",
                    { "Named": self.commander },
                    { "ControlledBy": "You" }
                ],
                "change": "Reduce",
                "mana": format!("{{{n}}}")
            }}),
            Boon::Tax(n) => json!({ "CostModifier": {
                "applies_to": [
                    "Spell",
                    "Commander",
                    { "Named": self.commander },
                    { "ControlledBy": "You" }
                ],
                "change": "Increase",
                "mana": format!("{{{n}}}")
            }}),
            Boon::Shrink(n) => json!({ "Static": {
                "affected": { "Each": creature },
                "mods": [{ "ModifyPT": { "power": -n, "toughness": -n } }]
            }}),
            Boon::Keyword(gift) => json!({ "Static": {
                "affected": { "Each": creature },
                "mods": [{ "AddKeyword": gift.keyword() }]
            }}),
            Boon::Pump(n) => json!({ "Static": {
                "affected": { "Each": creature },
                "mods": [{ "ModifyPT": { "power": n, "toughness": n } }]
            }}),
            Boon::Counter | Boon::Loyalty => json!({ "Replacement": {
                "when": { "Enters": { "what": own } },
                "by": { "WithCounters": {
                    "counter": if self.boon == Boon::Counter { "+1/+1" } else { "loyalty" },
                    "count": 1
                }}
            }}),
        };
        let ability: Ability = serde_json::from_value(ability)
            .map_err(|e| CampaignError::Content(format!("upgrade {}: {e}", self.words())))?;
        Ok(Arc::new(mtg_engine::w6b_emblems::emblem_definition(
            &format!(
                "{}: {}",
                if self.is_bane() { "Bane" } else { "Upgrade" },
                self.words()
            ),
            &[ability],
        )))
    }
}

/// Every upgrade that would do something for one of `commanders`, given the upgrades the hero already has: cost
/// reductions while generic mana is left to take off, P/T and keywords for a creature (no keyword it has, no combat
/// keyword for one with no power), the loyalty counter for a planeswalker; each upgrade at most once.
pub fn fitting(commanders: &[CommanderFacts], taken: &[Upgrade]) -> Vec<Upgrade> {
    let mut out = Vec::new();
    for facts in commanders {
        let mine: Vec<&Boon> = taken
            .iter()
            .filter(|u| u.commander == facts.name)
            .map(|u| &u.boon)
            .collect();
        let reduced: u32 = mine
            .iter()
            .map(|b| if let Boon::Cheaper(n) = b { *n } else { 0 })
            .sum();
        let left = facts.generic.saturating_sub(reduced);
        let mut boons = Vec::new();
        for n in [1, 2] {
            if left >= n {
                boons.push(Boon::Cheaper(n));
            }
        }
        if facts.creature {
            boons.extend([Boon::Pump(1), Boon::Pump(2), Boon::Counter]);
            let fights = facts.power.is_some_and(|p| p > 0);
            for gift in Gift::ALL {
                let name = gift.keyword().shown_name();
                let has = facts.keywords.iter().any(|k| k.eq_ignore_ascii_case(&name));
                if !has && (fights || !gift.for_combat()) {
                    boons.push(Boon::Keyword(gift));
                }
            }
        }
        if facts.planeswalker {
            boons.push(Boon::Loyalty);
        }
        out.extend(
            boons
                .into_iter()
                // A cost reduction may come again while mana is left; any other upgrade once.
                .filter(|b| matches!(b, Boon::Cheaper(_)) || !mine.contains(&b))
                .map(|boon| Upgrade {
                    commander: facts.name.clone(),
                    boon,
                }),
        );
    }
    out
}
