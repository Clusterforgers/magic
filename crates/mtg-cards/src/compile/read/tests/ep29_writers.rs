//! Goal: the writers of ep29 write the card's words and nothing near them. "Remove all <kind> counters from ~" reads as a removal of every counter of the kind on the card and "<kind> counters removed this way" as the number it removed (`RemovedThisWay`), for each and as "the number of"; a removal of a fixed number, of another kind, or a count of counters still on the card does not read as either.
use super::*;

const ALL_P1P1: &str = r#"{"RemoveCounters":{"counter":"+1/+1","count":{"Counters":{"on":"This","counter":"+1/+1"}}}}"#;

#[rstest]
#[case::molten_hydra(
    "{T}, Remove all +1/+1 counters from ~: It deals damage to any target equal to the number of +1/+1 counters removed this way.",
    "\"+1/+1\"",
    "\"+1/+1\""
)]
#[case::essence_bottle(
    "{T}, Remove all elixir counters from ~: You gain 2 life for each elixir counter removed this way.",
    "{\"Other\":\"elixir\"}",
    "{\"Other\":\"elixir\"}"
)]
#[case::geometric_nexus(
    "{6}, {T}, Remove all charge counters from ~: Create a 0/0 green and blue Fractal creature token. Put X +1/+1 counters on it, where X is the number of charge counters removed this way.",
    "\"charge\"",
    "\"charge\""
)]
#[case::torture_chamber(
    "{1}, {T}, Remove all pain counters from ~: It deals damage to target creature equal to the number of pain counters removed this way.",
    "{\"Other\":\"pain\"}",
    "{\"Other\":\"pain\"}"
)]
fn ep29_remove_all_reads_and_writes(
    #[case] text: &str,
    #[case] kind: &str,
    #[case] read_kind: &str,
) {
    let read = golden(text, ClauseKind::Activated, false);
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert!(
        json.contains(&format!(
            r#"{{"Counters":{{"on":"This","counter":{kind}}}}}"#
        )),
        "{json}"
    );
    assert!(
        json.contains(&format!(r#"{{"RemovedThisWay":{read_kind}}}"#)),
        "{json}"
    );
}

fn hydra(cost: &str, amount: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":["Tap",{cost}],"targets":[{{"what":"AnyTarget"}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]}}}}"#
    ))
}

const HYDRA: &str = "{T}, Remove all +1/+1 counters from ~: ~ deals damage to any target equal to the number of +1/+1 counters removed this way.";

#[rstest]
#[case::hydra(hydra(ALL_P1P1, r#"{"RemovedThisWay":"+1/+1"}"#), true)]
#[case::a_fixed_number(
    hydra(
        r#"{"RemoveCounters":{"counter":"+1/+1","count":2}}"#,
        r#"{"RemovedThisWay":"+1/+1"}"#
    ),
    false
)]
#[case::another_kind_removed(
    hydra(&ALL_P1P1.replace("\"+1/+1\"}}", "\"charge\"}}"), r#"{"RemovedThisWay":"+1/+1"}"#),
    false
)]
#[case::the_counters_still_on_it(
    hydra(ALL_P1P1, r#"{"Counters":{"on":"This","counter":"+1/+1"}}"#),
    false
)]
#[case::another_kind_read(hydra(ALL_P1P1, r#"{"RemovedThisWay":"charge"}"#), false)]
fn ep29_remove_all_near_misses_differ(#[case] ability: Ability, #[case] reads: bool) {
    let found = written_variants(&ability);
    assert_eq!(
        found.iter().any(|w| same_text(w, HYDRA)),
        reads,
        "{found:?}"
    );
}

const SCOURING: &str = "You may cast this spell as though it had flash by tapping three untapped creatures you control with flying in addition to paying its other costs.";

/// Tegwyll's Scouring: the permission adds a tap of three creatures with flying; another count, kind or cost is not it.
#[rstest]
#[case::three_fliers(
    r#"{"TapPermanents":{"filter":[{"Type":"Creature"},{"HasKeyword":"Flying"}],"count":3}}"#,
    true
)]
#[case::two_fliers(
    r#"{"TapPermanents":{"filter":[{"Type":"Creature"},{"HasKeyword":"Flying"}],"count":2}}"#,
    false
)]
#[case::any_creatures(
    r#"{"TapPermanents":{"filter":[{"Type":"Creature"}],"count":3}}"#,
    false
)]
#[case::three_life(r#"{"PayLife":3}"#, false)]
fn ep29_flash_by_tapping_differs_from_near_misses(#[case] tap: &str, #[case] reads: bool) {
    let rule = ability(&format!(
        r#"{{"Rule":{{"rule":{{"MayCast":{{"what":["This"],"flash":true,"additional":[{tap}]}}}},"from":"Hand"}}}}"#
    ));
    let found = written_variants(&rule);
    assert_eq!(
        found.iter().any(|w| same_text(w, SCOURING)),
        reads,
        "{found:?}"
    );
}

const RELIC: &str =
    "{2}, {T}, Remove all charge counters from ~: ~ deals that much damage to target creature.";

/// Relic Amulet: the counters removed are "that much"; the counters still on the card or a fixed removal are not.
#[rstest]
#[case::removed(r#"{"RemovedThisWay":"charge"}"#, ALL_CHARGE, true)]
#[case::still_on_it(r#"{"Counters":{"on":"This","counter":"charge"}}"#, ALL_CHARGE, false)]
#[case::two_removed(
    r#"{"RemovedThisWay":"charge"}"#,
    r#"{"RemoveCounters":{"counter":"charge","count":2}}"#,
    false
)]
fn ep29_that_much_is_what_was_removed(
    #[case] amount: &str,
    #[case] removal: &str,
    #[case] reads: bool,
) {
    let relic = ability(&format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{2}}"}},"Tap",{removal}],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]}}}}"#
    ));
    let found = written_variants(&relic);
    assert_eq!(
        found.iter().any(|w| same_text(w, RELIC)),
        reads,
        "{found:?}"
    );
}

const ALL_CHARGE: &str = r#"{"RemoveCounters":{"counter":"charge","count":{"Counters":{"on":"This","counter":"charge"}}}}"#;

const WYLL: &str = "If you would roll one or more dice, instead roll that many dice plus one and ignore the lowest roll.";

#[test]
fn ep29_extra_die_reads_and_writes() {
    let read = golden(WYLL, ClauseKind::Static, false);
    assert_eq!(read, ability(r#"{"Rule":{"rule":{"ExtraDie":"You"}}}"#));
}

#[rstest]
#[case::you(r#"{"Rule":{"rule":{"ExtraDie":"You"}}}"#, true)]
#[case::opponents(r#"{"Rule":{"rule":{"ExtraDie":"Opponents"}}}"#, false)]
#[case::coins(r#"{"Rule":{"rule":{"FlipTwoCoins":"You"}}}"#, false)]
#[case::from_the_graveyard(r#"{"Rule":{"rule":{"ExtraDie":"You"},"from":"Graveyard"}}"#, false)]
#[case::under_a_condition(
    r#"{"Rule":{"rule":{"ExtraDie":"You"},"condition":"YourTurn"}}"#,
    false
)]
fn ep29_extra_die_near_misses_differ(#[case] json: &str, #[case] reads: bool) {
    let found = written_variants(&ability(json));
    assert_eq!(found.iter().any(|w| same_text(w, WYLL)), reads, "{found:?}");
}

const GHITU: &str = "You may cast ~ as though it had flash if you pay {2} more to cast it.";

fn paid_flash(permission: &str) -> Ability {
    ability(&format!(
        r#"{{"Rule":{{"rule":{{"MayCast":{permission}}},"from":"Hand"}}}}"#
    ))
}

#[rstest]
#[case::named(GHITU)]
#[case::this_spell(
    "You may cast this spell as though it had flash if you pay {2} more to cast it."
)]
fn ep29_paid_flash_reads_and_writes(#[case] text: &str) {
    let read = golden(text, ClauseKind::Static, false);
    assert_eq!(
        read,
        paid_flash(r#"{"what":["This"],"flash":true,"additional":[{"Mana":"{2}"}]}"#)
    );
}

#[rstest]
#[case::two_more(
    r#"{"what":["This"],"flash":true,"additional":[{"Mana":"{2}"}]}"#,
    true
)]
#[case::three_more(
    r#"{"what":["This"],"flash":true,"additional":[{"Mana":"{3}"}]}"#,
    false
)]
#[case::without_flash(r#"{"what":["This"],"additional":[{"Mana":"{2}"}]}"#, false)]
#[case::other_cards(
    r#"{"what":[{"Type":"Creature"}],"flash":true,"additional":[{"Mana":"{2}"}]}"#,
    false
)]
#[case::life_more(
    r#"{"what":["This"],"flash":true,"additional":[{"PayLife":2}]}"#,
    false
)]
#[case::nothing_more(r#"{"what":["This"],"flash":true}"#, false)]
fn ep29_paid_flash_near_misses_differ(#[case] permission: &str, #[case] reads: bool) {
    let found = written_variants(&paid_flash(permission));
    assert_eq!(
        found.iter().any(|w| same_text(w, GHITU)),
        reads,
        "{found:?}"
    );
}
