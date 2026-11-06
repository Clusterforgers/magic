//! Goal: the writers write each ability once per outer call and join the parts of a static three deep at most, so a static with many changes has a bounded number of wordings and writes and keeps its wordings of two and three parts, and a four-part chain is not written.
use super::super::written_memo::written_count;
use super::*;

/// Nissa's Zendikon: five changes, each a part another writer joins.
const ZENDIKON: &str = r#"{"Static":{"affected":"Attached","mods":[{"SetPT":{"power":4,"toughness":4}},{"AddKeyword":"Reach"},{"AddKeyword":"Haste"},{"AddType":"Creature"},{"AddSubtype":"Elemental"}],"condition":{"Matches":{"subject":"Attached","selector":[{"Type":"Land"}]}}}}"#;
const CAGEMAIL: &str = r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":2}},"CantAttack"]}}"#;
const ORZHOVA: &str = r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Lifelink"},{"AddKeyword":"Flying"}]}}"#;
const THREE: &str = r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":2}},"CantAttack","CantBlock"]}}"#;
const FOUR: &str = r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":2}},"CantAttack","CantBlock",{"AddKeyword":"Flying"}]}}"#;

/// The wordings of an ability and the abilities written for them.
fn counted(json: &str) -> (Vec<String>, usize) {
    let before = written_count();
    let texts = written_variants(&ability(json));
    (texts, written_count() - before)
}

#[rstest]
#[case::zendikon(
    ZENDIKON,
    "Enchanted permanent has base power and toughness 4/4, has reach and haste, is a creature in addition to its other types, and is an Elemental in addition to its other types as long as it is a land.",
    50_000,
    40
)]
#[case::two_parts(CAGEMAIL, "Enchanted creature gets +2/+2 and can't attack.", 100, 20)]
#[case::keyword_pair(
    ORZHOVA,
    "Enchanted creature gets +1/+1 and has flying and lifelink.",
    200,
    20
)]
#[case::three_parts(
    THREE,
    "Enchanted creature gets +2/+2 and can't attack and can't block.",
    200,
    20
)]
fn a_static_of_many_changes_is_written_in_bounded_work(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] most_wordings: usize,
    #[case] most_writes: usize,
) {
    let (texts, writes) = counted(json);
    assert!(
        texts.iter().any(|text| same_text(text, clause)),
        "{clause}: {texts:#?}"
    );
    assert!(texts.len() <= most_wordings, "{} wordings", texts.len());
    assert!(writes <= most_writes, "{writes} abilities written");
}

#[test]
fn a_part_of_a_part_joins_no_parts_of_its_own() {
    let (texts, _) = counted(FOUR);
    let nested = "Enchanted creature gets +2/+2 and can't attack and can't block and has flying.";
    assert!(
        !texts.iter().any(|text| same_text(text, nested)),
        "{nested}"
    );
}

#[test]
fn an_ability_written_twice_gives_the_same_wordings() {
    let first = written_variants(&ability(ZENDIKON));
    assert_eq!(written_variants(&ability(ZENDIKON)), first);
}
