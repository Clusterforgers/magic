//! Goal: the writers of ep30 write the card's words and nothing near them. A repeatable "you may" cost of one card or
//! creature beside the spell's reduction for each payment ("you may exile any number of white cards from your hand. This
//! spell costs {2} less to cast for each card exiled this way."), mana paid any number of times, moves of "any number
//! of" counters, and damage shields over two recipients at once ("you and/or permanents you control"); a cost paid once,
//! another reduction, another count, a fixed number of counters or one recipient do not read back.
use super::*;

const WHITE_CARD: &str = r#"{"ExileFromHand":{"filter":[{"Color":"White"}]}}"#;
const TAP_CREATURE: &str = r#"{"TapPermanents":{"filter":[{"Type":"Creature"}]}}"#;
const MARCH: &str = "As an additional cost to cast this spell, you may exile any number of white cards from your hand. This spell costs {2} less to cast for each card exiled this way.";
const SINGULARITY: &str = "As an additional cost to cast this spell, you may tap any number of untapped creatures you control. This spell costs {1} less to cast for each creature tapped this way.";

fn paid(cost: &str, repeatable: bool) -> Ability {
    ability(&format!(
        r#"{{"AdditionalCost":{{"cost":[{cost}],"repeatable":{repeatable}}}}}"#
    ))
}

fn less(mana: &str, per: &str) -> Ability {
    ability(&format!(
        r#"{{"CostModifier":{{"applies_to":["This"],"change":"Reduce","mana":"{mana}","per":{per}}}}}"#
    ))
}

#[rstest]
#[case::two_each(WHITE_CARD, true, "{2}", r#""AdditionalPaid""#, MARCH, true)]
#[case::one_twice(
    WHITE_CARD,
    true,
    "{1}",
    r#"{"Times":{"by":2,"of":"AdditionalPaid"}}"#,
    MARCH,
    true
)]
#[case::tapped(TAP_CREATURE, true, "{1}", r#""AdditionalPaid""#, SINGULARITY, true)]
#[case::paid_once(WHITE_CARD, false, "{2}", r#""AdditionalPaid""#, MARCH, false)]
#[case::one_less_is_not_two(WHITE_CARD, true, "{1}", r#""AdditionalPaid""#, MARCH, false)]
#[case::the_x_of_the_spell(WHITE_CARD, true, "{2}", r#""X""#, MARCH, false)]
#[case::another_color(
    r#"{"ExileFromHand":{"filter":[{"Color":"Red"}]}}"#,
    true,
    "{2}",
    r#""AdditionalPaid""#,
    MARCH,
    false
)]
#[case::tapped_is_not_exiled(TAP_CREATURE, true, "{1}", r#""AdditionalPaid""#, MARCH, false)]
fn ep30_any_number_and_its_reduction_write_as_one_clause(
    #[case] cost: &str,
    #[case] repeatable: bool,
    #[case] mana: &str,
    #[case] per: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = static_pair_texts(&paid(cost, repeatable), &less(mana, per));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

const TASTE: &str =
    "As an additional cost to cast this spell, you may pay {1}{G} any number of times.";

#[rstest]
#[case::repeatable(r#"{"Mana":"{1}{G}"}"#, true, true)]
#[case::paid_once(r#"{"Mana":"{1}{G}"}"#, false, false)]
#[case::other_mana(r#"{"Mana":"{2}{G}"}"#, true, false)]
fn ep30_mana_any_number_of_times(
    #[case] cost: &str,
    #[case] repeatable: bool,
    #[case] reads: bool,
) {
    let found = written_variants(&paid(cost, repeatable));
    assert_eq!(
        found.iter().any(|w| same_text(w, TASTE)),
        reads,
        "{found:?}"
    );
}

const BANDAR: &str = "At the beginning of your upkeep, you may move any number of +1/+1 counters from ~ onto another target creature.";

fn bandar(count: &str, up_to: bool) -> Ability {
    ability(&format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},"Other"]}}}}],"effects":[{{"May":{{"effects":[{{"MoveCounters":{{"from":"This","to":{{"Target":0}},"counter":"+1/+1","count":{count},"up_to":{up_to}}}}}]}}}}]}}}}"#
    ))
}

const ALL_ON_IT: &str = r#"{"Counters":{"on":"This","counter":"+1/+1"}}"#;

#[rstest]
#[case::any_number(bandar(ALL_ON_IT, true), true)]
#[case::all_of_them(bandar(ALL_ON_IT, false), false)]
#[case::up_to_two(bandar("2", true), false)]
#[case::the_target_s_counters(
    bandar(r#"{"Counters":{"on":{"Target":0},"counter":"+1/+1"}}"#, true),
    false
)]
fn ep30_any_number_of_counters(#[case] bandar: Ability, #[case] reads: bool) {
    let found = written_variants(&bandar);
    assert_eq!(
        found.iter().any(|w| same_text(w, BANDAR)),
        reads,
        "{found:?}"
    );
}

/// "Move any number of counters" reads as a move of every counter there that the controller may stop.
#[test]
fn ep30_any_number_of_counters_reads_as_a_stoppable_move() {
    let read = clause(
        "Move any number of counters from target creature onto ~.",
        ClauseKind::Spell,
        false,
    );
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert!(
        json.contains(r#""count":{"AnyCounters":{"Target":0}},"up_to":true"#),
        "{json}"
    );
}

const GLARECASTER: &str = "{5}{W}: The next time damage would be dealt to ~ and/or you this turn, that damage is dealt to any target instead.";

fn glarecaster(to: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":"{{5}}{{W}}","targets":[{{"what":"AnyTarget"}}],"effects":[{{"Shield":{{"when":{{"Damage":{{"to":{to}}}}},"by":{{"Redirect":{{"to":{{"Target":0}},"next":true}}}},"duration":"UntilEndOfTurn"}}}}]}}}}"#
    ))
}

#[rstest]
#[case::both(glarecaster(r#"{"Union":["This","You"]}"#), true)]
#[case::the_card_alone(glarecaster(r#""This""#), false)]
#[case::you_alone(glarecaster(r#""You""#), false)]
#[case::opponents(glarecaster(r#"{"Union":["This","Opponents"]}"#), false)]
fn ep30_two_recipients_share_a_shield(#[case] shield: Ability, #[case] reads: bool) {
    let found = written_variants(&shield);
    assert_eq!(
        found.iter().any(|w| same_text(w, GLARECASTER)),
        reads,
        "{found:?}"
    );
}

const HARMS_WAY: &str = "The next 2 damage that a source of your choice would deal to you and/or permanents you control this turn is dealt to any target instead.";

fn harms_way(most: u32, to: &str) -> Ability {
    ability(&format!(
        r#"{{"Spell":{{"targets":[{{"what":"AnyTarget"}}],"effects":[{{"Choose":{{"from":{{"Each":[{{"AnyOf":[{{"In":"Battlefield"}},{{"In":"Stack"}},{{"In":"Command"}}]}}]}},"as":"source"}}}},{{"Shield":{{"when":{{"Damage":{{"to":{to},"source":[{{"Is":{{"Bound":"source"}}}}]}}}},"by":{{"RedirectUpTo":{{"most":{most},"to":{{"Target":0}}}}}},"duration":"UntilEndOfTurn"}}}}]}}}}"#
    ))
}

const YOURS: &str = r#"{"Union":["You",{"Each":[{"ControlledBy":"You"}]}]}"#;

#[rstest]
#[case::two(harms_way(2, YOURS), true)]
#[case::three(harms_way(3, YOURS), false)]
#[case::you_alone(harms_way(2, r#""You""#), false)]
#[case::creatures(
    harms_way(
        2,
        r#"{"Union":["You",{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}]}"#
    ),
    false
)]
fn ep30_a_redirected_budget_over_two_recipients(#[case] shield: Ability, #[case] reads: bool) {
    let found = written_variants(&shield);
    assert_eq!(
        found.iter().any(|w| same_text(w, HARMS_WAY)),
        reads,
        "{found:?}"
    );
}
