//! Goal: all 40 corpus cards assemble into the characteristics their imported record prints whatever the engine runs, rules are attached only when the support record says the engine runs them, and a card comes out Verified, Partial or Manual as the capabilities allow.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;

use mtg_cards::assemble::{FACES, Lowered, PLAIN_MANA_ABILITY, X_COST, assemble, lower};
use mtg_cards::load::Library;
use mtg_cards::mana::ManaCostX;
use mtg_cards::support::{Caps, Support};
use mtg_cards::{AbilityDef, ManaAbility};
use mtg_core::color::{Color, ColorSet};
use mtg_core::mana::ManaType;
use mtg_import::scryfall::CardRecord;
use rstest::rstest;

use common::{cards_dir, printed_of, records};

fn library() -> Library {
    Library::load_dir(&cards_dir()).unwrap_or_else(|errors| panic!("{errors}"))
}

fn letters(text: &str) -> ColorSet {
    text.chars().filter_map(Color::from_symbol).collect()
}

/// The colors a record prints for its front face, found without the code under test: the
/// face's own colors; else the card's colors when it has one face; else the letters of its cost.
fn printed_colors(record: &CardRecord) -> ColorSet {
    if !record.front.colors.is_empty() {
        record.front.colors
    } else if !record.is_multi_faced() {
        record.colors
    } else {
        let inside_braces = record.front.mana_cost.replace(['{', '}'], "");
        letters(&inside_braces)
    }
}

fn owned_words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

fn words(type_line: &str) -> (BTreeSet<String>, Vec<String>) {
    let (head, tail) = type_line.split_once('\u{2014}').unwrap_or((type_line, ""));
    (owned_words(head).into_iter().collect(), owned_words(tail))
}

/// The mana ability a definition carries (the only kind it has in `abilities`).
fn mana_of(ability: AbilityDef) -> ManaAbility {
    let AbilityDef::Mana(mana) = ability;
    mana
}

fn caps_of(names: &[&str]) -> Caps {
    Caps::from_names(names.iter().copied())
}

#[rstest]
#[case::nothing(Caps::none())]
#[case::mana_abilities(caps_of(&[PLAIN_MANA_ABILITY]))]
#[case::everything(Caps::everything())]
fn the_printed_side_of_every_corpus_card_equals_its_record(#[case] caps: Caps) {
    let records = records();
    assert_eq!(records.len(), 157);
    for record in records {
        let name = &record.name;
        let front = &record.front;
        let shown = assemble(&printed_of(&record), None, &caps).definition;

        assert_eq!(shown.name, front.name, "{name}");
        let cost: Option<ManaCostX> =
            (!front.mana_cost.is_empty()).then(|| front.mana_cost.parse().unwrap());
        assert_eq!(
            shown.mana_cost.as_ref(),
            cost.as_ref().map(ManaCostX::rest),
            "{name}: cost"
        );
        assert_eq!(
            shown.x_in_cost,
            cost.as_ref().map_or(0, ManaCostX::x),
            "{name}: X"
        );

        let (head, tail) = words(&front.type_line);
        let kinds: BTreeSet<String> = shown
            .supertypes
            .iter()
            .map(|s| format!("{s:?}"))
            .chain(shown.types.iter().map(|t| format!("{t:?}")))
            .collect();
        assert_eq!(kinds, head, "{name}: supertypes and types");
        let subtypes: Vec<String> = shown.subtypes.iter().map(|s| s.0.clone()).collect();
        assert_eq!(subtypes, tail, "{name}: subtypes");

        let text = |value: Option<u32>| value.map(|n| n.to_string());
        // A `*` is no number: a characteristic-defining ability gives it its value.
        let number = |stat: &Option<String>| stat.clone().filter(|stat| stat != "*");
        assert_eq!(
            shown.power.map(|n| n.to_string()),
            number(&front.power),
            "{name}: power"
        );
        assert_eq!(
            shown.toughness.map(|n| n.to_string()),
            number(&front.toughness),
            "{name}: toughness"
        );
        assert_eq!(text(shown.loyalty), front.loyalty, "{name}: loyalty");
        assert_eq!(text(shown.defense), front.defense, "{name}: defense");
        assert_eq!(shown.colors(), printed_colors(&record), "{name}: colors");
    }
}

#[test]
fn the_cards_with_a_number_to_check_are_in_the_corpus() {
    let by_name = |wanted: &str| {
        let record = records().into_iter().find(|r| r.name == wanted).unwrap();
        assemble(&printed_of(&record), None, &Caps::none()).definition
    };
    let jace = by_name("Jace, the Mind Sculptor");
    assert_eq!((jace.loyalty, jace.power), (Some(3), None));
    let fireball = by_name("Fireball");
    assert_eq!((fireball.x_in_cost, fireball.mana_value()), (1, 1));
    assert_eq!(fireball.mana_cost.unwrap().to_string(), "{R}");
    let fire = by_name("Fire // Ice");
    assert_eq!(
        fire.name, "Fire",
        "a card with several faces is its front face"
    );
    let delver = by_name("Delver of Secrets // Insectile Aberration");
    assert_eq!((delver.power, delver.toughness), (Some(1), Some(1)));
}

fn state_of(library: &Library, name: &str, caps: &Caps) -> Support {
    let record = records().into_iter().find(|r| r.name == name).unwrap();
    library.assemble(&printed_of(&record), caps).support.support
}

#[test]
fn only_what_the_engine_runs_is_verified() {
    let library = library();
    let caps = caps_of(&[PLAIN_MANA_ABILITY]);
    let mut verified = Vec::new();
    for record in records() {
        let assembled = library.assemble(&printed_of(&record), &caps);
        if assembled.support.support == Support::Verified {
            verified.push(record.name.clone());
        }
        // The same record the deck check computes, so the two cannot disagree.
        assert_eq!(
            assembled.support,
            library.support(&printed_of(&record), &caps)
        );
    }
    verified.sort();
    assert_eq!(verified, ["Forest", "Grizzly Bears", "Llanowar Elves"]);
    assert_eq!(state_of(&library, "Sol Ring", &caps), Support::Manual);
    assert_eq!(state_of(&library, "Gaea's Cradle", &caps), Support::Manual);
}

#[test]
fn a_vanilla_or_basic_card_is_verified_without_a_rules_file() {
    let no_files = Library::from_sources(&[]).unwrap_or_else(|errors| panic!("{errors}"));
    for caps in [Caps::none(), caps_of(&[PLAIN_MANA_ABILITY])] {
        let mut verified = Vec::new();
        for record in records() {
            let assembled = no_files.assemble(&printed_of(&record), &caps);
            if assembled.support.support == Support::Verified {
                verified.push(record.name.clone());
            }
            if record.name == "Forest" {
                // The land type gives the mana ability, so the card plays with no file at all.
                assert_eq!(
                    assembled.definition.abilities,
                    [AbilityDef::Mana(ManaAbility {
                        produces: ManaType::Green
                    })]
                );
            }
        }
        verified.sort();
        assert_eq!(
            verified,
            ["Forest", "Grizzly Bears"],
            "every other card of the corpus has rules text"
        );
    }
}

#[test]
fn rules_are_attached_only_when_the_support_record_says_the_engine_runs_them() {
    let library = library();
    let caps_to_try = [
        Caps::none(),
        caps_of(&[PLAIN_MANA_ABILITY]),
        caps_of(&["Keyword.Vigilance", "Keyword.Deathtouch", "Keyword.Haste"]),
        Caps::everything(),
    ];
    for caps in &caps_to_try {
        for record in records() {
            let printed = printed_of(&record);
            let assembled = library.assemble(&printed, caps);
            let shown = &assembled.definition;
            // What the land types give is part of the printed side, with or without rules.
            let types = &printed.faces[0].type_line;
            let intrinsic = if types.contains("Land") {
                types
                    .split(|c: char| !c.is_alphabetic())
                    .filter(|word| {
                        ["Plains", "Island", "Swamp", "Mountain", "Forest"].contains(word)
                    })
                    .count()
            } else {
                0
            };
            let carried = &shown.abilities[intrinsic..];
            let state = assembled.support.support;
            if matches!(state, Support::Verified | Support::Partial) {
                let rules = library.card(&printed.oracle_id).unwrap();
                let lowered: Vec<Lowered> =
                    rules.faces[0].abilities.iter().filter_map(lower).collect();
                assert!(
                    carried
                        .iter()
                        .all(|a| lowered.contains(&Lowered::Mana(mana_of(*a)))),
                    "{}: carries more than its rules",
                    record.name
                );
                assert!(
                    shown
                        .rules
                        .iter()
                        .all(|r| lowered.contains(&Lowered::Rules(r.clone()))),
                    "{}: carries more rules than it has",
                    record.name
                );
            } else {
                assert!(
                    carried.is_empty() && shown.rules.is_empty(),
                    "{} is {state:?} and carries rules",
                    record.name
                );
            }
        }
    }
}

#[test]
fn an_engine_that_runs_some_of_a_card_makes_it_partial() {
    let library = library();
    let keywords = caps_of(&["Keyword.Vigilance", "Keyword.Deathtouch", "Keyword.Haste"]);
    assert_eq!(
        state_of(&library, "Questing Beast", &keywords),
        Support::Partial
    );
    assert_eq!(
        state_of(&library, "Questing Beast", &Caps::none()),
        Support::Manual
    );
    assert_eq!(
        state_of(&library, "Questing Beast", &Caps::everything()),
        Support::Partial,
        "its three clauses stay left to the table"
    );
}

#[test]
fn the_llanowar_elves_ability_arrives_as_the_engine_ability() {
    let library = library();
    let elves = records()
        .into_iter()
        .find(|r| r.name == "Llanowar Elves")
        .unwrap();
    let caps = caps_of(&[PLAIN_MANA_ABILITY]);
    let assembled = library.assemble(&printed_of(&elves), &caps);
    assert_eq!(
        assembled.definition.abilities,
        [AbilityDef::Mana(ManaAbility {
            produces: ManaType::Green
        })]
    );
    let off = library.assemble(&printed_of(&elves), &Caps::none());
    assert!(
        off.definition.abilities.is_empty(),
        "no rules without the engine"
    );
    assert_eq!(off.definition.power, assembled.definition.power);
}

#[test]
fn faces_and_x_the_engine_cannot_play_are_clauses_until_it_can() {
    let library = library();
    let base = caps_of(&[PLAIN_MANA_ABILITY]);
    let multi = [
        "Fire // Ice",
        "Delver of Secrets // Insectile Aberration",
        "Bonecrusher Giant // Stomp",
    ];
    for name in multi {
        let record = records().into_iter().find(|r| r.name == name).unwrap();
        let assembled = library.assemble(&printed_of(&record), &base);
        assert_eq!(assembled.support.support, Support::Manual, "{name}");
        let clause = assembled
            .support
            .clauses
            .iter()
            .find(|c| c.face == 1 && c.reason.contains("front face"))
            .unwrap_or_else(|| panic!("{name}: {:?}", assembled.support.clauses));
        assert!(
            clause.span.end > 0,
            "{name}: the clause covers the back face's text"
        );
    }
    let fireball = records()
        .into_iter()
        .find(|r| r.name == "Fireball")
        .unwrap();
    let blocked = library.assemble(&printed_of(&fireball), &base);
    assert!(
        blocked
            .support
            .clauses
            .iter()
            .any(|c| c.reason.contains("{X}"))
    );
    let ready = caps_of(&[PLAIN_MANA_ABILITY, X_COST, FACES]);
    let lifted = library.assemble(&printed_of(&fireball), &ready);
    assert!(
        !lifted
            .support
            .clauses
            .iter()
            .any(|c| c.reason.contains("{X}"))
    );
    assert_eq!(
        lifted.support.support,
        Support::Manual,
        "Fireball still needs a Spell ability"
    );
}
