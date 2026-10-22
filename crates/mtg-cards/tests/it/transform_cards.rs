//! Goal: a transforming double-faced card is played with both faces when the engine plays faces and its transform when it runs `Effect.Transform` (else the support walker names the missing primitive, never the faces), daybound and nightbound are named as keywords, and a split card with fuse is assembled with the spell of both halves: their targets in order and the effects of the left half and then the right half with the right half's targets renumbered after the left half's.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_cards::ability::Ability;
use mtg_cards::assemble::FACES;
use mtg_cards::definition::Layout;
use mtg_cards::keyword::Keyword;
use mtg_cards::load::Library;
use mtg_cards::support::{Caps, Support};
use mtg_cards::validate::used_primitives;
use rstest::rstest;

use common::{Corpus, printed_of, records};

fn library() -> Library {
    Corpus::repo().library()
}

fn caps_without(missing: &[&str]) -> Caps {
    let all = [
        FACES,
        "Ability.Activated",
        "Ability.Triggered",
        "Ability.Spell",
        "Cost.Mana",
        "Effect.PutCounters",
        "Effect.RemoveCounters",
        "Effect.If",
        "Effect.Transform",
        "Effect.Destroy",
        "Keyword.Defender",
        "Keyword.Trample",
        "Keyword.Daybound",
        "Keyword.Nightbound",
        "Keyword.Fuse",
    ];
    Caps::from_names(all.into_iter().filter(|name| !missing.contains(name)))
}

fn state_of(name: &str, caps: &Caps) -> (Support, Vec<String>) {
    let record = records().into_iter().find(|r| r.name == name).unwrap();
    let assembled = library().assemble(&printed_of(&record), caps);
    let reasons = assembled
        .support
        .clauses
        .iter()
        .map(|clause| clause.reason.clone())
        .collect();
    (assembled.support.support, reasons)
}

#[rstest]
#[case::everything_runs("Ludevic's Test Subject // Ludevic's Abomination", &[], Support::Verified)]
#[case::no_transform("Ludevic's Test Subject // Ludevic's Abomination", &["Effect.Transform"], Support::Partial)]
#[case::no_daybound("Tavern Ruffian // Tavern Smasher", &["Keyword.Daybound"], Support::Partial)]
#[case::no_nightbound("Tavern Ruffian // Tavern Smasher", &["Keyword.Nightbound"], Support::Partial)]
#[case::no_fuse("Wear // Tear", &["Keyword.Fuse"], Support::Partial)]
fn the_walker_names_the_primitive_that_is_missing(
    #[case] name: &str,
    #[case] missing: &[&str],
    #[case] expected: Support,
) {
    let (state, reasons) = state_of(name, &caps_without(missing));
    assert_eq!(state, expected, "{reasons:?}");
    for reason in &reasons {
        assert!(
            !reason.contains("front face"),
            "the faces are played: {reason}"
        );
    }
    if !missing.is_empty() {
        assert!(
            reasons
                .iter()
                .any(|reason| missing.iter().any(|m| reason.contains(m))),
            "{missing:?} is named: {reasons:?}"
        );
    }
}

#[test]
fn without_faces_the_back_face_is_a_clause_of_its_own() {
    let (state, reasons) = state_of(
        "Ludevic's Test Subject // Ludevic's Abomination",
        &caps_without(&[FACES]),
    );
    assert_eq!(state, Support::Manual);
    assert!(
        reasons.iter().any(|reason| reason.contains("front face")),
        "{reasons:?}"
    );
}

#[test]
fn the_primitives_of_a_transform_are_named() {
    let library = library();
    let card = library
        .card_by_name("Ludevic's Test Subject // Ludevic's Abomination")
        .unwrap();
    let used: std::collections::BTreeSet<String> = card
        .abilities()
        .flat_map(|(_, ability)| used_primitives(ability))
        .collect();
    assert!(used.contains("Effect.Transform"), "{used:?}");
    let ruffian = library
        .card_by_name("Tavern Ruffian // Tavern Smasher")
        .unwrap();
    let used: std::collections::BTreeSet<String> = ruffian
        .abilities()
        .flat_map(|(_, ability)| used_primitives(ability))
        .collect();
    assert!(
        used.contains("Keyword.Daybound") && used.contains("Keyword.Nightbound"),
        "{used:?}"
    );
}

#[test]
fn a_transforming_card_is_its_front_face_with_both_faces_inside() {
    let record = records()
        .into_iter()
        .find(|r| r.name == "Delver of Secrets // Insectile Aberration")
        .unwrap();
    let assembled = library().assemble(&printed_of(&record), &mtg_cards::Caps::everything());
    let definition = assembled.definition;
    assert_eq!(definition.layout, Layout::Transform);
    assert_eq!(definition.name, "Delver of Secrets");
    let names: Vec<&str> = definition
        .faces
        .iter()
        .map(|face| face.name.as_str())
        .collect();
    assert_eq!(names, ["Delver of Secrets", "Insectile Aberration"]);
    assert_eq!(definition.face(1).power, Some(3));
}

#[test]
fn a_split_card_with_fuse_carries_the_spell_of_both_halves() {
    let record = records()
        .into_iter()
        .find(|r| r.name == "Wear // Tear")
        .unwrap();
    let assembled = library().assemble(&printed_of(&record), &Caps::everything());
    let whole = assembled.definition;
    assert_eq!(whole.layout, Layout::Split);
    let [spell] = whole.rules.as_slice() else {
        panic!("one fused spell: {:?}", whole.rules);
    };
    let Ability::Spell { targets, effects } = &**spell else {
        panic!("a spell");
    };
    assert_eq!(targets.len(), 2);
    let json = serde_json::to_string(effects).unwrap();
    assert_eq!(
        json,
        r#"[{"Destroy":{"what":{"Target":0}}},{"Destroy":{"what":{"Target":1}}}]"#
    );
    // Each half keeps its own spell and fuse.
    for half in &whole.faces {
        assert!(
            half.rules
                .iter()
                .any(|rules| matches!(**rules, Ability::Keyword(Keyword::Fuse)))
        );
    }
    // A split card without fuse (Fire // Ice) has no fused spell.
    let record = records()
        .into_iter()
        .find(|r| r.name == "Fire // Ice")
        .unwrap();
    let plain = library().assemble(&printed_of(&record), &Caps::everything());
    assert_eq!(plain.definition.rules, []);
}
