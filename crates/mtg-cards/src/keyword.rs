//! Keywords: a card names them with their parameters; the engine implements each once.

use serde::{Deserialize, Serialize};

use crate::cost::Cost;
use crate::refs::Selector;

/// A keyword ability with its parameters (CR 702).
///
/// The engine holds one rules value per variant, each in its own file, so a keyword task never
/// edits this enum (`docs/cards-dsl.md`, Keywords).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Keyword {
    /// Flash: may be cast any time the player could cast an instant.
    Flash,
    /// Flying.
    Flying,
    /// Reach.
    Reach,
    /// First strike.
    FirstStrike,
    /// Double strike.
    DoubleStrike,
    /// Deathtouch.
    Deathtouch,
    /// Trample.
    Trample,
    /// Lifelink.
    Lifelink,
    /// Vigilance.
    Vigilance,
    /// Haste.
    Haste,
    /// Menace.
    Menace,
    /// Defender.
    Defender,
    /// Indestructible.
    Indestructible,
    /// Hexproof.
    Hexproof,
    /// Shroud: can't be the target of any spell or ability.
    Shroud,
    /// Protection from a quality (CR 702.16): the sources the selector matches can't damage, enchant, equip, block or
    /// target the holder. The empty selector is protection from everything.
    Protection(Selector),
    /// Prowess.
    Prowess,
    /// Convoke.
    Convoke,
    /// Delve.
    Delve,
    /// Cascade.
    Cascade,
    /// Storm.
    Storm,
    /// Partner.
    Partner,
    /// Companion.
    Companion,
    /// Battle cry.
    BattleCry,
    /// Fuse: both halves of a split card may be cast from hand together.
    Fuse,
    /// Ward with its cost.
    Ward(Cost),
    /// Equip with its cost.
    Equip(Cost),
    /// Reconfigure with its cost.
    Reconfigure(Cost),
    /// Living weapon: when the Equipment enters, a 0/0 black Phyrexian Germ is created and the Equipment attached to it.
    LivingWeapon,
    /// Enchant: what an Aura may enchant.
    Enchant(Selector),
    /// Cycling with its cost.
    Cycling(Cost),
    /// Kicker with its cost.
    Kicker(Cost),
    /// Multikicker with its cost.
    Multikicker(Cost),
    /// Flashback with its cost.
    Flashback(Cost),
    /// Madness with its cost.
    Madness(Cost),
    /// Morph with its cost.
    Morph(Cost),
    /// Disguise with its cost.
    Disguise(Cost),
    /// Suspend: the number of time counters and the cost.
    Suspend {
        /// Time counters put on the card in exile.
        time: u32,
        /// The suspend cost.
        cost: Cost,
    },
    /// Evoke with its cost.
    Evoke(Cost),
    /// Overload with its cost.
    Overload(Cost),
    /// Partner with the named card.
    PartnerWith(String),
}

impl Keyword {
    /// Scryfall's spelling of every keyword in this enum, for matching a record's `keywords`.
    pub const NAMES: [&'static str; 41] = [
        "Flash",
        "Flying",
        "Reach",
        "First strike",
        "Double strike",
        "Deathtouch",
        "Trample",
        "Lifelink",
        "Vigilance",
        "Haste",
        "Menace",
        "Defender",
        "Indestructible",
        "Hexproof",
        "Shroud",
        "Protection",
        "Prowess",
        "Convoke",
        "Delve",
        "Cascade",
        "Storm",
        "Partner",
        "Companion",
        "Battle cry",
        "Fuse",
        "Ward",
        "Equip",
        "Reconfigure",
        "Living weapon",
        "Enchant",
        "Cycling",
        "Kicker",
        "Multikicker",
        "Flashback",
        "Madness",
        "Morph",
        "Disguise",
        "Suspend",
        "Evoke",
        "Overload",
        "Partner with",
    ];

    /// The keyword as Scryfall spells it (`First strike`).
    pub const fn name(&self) -> &'static str {
        match self {
            Keyword::Flash => "Flash",
            Keyword::Flying => "Flying",
            Keyword::Reach => "Reach",
            Keyword::FirstStrike => "First strike",
            Keyword::DoubleStrike => "Double strike",
            Keyword::Deathtouch => "Deathtouch",
            Keyword::Trample => "Trample",
            Keyword::Lifelink => "Lifelink",
            Keyword::Vigilance => "Vigilance",
            Keyword::Haste => "Haste",
            Keyword::Menace => "Menace",
            Keyword::Defender => "Defender",
            Keyword::Indestructible => "Indestructible",
            Keyword::Hexproof => "Hexproof",
            Keyword::Shroud => "Shroud",
            Keyword::Protection(_) => "Protection",
            Keyword::Prowess => "Prowess",
            Keyword::Convoke => "Convoke",
            Keyword::Delve => "Delve",
            Keyword::Cascade => "Cascade",
            Keyword::Storm => "Storm",
            Keyword::Partner => "Partner",
            Keyword::Companion => "Companion",
            Keyword::BattleCry => "Battle cry",
            Keyword::Fuse => "Fuse",
            Keyword::Ward(_) => "Ward",
            Keyword::Equip(_) => "Equip",
            Keyword::Reconfigure(_) => "Reconfigure",
            Keyword::LivingWeapon => "Living weapon",
            Keyword::Enchant(_) => "Enchant",
            Keyword::Cycling(_) => "Cycling",
            Keyword::Kicker(_) => "Kicker",
            Keyword::Multikicker(_) => "Multikicker",
            Keyword::Flashback(_) => "Flashback",
            Keyword::Madness(_) => "Madness",
            Keyword::Morph(_) => "Morph",
            Keyword::Disguise(_) => "Disguise",
            Keyword::Suspend { .. } => "Suspend",
            Keyword::Evoke(_) => "Evoke",
            Keyword::Overload(_) => "Overload",
            Keyword::PartnerWith(_) => "Partner with",
        }
    }

    /// Whether Scryfall's keyword text names a keyword of this enum (case-insensitive).
    pub fn is_known_name(printed: &str) -> bool {
        Keyword::NAMES
            .iter()
            .any(|name| name.eq_ignore_ascii_case(printed))
    }

    /// Whether this is a kicker or multikicker keyword.
    pub const fn is_kicker(&self) -> bool {
        matches!(self, Keyword::Kicker(_) | Keyword::Multikicker(_))
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every keyword has a unique Scryfall name listed in `NAMES`, and keywords serialize as plain words or single-key maps.
    use super::*;
    use rstest::rstest;

    fn samples() -> Vec<Keyword> {
        let cost = Cost::default();
        vec![
            Keyword::Flash,
            Keyword::Flying,
            Keyword::Reach,
            Keyword::FirstStrike,
            Keyword::DoubleStrike,
            Keyword::Deathtouch,
            Keyword::Trample,
            Keyword::Lifelink,
            Keyword::Vigilance,
            Keyword::Haste,
            Keyword::Menace,
            Keyword::Defender,
            Keyword::Indestructible,
            Keyword::Hexproof,
            Keyword::Shroud,
            Keyword::Protection(Selector::default()),
            Keyword::Prowess,
            Keyword::Convoke,
            Keyword::Delve,
            Keyword::Cascade,
            Keyword::Storm,
            Keyword::Partner,
            Keyword::Companion,
            Keyword::BattleCry,
            Keyword::Fuse,
            Keyword::Ward(cost.clone()),
            Keyword::Equip(cost.clone()),
            Keyword::Reconfigure(cost.clone()),
            Keyword::LivingWeapon,
            Keyword::Enchant(Selector::default()),
            Keyword::Cycling(cost.clone()),
            Keyword::Kicker(cost.clone()),
            Keyword::Multikicker(cost.clone()),
            Keyword::Flashback(cost.clone()),
            Keyword::Madness(cost.clone()),
            Keyword::Morph(cost.clone()),
            Keyword::Disguise(cost.clone()),
            Keyword::Suspend {
                time: 3,
                cost: cost.clone(),
            },
            Keyword::Evoke(cost.clone()),
            Keyword::Overload(cost),
            Keyword::PartnerWith("Some Card".into()),
        ]
    }

    #[test]
    fn names_are_unique_and_listed() {
        let names: Vec<_> = samples().iter().map(Keyword::name).collect();
        assert_eq!(names, Keyword::NAMES);
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
    }

    #[rstest]
    #[case("flying", true)]
    #[case("First Strike", true)]
    #[case("Battle Cry", true)]
    #[case("Landfall", false)]
    #[case("Scry", false)]
    fn printed_names_match_case_insensitively(#[case] printed: &str, #[case] known: bool) {
        assert_eq!(Keyword::is_known_name(printed), known);
    }

    #[test]
    fn keywords_round_trip_through_json() {
        for keyword in samples() {
            let json = serde_json::to_string(&keyword).unwrap();
            assert_eq!(serde_json::from_str::<Keyword>(&json).unwrap(), keyword);
        }
        assert_eq!(
            serde_json::to_string(&Keyword::Flying).unwrap(),
            "\"Flying\""
        );
    }
}
