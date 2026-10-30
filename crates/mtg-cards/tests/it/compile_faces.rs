//! Goal: the compiler reads a card face by face from the oracle text of each face, whatever the layout (split, adventure, modal and transforming double-faced): each clause belongs to its face and lies in that face's text, `~` is the face's own name, a clause it cannot read is a `Manual` ability of its own face, and the card is `Full`, `Partial` or `Manual` by all its faces together.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{printed_of, records};
use mtg_cards::card::ImplStatus;
use mtg_cards::compile::compile;
use rstest::rstest;

const FIRE: &str =
    r#"{"Manual":{"reason":"you resolve this spell effect by hand","span":{"start":0,"end":67}}}"#;
const ICE: &str = r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"Tap":{"what":{"Target":0}}},{"Draw":{}}]}}"#;
const STOMP: &str = r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"Rule":{"rule":{"DamageCantBePrevented":{}},"duration":"UntilEndOfTurn"}},{"DealDamage":{"to":{"Target":0},"amount":2}}]}}"#;
const GIANT: &str = r#"{"Manual":{"reason":"you apply this triggered ability by hand","span":{"start":0,"end":110}}}"#;
const WHITE: &str =
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["White"]}}}]}}"#;
const BLUE: &str =
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Blue"]}}}]}}"#;
const DESTROY_ARTIFACT: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;
const DESTROY_ENCHANTMENT: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Enchantment"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;

/// The abilities of each face of the card compiled from its record, as JSON.
fn compiled(name: &str) -> (ImplStatus, Vec<Vec<String>>) {
    let record = records().into_iter().find(|r| r.name == name).unwrap();
    let printed = printed_of(&record);
    let compiled = compile(&printed);
    let card = compiled.card.expect("the card has rules text");
    for clause in &compiled.clauses {
        let text = &printed.faces[clause.face].oracle_text;
        assert!(clause.span.slice(text).is_some(), "{clause:?} in {text:?}");
    }
    let faces = card
        .faces
        .iter()
        .map(|face| {
            face.abilities
                .iter()
                .map(|ability| serde_json::to_string(ability).unwrap())
                .collect()
        })
        .collect();
    (card.impl_status, faces)
}

#[rstest]
#[case::split("Fire // Ice", ImplStatus::Partial, vec![vec![FIRE], vec![ICE]])]
#[case::adventure("Bonecrusher Giant // Stomp", ImplStatus::Partial, vec![vec![GIANT], vec![STOMP]])]
#[case::modal_with_lands("Hengegate Pathway // Mistgate Pathway", ImplStatus::Full, vec![vec![WHITE], vec![BLUE]])]
#[case::split_with_fuse("Wear // Tear", ImplStatus::Full, vec![vec![DESTROY_ARTIFACT, r#"{"Keyword":"Fuse"}"#], vec![DESTROY_ENCHANTMENT, r#"{"Keyword":"Fuse"}"#]])]
#[case::modal_one_face_read("Selfless Glyphweaver // Deadly Vanity", ImplStatus::Partial, vec![vec![r#"{"Activated":{"cost":["ExileSelf"],"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}}"#], vec![r#"{"Manual":{"reason":"you resolve this spell effect by hand","span":{"start":0,"end":86}}}"#]])]
#[case::transform_keywords("Tavern Ruffian // Tavern Smasher", ImplStatus::Full, vec![vec![r#"{"Keyword":"Daybound"}"#], vec![r#"{"Keyword":"Nightbound"}"#]])]
#[case::transform_keyword_pairs(
    "Bird Admirer // Wing Shredder",
    ImplStatus::Full,
    vec![vec![r#"{"Keyword":"Reach"}"#, r#"{"Keyword":"Daybound"}"#], vec![r#"{"Keyword":"Reach"}"#, r#"{"Keyword":"Nightbound"}"#]]
)]
fn a_card_is_compiled_face_by_face(
    #[case] name: &str,
    #[case] status: ImplStatus,
    #[case] expected: Vec<Vec<&str>>,
) {
    let (found_status, faces) = compiled(name);
    assert_eq!(found_status, status, "{name}");
    let expected: Vec<Vec<String>> = expected
        .iter()
        .map(|face| face.iter().map(|ability| (*ability).to_owned()).collect())
        .collect();
    assert_eq!(faces, expected, "{name}");
}

#[rstest]
#[case::split("Fire // Ice", &[0, 1, 1])]
#[case::adventure("Bonecrusher Giant // Stomp", &[0, 1])]
#[case::fuse("Wear // Tear", &[0, 0, 1, 1])]
#[case::transform("Bird Admirer // Wing Shredder", &[0, 0, 1, 1])]
fn every_clause_belongs_to_the_face_whose_text_holds_it(
    #[case] name: &str,
    #[case] faces: &[usize],
) {
    let record = records().into_iter().find(|r| r.name == name).unwrap();
    let printed = printed_of(&record);
    let found: Vec<usize> = compile(&printed)
        .clauses
        .iter()
        .map(|clause| clause.face)
        .collect();
    assert_eq!(found, faces, "{name}");
}
