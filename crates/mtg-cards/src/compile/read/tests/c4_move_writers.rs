//! Goal: the writers of choices followed by moves and of tokens counted by a group give the oracle wording of a creature returned before a sweep, lands put onto the battlefield up to X from a hand and a graveyard, a search for a land with a basic land type, tokens for the creatures attacking you, for the artifacts of a chosen opponent and for a devotion, and a wrong zone, count, kind, player or color never reads back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn time_wipe(chosen: &str, destroyed: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Choose":{{"from":{{"Each":[{chosen}]}},"as":"back"}}}},{{"Move":{{"what":{{"Bound":"back"}},"to":"Hand"}}}},{{"Destroy":{{"what":{{"Each":[{destroyed}]}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::creatures(
    r#"{"Type":"Creature"},{"ControlledBy":"You"}"#,
    r#"{"Type":"Creature"}"#,
    "Return a creature you control to its owner's hand, then destroy all creatures."
)]
#[case::artifacts(
    r#"{"Type":"Artifact"},{"ControlledBy":"You"}"#,
    r#"{"Type":"Artifact"}"#,
    "Return an artifact you control to its owner's hand, then destroy all artifacts."
)]
fn a_permanent_returned_before_a_sweep_is_written(
    #[case] chosen: &str,
    #[case] destroyed: &str,
    #[case] clause: &str,
) {
    // The article follows the noun: "a creature", "an artifact".
    assert!(written(&time_wipe(chosen, destroyed), clause), "{clause}");
}

#[rstest]
#[case::any_creature("Return a creature to its owner's hand, then destroy all creatures.")]
#[case::another_sweep(
    "Return a creature you control to its owner's hand, then destroy all artifacts."
)]
#[case::exile("Return a creature you control to its owner's hand, then exile all creatures.")]
fn a_return_before_a_sweep_does_not_read_as_another_group_or_sweep(#[case] clause: &str) {
    assert!(!written(
        &time_wipe(
            r#"{"Type":"Creature"},{"ControlledBy":"You"}"#,
            r#"{"Type":"Creature"}"#
        ),
        clause
    ));
}

fn rage(zones: &str, tapped: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":"AnyTarget"}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":"X"}}}},{{"Choose":{{"from":{{"Each":[{{"Type":"Land"}},{{"OwnedBy":"You"}},{{"AnyOf":[{zones}]}}]}},"count":{{"min":0,"max":"X"}},"as":"lands"}}}},{{"Move":{{"what":{{"Bound":"lands"}},"to":{{"Battlefield":{{{tapped}}}}}}}}}]}}}}"#
    )
}

const BOTH: &str = r#"{"In":"Hand"},{"In":"Graveyard"}"#;
const CLAUSE: &str = "~ deals X damage to any target. Put up to X land cards from your hand and/or graveyard onto the battlefield tapped.";

#[test]
fn damage_and_up_to_x_lands_from_the_hand_and_the_graveyard_is_written() {
    assert!(written(&rage(BOTH, r#""tapped":true"#), CLAUSE));
}

#[rstest]
#[case::only_the_hand(r#"{"In":"Hand"}"#, r#""tapped":true"#)]
#[case::untapped(BOTH, "")]
#[case::the_library(r#"{"In":"Hand"},{"In":"Library"}"#, r#""tapped":true"#)]
fn the_lands_do_not_read_from_another_zone_or_untapped(#[case] zones: &str, #[case] tapped: &str) {
    assert!(!written(&rage(zones, tapped), CLAUSE));
}

fn gardener(types: &[&str], reveal: bool) -> String {
    let list: Vec<String> = types
        .iter()
        .map(|kind| format!(r#"{{"Subtype":"{kind}"}}"#))
        .collect();
    format!(
        r#"{{"Triggered":{{"when":{{"TurnedFaceUp":{{"what":["This"]}}}},"effects":[{{"Search":{{"filter":[{{"Type":"Land"}},{{"AnyOf":[{}]}}],"count":{{"min":0,"max":1}},"reveal":{reveal},"as":"land"}}}},{{"Move":{{"what":{{"Bound":"land"}},"to":"Hand"}}}},{{"Shuffle":{{}}}}]}}}}"#,
        list.join(",")
    )
}

const BASICS: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];
const SEARCH: &str = "When ~ is turned face up, search your library for a land card with a basic land type, reveal it, put it into your hand, then shuffle.";

#[test]
fn a_search_for_a_land_with_a_basic_land_type_is_written() {
    assert!(written(&gardener(&BASICS, true), SEARCH));
}

#[rstest]
#[case::three_types(&["Plains", "Island", "Swamp"], true)]
#[case::not_revealed(&BASICS, false)]
fn a_search_does_not_read_as_a_land_with_fewer_types_or_unrevealed(
    #[case] types: &[&str],
    #[case] reveal: bool,
) {
    assert!(!written(&gardener(types, reveal), SEARCH));
}

fn herd(count: &str, spec: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{spec}}}],"effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Beast","colors":["Green"],"types":["Creature"],"subtypes":["Beast"],"power":3,"toughness":3}}}},"count":{count}}}}}]}}}}"#
    )
}

const HERD: &str = "Choose target opponent. You create X 3/3 green Beast creature tokens, where X is the number of artifacts that player controls.";

#[test]
fn tokens_for_the_artifacts_of_a_chosen_opponent_are_written() {
    assert!(written(
        &herd(
            r#"{"Count":[{"Type":"Artifact"},{"ControlledBy":{"Target":0}}]}"#,
            r#""Opponent""#
        ),
        HERD
    ));
}

#[rstest]
#[case::lands(
    r#"{"Count":[{"Type":"Land"},{"ControlledBy":{"Target":0}}]}"#,
    r#""Opponent""#
)]
#[case::a_player(
    r#"{"Count":[{"Type":"Artifact"},{"ControlledBy":{"Target":0}}]}"#,
    r#""Player""#
)]
#[case::your_artifacts(
    r#"{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]}"#,
    r#""Opponent""#
)]
fn the_tokens_do_not_read_for_another_kind_player_or_owner(
    #[case] count: &str,
    #[case] spec: &str,
) {
    assert!(!written(&herd(count, spec), HERD));
}

fn soldiers(count: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}}}},"count":{count}}}}}]}}}}"#
    )
}

#[rstest]
#[case::white(
    r#"{"Devotion":["White"]}"#,
    "When ~ enters, create a number of 1/1 white Soldier creature tokens equal to your devotion to white."
)]
#[case::white_and_blue(
    r#"{"Devotion":["White","Blue"]}"#,
    "When ~ enters, create a number of 1/1 white Soldier creature tokens equal to your devotion to white and blue."
)]
fn tokens_for_a_devotion_name_the_colors(#[case] count: &str, #[case] clause: &str) {
    assert!(written(&soldiers(count), clause), "{clause}");
}

#[rstest]
#[case::another_color(
    r#"{"Devotion":["White"]}"#,
    "When ~ enters, create a number of 1/1 white Soldier creature tokens equal to your devotion to black."
)]
#[case::fixed_count(
    r#"{"Devotion":["White"]}"#,
    "When ~ enters, create two 1/1 white Soldier creature tokens."
)]
fn a_devotion_does_not_read_as_another_color_or_a_fixed_number(
    #[case] count: &str,
    #[case] clause: &str,
) {
    assert!(!written(&soldiers(count), clause), "{clause}");
}

fn spiders(selector: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Spider","colors":["Green"],"types":["Creature"],"subtypes":["Spider"],"power":1,"toughness":2,"abilities":[{{"Keyword":"Reach"}}]}}}},"count":{{"Count":[{selector}]}}}}}},{{"Shield":{{"when":{{"Damage":{{"source":[{{"Type":"Creature"}},{{"Not":{{"Subtype":"Spider"}}}}],"combat":true}}}},"by":"Prevent","duration":"UntilEndOfTurn"}}}}]}}}}"#
    )
}

const WEB: &str = "Create X 1/2 green Spider creature tokens with reach, where X is the number of creatures attacking you. Prevent all combat damage that would be dealt this turn by non-Spider creatures.";

#[test]
fn spiders_for_the_creatures_attacking_you_are_written() {
    assert!(written(
        &spiders(r#"{"Type":"Creature"},{"AttackingPlayer":"You"}"#),
        WEB
    ));
}

#[rstest]
#[case::any_attacker(r#"{"Type":"Creature"},"Attacking""#)]
#[case::all_creatures(r#"{"Type":"Creature"}"#)]
fn the_spiders_do_not_read_for_every_attacker_or_every_creature(#[case] selector: &str) {
    assert!(!written(&spiders(selector), WEB));
}
