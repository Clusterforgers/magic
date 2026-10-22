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
    /// Enchant player or enchant opponent: an Aura that enchants a player (CR 303.4).
    EnchantPlayer {
        /// Whether only an opponent of the Aura's controller may be enchanted.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        opponent: bool,
    },
    /// Cycling with its cost.
    Cycling(Cost),
    /// Kicker with its cost.
    Kicker(Cost),
    /// Multikicker with its cost.
    Multikicker(Cost),
    /// Buyback with its cost: the spell returns to its owner's hand as it resolves when the cost was paid (CR 702.27).
    Buyback(Cost),
    /// Entwine with its cost: all modes are chosen when the cost is paid (CR 702.42).
    Entwine(Cost),
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
    /// Changeling: the object is every creature type (CR 702.73).
    Changeling,
    /// Devoid: the object is colorless (CR 702.114).
    Devoid,
    /// Infect: damage to creatures is dealt as -1/-1 counters and to players as poison counters (CR 702.90).
    Infect,
    /// Myriad: when the creature attacks, token copies attack each other opponent (CR 702.116).
    Myriad,
    /// Crew with its power: tapping creatures with that total power turns a Vehicle into a creature (CR 702.122).
    Crew(u32),
    /// Mutate with its cost: the spell may be cast onto a non-Human creature (CR 702.140).
    Mutate(Cost),
    /// Encore with its cost: exiled from a graveyard, a token copy attacks each opponent (CR 702.141).
    Encore(Cost),
    /// Squad with its cost: paid any number of times as the spell is cast, each time a token copy enters (CR 702.157).
    Squad(Cost),
    /// Foretell with its cost: a special action from the hand exiles the card face down, and it is cast later for its foretell cost (CR 702.143).
    Foretell(Cost),
    /// Daybound (CR 702.145): the game becomes day when this permanent appears while it is neither, and the permanent is turned to its night face when it becomes night.
    Daybound,
    /// Nightbound (CR 702.145): the permanent is turned to its day face when it becomes day.
    Nightbound,
    /// Wither (CR 702.80): damage this deals to a creature is dealt in the form of -1/-1 counters.
    Wither,
    /// Toxic N (CR 702.164): players dealt combat damage by this creature also get N poison counters.
    Toxic(u32),
    /// Fear (CR 702.36): can't be blocked except by artifact creatures and/or black creatures.
    Fear,
    /// Intimidate (CR 702.13): can't be blocked except by artifact creatures and/or creatures that share a color with it.
    Intimidate,
    /// Landwalk (CR 702.14): can't be blocked as long as defending player controls a land the selector names (Islandwalk: an Island).
    Landwalk(Selector),
    /// Skulk (CR 702.118): can't be blocked by creatures with greater power.
    Skulk,
    /// Shadow (CR 702.28): can block or be blocked by only creatures with shadow.
    Shadow,
    /// Split second (CR 702.61): while this spell is on the stack, players can't cast spells or activate abilities that aren't mana abilities.
    SplitSecond,
    /// Undaunted (CR 702.125): this spell costs {1} less to cast for each opponent.
    Undaunted,
    /// Partner with a text (`Partner—Friends forever`, `Partner—Character select`): two commanders that both have the
    /// same text pair (CR 702.124).
    PartnerGroup(String),
    /// Doctor's companion: the second commander beside a legendary Time Lord Doctor (CR 702.124).
    DoctorsCompanion,
    /// Choose a Background: the second commander may be a Background enchantment (CR 702.124).
    ChooseABackground,
    /// Unearth with its cost: from a graveyard at sorcery speed the card returns with haste and is exiled at the next end step (CR 702.84).
    Unearth(Cost),
    /// Escape with its cost: the card may be cast from a graveyard for it, which includes exiling other cards of the graveyard (CR 702.138).
    Escape(Cost),
    /// Rebound: a spell cast from its owner's hand is exiled as it resolves and cast free at their next upkeep (CR 702.88).
    Rebound,
}

impl Keyword {
    /// Scryfall's spelling of every keyword in this enum, for matching a record's `keywords`.
    pub const NAMES: &[&str] = &[
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
        "Enchant player",
        "Cycling",
        "Kicker",
        "Multikicker",
        "Buyback",
        "Entwine",
        "Flashback",
        "Madness",
        "Morph",
        "Disguise",
        "Suspend",
        "Evoke",
        "Overload",
        "Partner with",
        "Changeling",
        "Devoid",
        "Infect",
        "Myriad",
        "Crew",
        "Mutate",
        "Encore",
        "Squad",
        "Foretell",
        "Daybound",
        "Nightbound",
        "Wither",
        "Toxic",
        "Fear",
        "Intimidate",
        "Landwalk",
        "Skulk",
        "Shadow",
        "Split second",
        "Undaunted",
        "Partner group",
        "Doctor's companion",
        "Choose a Background",
        "Unearth",
        "Escape",
        "Rebound",
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
            Keyword::EnchantPlayer { .. } => "Enchant player",
            Keyword::Cycling(_) => "Cycling",
            Keyword::Kicker(_) => "Kicker",
            Keyword::Multikicker(_) => "Multikicker",
            Keyword::Buyback(_) => "Buyback",
            Keyword::Entwine(_) => "Entwine",
            Keyword::Flashback(_) => "Flashback",
            Keyword::Madness(_) => "Madness",
            Keyword::Morph(_) => "Morph",
            Keyword::Disguise(_) => "Disguise",
            Keyword::Suspend { .. } => "Suspend",
            Keyword::Evoke(_) => "Evoke",
            Keyword::Overload(_) => "Overload",
            Keyword::PartnerWith(_) => "Partner with",
            Keyword::Changeling => "Changeling",
            Keyword::Devoid => "Devoid",
            Keyword::Infect => "Infect",
            Keyword::Myriad => "Myriad",
            Keyword::Crew(_) => "Crew",
            Keyword::Mutate(_) => "Mutate",
            Keyword::Encore(_) => "Encore",
            Keyword::Squad(_) => "Squad",
            Keyword::Foretell(_) => "Foretell",
            Keyword::Daybound => "Daybound",
            Keyword::Nightbound => "Nightbound",
            Keyword::Wither => "Wither",
            Keyword::Toxic(_) => "Toxic",
            Keyword::Fear => "Fear",
            Keyword::Intimidate => "Intimidate",
            Keyword::Landwalk(_) => "Landwalk",
            Keyword::Skulk => "Skulk",
            Keyword::Shadow => "Shadow",
            Keyword::SplitSecond => "Split second",
            Keyword::Undaunted => "Undaunted",
            Keyword::PartnerGroup(_) => "Partner group",
            Keyword::DoctorsCompanion => "Doctor's companion",
            Keyword::ChooseABackground => "Choose a Background",
            Keyword::Unearth(_) => "Unearth",
            Keyword::Escape(_) => "Escape",
            Keyword::Rebound => "Rebound",
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
            Keyword::EnchantPlayer { opponent: false },
            Keyword::Cycling(cost.clone()),
            Keyword::Kicker(cost.clone()),
            Keyword::Multikicker(cost.clone()),
            Keyword::Buyback(cost.clone()),
            Keyword::Entwine(cost.clone()),
            Keyword::Flashback(cost.clone()),
            Keyword::Madness(cost.clone()),
            Keyword::Morph(cost.clone()),
            Keyword::Disguise(cost.clone()),
            Keyword::Suspend {
                time: 3,
                cost: cost.clone(),
            },
            Keyword::Evoke(cost.clone()),
            Keyword::Overload(cost.clone()),
            Keyword::PartnerWith("Some Card".into()),
            Keyword::Changeling,
            Keyword::Devoid,
            Keyword::Infect,
            Keyword::Myriad,
            Keyword::Crew(3),
            Keyword::Mutate(cost.clone()),
            Keyword::Encore(cost.clone()),
            Keyword::Squad(cost.clone()),
            Keyword::Foretell(cost.clone()),
            Keyword::Daybound,
            Keyword::Nightbound,
            Keyword::Wither,
            Keyword::Toxic(1),
            Keyword::Fear,
            Keyword::Intimidate,
            Keyword::Landwalk(Selector::default()),
            Keyword::Skulk,
            Keyword::Shadow,
            Keyword::SplitSecond,
            Keyword::Undaunted,
            Keyword::PartnerGroup("Friends forever".into()),
            Keyword::DoctorsCompanion,
            Keyword::ChooseABackground,
            Keyword::Unearth(cost.clone()),
            Keyword::Escape(cost),
            Keyword::Rebound,
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

    #[test]
    fn names_list_every_variant_once_in_any_case() {
        // Rust cannot count the variants of an enum, so the source is counted: rustfmt keeps one variant per
        // line at four spaces, while docs, attributes and the fields of a struct variant do not start with a capital there.
        let source = include_str!("keyword.rs");
        let body = source
            .split("pub enum Keyword {")
            .nth(1)
            .and_then(|rest| rest.split("\n}\n").next())
            .unwrap_or_default();
        let variants = body
            .lines()
            .filter_map(|line| line.strip_prefix("    "))
            .filter(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()))
            .count();
        assert!(variants > 0, "the enum was not found in the source");
        assert_eq!(Keyword::NAMES.len(), variants);
        let lowercase: std::collections::BTreeSet<_> = Keyword::NAMES
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();
        assert_eq!(lowercase.len(), Keyword::NAMES.len());
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
