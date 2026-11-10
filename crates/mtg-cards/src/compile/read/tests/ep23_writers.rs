//! Goal: the writers of ep23 write the card's words and nothing near them: a counter moved "onto a second target permanent" (other than the first target or not, as the reader reads it), lands of every zone ("Lands you control and land cards you own that aren't on the battlefield are Deserts in addition to their other types.") and a required cost with two reductions in one clause ("... {2} less to cast for each permanent sacrificed this way and {2} less to cast for each other artifact or creature you've sacrificed this turn."); creatures in that wording, fewer zones, another player's sacrifices, another amount or another kind do not read back.
use super::*;

const ALL_ZONES: &str = r#"{"AnyOf":[{"In":"Battlefield"},{"In":"Stack"},{"In":"Hand"},{"In":"Library"},{"In":"Graveyard"},{"In":"Exile"},{"In":"Command"}]}"#;
const SOME_ZONES: &str = r#"{"AnyOf":[{"In":"Battlefield"},{"In":"Hand"}]}"#;
const DESERTS: &str = "Lands you control and land cards you own that aren't on the battlefield are Deserts in addition to their other types.";
const CREATURE_DESERTS: &str = "Creatures you control and creature cards you own that aren't on the battlefield are Deserts in addition to their other types.";

fn deserts(kind: &str, zones: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"{kind}"}},{{"ControlledBy":"You"}},{zones}]}},"mods":[{{"AddSubtype":"Desert"}}]}}}}"#
    )
}

#[rstest]
#[case::lands_everywhere(deserts("Land", ALL_ZONES), DESERTS, true)]
#[case::lands_in_some_zones(deserts("Land", SOME_ZONES), DESERTS, false)]
#[case::creatures_are_not_said_so(deserts("Creature", ALL_ZONES), CREATURE_DESERTS, false)]
fn ep23_lands_of_every_zone_write_as_printed(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability(&json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

const PAID: &str = r#"{"AdditionalCost":{"cost":[{"Sacrifice":{"count":"X","filter":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]}]}}],"required":true}}"#;
const BY_X: &str = r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{1}","per":{"Times":{"by":2,"of":"X"}}}}"#;
const KINDS: &str = r#"[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]}]"#;
const DARGO: &str = "As an additional cost to cast this spell, you may sacrifice any number of artifacts and/or creatures. This spell costs {2} less to cast for each permanent sacrificed this way and {2} less to cast for each other artifact or creature you've sacrificed this turn.";

fn tallied(mana: &str, by: u32, who: &str, filter: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":["This"],"change":"Reduce","mana":"{mana}","per":{{"Times":{{"by":{by},"of":{{"Tally":{{"event":"Sacrificed","filter":{filter},"who":"{who}"}}}}}}}}}}}}"#
    )
}

#[rstest]
#[case::as_printed(tallied("{1}", 2, "You", KINDS), true)]
#[case::two_once(tallied("{2}", 1, "You", KINDS), true)]
#[case::one_less_is_not_two(tallied("{1}", 1, "You", KINDS), false)]
#[case::an_opponent_s(tallied("{1}", 2, "Opponents", KINDS), false)]
#[case::creatures_only(tallied("{1}", 2, "You", r#"[{"Type":"Creature"}]"#), false)]
fn ep23_a_cost_and_two_reductions_write_as_one_clause(#[case] third: String, #[case] reads: bool) {
    let found = crate::compile::ep23_three_texts(&ability(PAID), &ability(BY_X), &ability(&third));
    assert_eq!(
        found.iter().any(|w| same_text(w, DARGO)),
        reads,
        "{found:?}"
    );
}

#[test]
fn ep23_the_pair_alone_is_not_the_clause() {
    let found = static_pair_texts(&ability(PAID), &ability(BY_X));
    assert!(!found.iter().any(|w| same_text(w, DARGO)), "{found:?}");
}

const NESTING: &str = "{1}, {T}: Move a counter from target permanent you control onto a second target permanent. Activate only as a sorcery.";
const MINE: &str = r#"{"what":{"Object":[{"ControlledBy":"You"}]}}"#;
const OTHER: &str = r#"{"what":{"Object":[{"Not":{"Is":{"Target":0}}}]}}"#;

fn moved(first: &str, second: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{1}}"}},"Tap"],"effects":[{{"MoveCounters":{{"from":{{"Target":0}},"to":{{"Target":1}}}}}}],"targets":[{first},{second}],"timing":"Sorcery"}}}}"#
    )
}

#[rstest]
#[case::another_target(moved(MINE, OTHER), true)]
#[case::the_reader_s_second_target(moved(MINE, r#"{"what":{"Object":[]}}"#), true)]
#[case::onto_a_creature(moved(MINE, r#"{"what":{"Object":[{"Type":"Creature"},{"Not":{"Is":{"Target":0}}}]}}"#), false)]
#[case::from_any_permanent(moved(r#"{"what":{"Object":[]}}"#, OTHER), false)]
fn ep23_a_counter_moves_onto_another_target(#[case] json: String, #[case] reads: bool) {
    let found = written_variants(&ability(&json));
    assert_eq!(found.iter().any(|w| same_text(w, NESTING)), reads, "{found:?}");
}
