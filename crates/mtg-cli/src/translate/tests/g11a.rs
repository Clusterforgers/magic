//! Goal: round 11 pack g11a reads players who control more, fewer or at least N more permanents of a type than the
//! player who asks (`Ref::Asker`), as a count, a static's players and a trigger's player, a goad for the rest of the game or while the source
//! stays, connive N and an optional scry, and leaves other words a gap by name.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given type line, script lines and Oracle text, as compact JSON, and its gaps.
fn card(types: &str, lines: &str, oracle: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\nPT:2/2\n{lines}\nOracle:{oracle}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

/// The test of a player who controls `kind` compared `op` with the asker, plus `more`.
fn test(kind: &str, op: &str, more: u32) -> String {
    let asker = format!(r#"{{"Count":[{{"Type":"{kind}"}},{{"ControlledBy":"Asker"}}]}}"#);
    let right = if more == 0 {
        asker
    } else {
        format!(r#"{{"Sum":[{asker},{more}]}}"#)
    };
    format!(
        r#"{{"Compare":{{"left":{{"Count":[{{"Type":"{kind}"}},{{"ControlledBy":"You"}}]}},"op":"{op}","right":{right}}}}}"#
    )
}

const DRAW: &str = "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ D\nSVar:D:DB$ Draw | NumCards$ X\nSVar:X:";

#[rstest]
#[case::opponents_more_lands(
    "PlayerCountOpponents$HasPropertywithMoreLandsThanYou",
    "Opponents",
    "Land",
    "Gt",
    0
)]
#[case::opponents_more_creatures(
    "PlayerCountOpponents$HasPropertywithMoreCreaturesThanYou",
    "Opponents",
    "Creature",
    "Gt",
    0
)]
#[case::opponents_fewer_creatures(
    "PlayerCountOpponents$HasPropertyhasFewerCreaturesInPlayThanYou",
    "Opponents",
    "Creature",
    "Lt",
    0
)]
#[case::players_one_more_land(
    "PlayerCountPropertywithAtLeast1MoreLandsThanYou$Amount",
    "EachPlayer",
    "Land",
    "Gt",
    0
)]
#[case::players_two_more_lands(
    "PlayerCountPropertywithAtLeast2MoreLandsThanYou$Amount",
    "EachPlayer",
    "Land",
    "Ge",
    2
)]
fn players_compared_with_the_asker_are_counted(
    #[case] amount: &str,
    #[case] who: &str,
    #[case] kind: &str,
    #[case] op: &str,
    #[case] more: u32,
) {
    let (json, found) = card("Creature Elf", &format!("{DRAW}{amount}"), "");
    assert!(found.is_empty(), "{found:?}");
    let group = format!(
        r#"{{"Players":{{"PlayersWhere":{{"test":{},"who":"{who}"}}}}}}"#,
        test(kind, op, more)
    );
    assert!(json.contains(&group), "{json}");
}

#[test]
fn a_static_names_the_opponents_with_more_lands() {
    let (json, found) = card(
        "Enchantment",
        "S:Mode$ CantPlayLand | Player$ Opponent.withMoreLandsThanYou | Description$ Each opponent who controls more lands than you can't play lands.",
        "Each opponent who controls more lands than you can't play lands.",
    );
    assert!(found.is_empty(), "{found:?}");
    let who = format!(
        r#"{{"PlayersWhere":{{"test":{},"who":"Opponents"}}}}"#,
        test("Land", "Gt", 0)
    );
    assert!(json.contains(&who), "{json}");
}

#[rstest]
#[case::another_type(
    "PlayerCountOpponents$HasPropertywithMorePlaneswalkersThanYou",
    "amount:PlayerCountOpponents$HasPropertywithMorePlaneswalkersThanYou"
)]
#[case::another_stat(
    "PlayerCountOpponents$HasPropertywithMoreLifeThanYou",
    "amount:PlayerCountOpponents$HasPropertywithMoreLifeThanYou"
)]
#[case::no_margin(
    "PlayerCountPropertywithAtLeast0MoreLandsThanYou$Amount",
    "amount:PlayerCountPropertywithAtLeast0MoreLandsThanYou"
)]
#[case::fewer_without_in_play(
    "PlayerCountOpponents$HasPropertyhasFewerCreaturesThanYou",
    "amount:PlayerCountOpponents$HasPropertyhasFewerCreaturesThanYou"
)]
fn another_comparison_stays_a_gap(#[case] amount: &str, #[case] gap: &str) {
    let (_, found) = card("Creature Elf", &format!("{DRAW}{amount}"), "");
    assert_eq!(found, [gap]);
}

const GOAD: &str = "A:SP$ Goad | ValidTgts$ Creature | TgtPrompt$ Select target creature";

#[rstest]
#[case::next_turn("", r#"{"Goad":{"what":{"Target":0}}}"#)]
#[case::rest_of_the_game(
    " | Duration$ Permanent",
    r#"{"Continuous":{"affected":{"Target":0},"duration":"Forever","mods":["Goaded"]}}"#
)]
#[case::while_on_the_battlefield(
    " | Duration$ AsLongAsInPlay",
    r#"{"Continuous":{"affected":{"Target":0},"duration":"WhileSourceOnBattlefield","mods":["Goaded"]}}"#
)]
fn a_goad_lasts_its_duration(#[case] duration: &str, #[case] effect: &str) {
    let (json, found) = card("Sorcery", &format!("{GOAD}{duration}"), "");
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(effect), "{json}");
}

#[test]
fn a_goad_of_another_duration_stays_a_gap() {
    let (_, found) = card("Sorcery", &format!("{GOAD} | Duration$ UntilUntaps"), "");
    assert_eq!(found, ["key:Goad.Duration=UntilUntaps"]);
}

#[rstest]
#[case::once("", r#"{"Connive":{"what":{"Target":0}}}"#)]
#[case::twice(" | ConniveNum$ 2", r#"{"Connive":{"count":2,"what":{"Target":0}}}"#)]
#[case::x(
    " | ConniveNum$ X\nSVar:X:Count$Valid Creature.attacking",
    r#"{"Connive":{"count":{"Count":[{"Type":"Creature"},"Attacking"]},"what":{"Target":0}}}"#
)]
fn a_connive_draws_and_discards_its_number(#[case] num: &str, #[case] effect: &str) {
    let (json, found) = card(
        "Sorcery",
        &format!("A:SP$ Connive | ValidTgts$ Creature | TgtPrompt$ Select target creature{num}"),
        "",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(effect), "{json}");
}

const SCRY: &str = "A:SP$ Scry | ScryNum$ 1";
const EACH_MAY: &str = r#"{"RepeatEach":{"as":"scrier","effects":[{"May":{"effects":[{"Scry":{"who":{"Bound":"scrier"}}}],"who":{"Bound":"scrier"}}}],"over":"#;

#[rstest]
#[case::you_must("", r#"[{"Scry":{}}]"#)]
#[case::you_may(" | Optional$ True", r#"[{"May":{"effects":[{"Scry":{}}]}}]"#)]
#[case::each_opponent_may(
    " | Defined$ Opponent | Optional$ True",
    &format!(r#"{EACH_MAY}"Opponents"}}}}]"#)
)]
#[case::each_player_may(
    " | Defined$ Player | Optional$ True",
    &format!(r#"{EACH_MAY}"EachPlayer"}}}}]"#)
)]
fn an_optional_scry_is_each_players_choice(#[case] keys: &str, #[case] effects: &str) {
    let (json, found) = card("Sorcery", &format!("{SCRY}{keys}"), "");
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(effects), "{json}");
}

#[test]
fn a_scry_optional_of_another_word_stays_a_gap() {
    let (_, found) = card("Sorcery", &format!("{SCRY} | Optional$ False"), "");
    assert_eq!(found, ["key:Scry.Optional=False"]);
}
