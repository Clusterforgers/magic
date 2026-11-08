//! Goal: "put its counters on target creature you control" and "for each kind of counter on target permanent or player, give that permanent or player another counter of that kind" write as the card prints them and the first reads back to the same rule; another object read, another place, a number of each kind or a target that is not the one read does not read back.
use super::*;

const TO_YOURS: &str =
    r#""targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}]"#;

fn dies(of: &str, each: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"from":"Battlefield","to":"Graveyard","what":["This"]}}}},"effects":[{{"PutCountersOf":{{"on":{{"Target":0}},"of":{of}{each}}}}}],{TO_YOURS}}}}}"#
    )
}

fn another_dies(of: &str, on: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"from":"Battlefield","to":"Graveyard","what":[{{"Type":"Creature"}},{{"ControlledBy":"You"}},"Other"]}}}},"effects":[{{"PutCountersOf":{{"on":{on},"of":{of}}}}}]}}}}"#
    )
}

fn each_kind(on: &str, of: &str, each: u32) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"PutCountersOf":{{"on":{on},"of":{of},"each":{each}}}}}],"targets":[{{"what":{{"Either":[{{"Object":[]}},"Player"]}}}}]}}}}"#
    )
}

const PUPIL: &str = "When ~ dies, put its counters on target creature you control.";
const COLONY: &str = "Whenever another creature you control dies, put its counters on ~.";
const COLONY_THOSE: &str = "Whenever another creature you control dies, put those counters on ~.";
const BROKER: &str = "{T}: For each kind of counter on target permanent or player, give that permanent or player another counter of that kind.";

#[rstest]
#[case::the_card_itself(dies(r#""This""#, ""), PUPIL, true)]
#[case::the_object_of_the_event(dies(r#""Triggering""#, ""), PUPIL, true)]
#[case::one_of_each_kind_is_not_its_counters(dies(r#""This""#, r#","each":1"#), PUPIL, false)]
#[case::the_target_s_counters_are_not_its(dies(r#"{"Target":0}"#, ""), PUPIL, false)]
#[case::another_creature_s(another_dies(r#""Triggering""#, r#""This""#), COLONY, true)]
#[case::those_counters(another_dies(r#""Triggering""#, r#""This""#), COLONY_THOSE, true)]
#[case::the_card_s_own_are_not_the_other_s(another_dies(r#""This""#, r#""This""#), COLONY, false)]
#[case::onto_the_other_is_not_onto_the_card(
    another_dies(r#""Triggering""#, r#""Triggering""#),
    COLONY,
    false
)]
#[case::another_of_each_kind(each_kind(r#"{"Target":0}"#, r#"{"Target":0}"#, 1), BROKER, true)]
#[case::two_of_each_kind(each_kind(r#"{"Target":0}"#, r#"{"Target":0}"#, 2), BROKER, false)]
#[case::the_card_s_kinds(each_kind(r#"{"Target":0}"#, r#""This""#, 1), BROKER, false)]
#[case::onto_the_card(each_kind(r#""This""#, r#"{"Target":0}"#, 1), BROKER, false)]
fn g12b_counters_of_write_as_printed(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability(&json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

fn one_kind(each: u32, one_kind: bool, what: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{3}}"}},"Tap"],"effects":[{{"PutCountersOf":{{"on":{{"Target":0}},"of":{{"Target":0}},"each":{each},"one_kind":{one_kind}}}}}],"targets":[{{"what":{what}}}]}}}}"#
    )
}

const EITHER: &str = r#"{"Either":[{"Object":[]},"Player"]}"#;
const PERMANENT: &str = r#"{"Object":[]}"#;
const MODULE: &str = "{3}, {T}: Choose a counter on target permanent or player. Give that permanent or player another counter of that kind.";
const GAUNTLET: &str = "{3}, {T}: Choose a counter on target permanent. Put an additional counter of that kind on that permanent.";

#[rstest]
#[case::give_another(one_kind(1, true, EITHER), MODULE, true)]
#[case::an_additional(one_kind(1, true, PERMANENT), GAUNTLET, true)]
#[case::each_kind_is_not_one(one_kind(1, false, EITHER), MODULE, false)]
#[case::two_is_not_another(one_kind(2, true, EITHER), MODULE, false)]
#[case::a_permanent_is_not_a_player_too(one_kind(1, true, PERMANENT), MODULE, false)]
fn ep22_one_kind_writes_as_printed(#[case] json: String, #[case] text: &str, #[case] reads: bool) {
    let found = written_variants(&ability(&json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

#[rstest]
#[case::its(PUPIL)]
#[case::another_creature_s(COLONY)]
#[case::those(COLONY_THOSE)]
fn g12b_counters_of_read_back(#[case] text: &str) {
    let json = serde_json::to_string(&golden(text, ClauseKind::Triggered, false)).unwrap();
    assert!(json.contains("PutCountersOf"), "{text}: {json}");
}

fn frog(except: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"CreateToken":{{"token":{{"CopyOf":{{"except":{except},"of":{{"Target":0}}}}}}}}}}],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}]}}}}"#
    )
}

const FROG: &str = "Create a token that's a copy of target creature, except it's a 1/1 green Frog.";
const INSECT: &str =
    "Create a token that's a copy of target creature, except it's a 1/1 black and green Insect.";
const FROG_TOO: &str = "Create a token that's a copy of target creature, except it's a 1/1 green Frog in addition to its other types.";
const SET_FROG: &str = r#"[{"SetPT":{"power":1,"toughness":1}},{"SetColors":["Green"]},{"SetCreatureTypes":["Frog"]}]"#;
const SET_INSECT: &str = r#"[{"SetPT":{"power":1,"toughness":1}},{"SetColors":["Black","Green"]},{"SetCreatureTypes":["Insect"]}]"#;

#[rstest]
#[case::a_frog(SET_FROG, FROG, true)]
#[case::two_colors(SET_INSECT, INSECT, true)]
#[case::another_size(r#"[{"SetPT":{"power":2,"toughness":2}},{"SetColors":["Green"]},{"SetCreatureTypes":["Frog"]}]"#, FROG, false)]
#[case::another_color(
    r#"[{"SetPT":{"power":1,"toughness":1}},{"SetColors":["Blue"]},{"SetCreatureTypes":["Frog"]}]"#,
    FROG,
    false
)]
#[case::a_frog_added_is_not_only_a_frog(
    r#"[{"SetPT":{"power":1,"toughness":1}},{"SetColors":["Green"]},{"AddSubtype":"Frog"}]"#,
    FROG,
    false
)]
#[case::only_a_frog_is_not_a_frog_added(SET_FROG, FROG_TOO, false)]
fn g12b_copy_creature_types_write_as_printed(
    #[case] except: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability(&frog(except)));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

#[rstest]
#[case::a_frog(FROG, SET_FROG)]
#[case::two_colors(INSECT, SET_INSECT)]
fn g12b_copy_creature_types_read_back(#[case] text: &str, #[case] except: &str) {
    reads_as(text, ClauseKind::Spell, false, &frog(except));
}
