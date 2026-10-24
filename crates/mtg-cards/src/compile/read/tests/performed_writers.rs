//! Goal: the writers of steps that follow a step which took something away give the oracle wording of a sacrifice followed by "When you do" and a search, a payment that buys a search, a sacrifice and a search in one sentence and a removal of every counter with a branch on none removed, and a wrong land type, number, life, counter kind, step or link never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn land_search(types: &[&str], life: i32) -> String {
    let types: Vec<String> = types
        .iter()
        .map(|kind| format!(r#"{{"Subtype":"{kind}"}}"#))
        .collect();
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"Sacrifice":{{"filter":["This"]}}}},{{"If":{{"condition":"Performed","then":[{{"Search":{{"filter":[{{"Supertype":"Basic"}},{{"AnyOf":[{}]}}],"as":"land"}}}},{{"Move":{{"what":{{"Bound":"land"}},"to":{{"Battlefield":{{"tapped":true}}}}}}}},{{"Shuffle":{{}}}},{{"GainLife":{{"amount":{life}}}}}]}}}}]}}}}"#,
        types.join(",")
    )
}

#[rstest]
#[case::plains_island_swamp(
    &["Plains", "Island", "Swamp"], 1,
    "When ~ enters, sacrifice it. When you do, search your library for a basic Plains, Island, or Swamp card, put it onto the battlefield tapped, then shuffle and you gain 1 life."
)]
#[case::island_swamp_mountain(
    &["Island", "Swamp", "Mountain"], 2,
    "When ~ enters, sacrifice it. When you do, search your library for a basic Island, Swamp, or Mountain card, put it onto the battlefield tapped, then shuffle and you gain 2 life."
)]
fn a_sacrifice_links_a_search_by_when_you_do(
    #[case] types: &[&str],
    #[case] life: i32,
    #[case] clause: &str,
) {
    assert!(written(&land_search(types, life), clause), "{clause}");
}

#[rstest]
#[case::another_type(
    "When ~ enters, sacrifice it. When you do, search your library for a basic Plains, Island, or Mountain card, put it onto the battlefield tapped, then shuffle and you gain 1 life."
)]
#[case::another_life(
    "When ~ enters, sacrifice it. When you do, search your library for a basic Plains, Island, or Swamp card, put it onto the battlefield tapped, then shuffle and you gain 2 life."
)]
#[case::untapped(
    "When ~ enters, sacrifice it. When you do, search your library for a basic Plains, Island, or Swamp card, put it onto the battlefield, then shuffle and you gain 1 life."
)]
#[case::no_sacrifice(
    "When ~ enters, search your library for a basic Plains, Island, or Swamp card, put it onto the battlefield tapped, then shuffle and you gain 1 life."
)]
#[case::another_event(
    "Whenever ~ attacks, sacrifice it. When you do, search your library for a basic Plains, Island, or Swamp card, put it onto the battlefield tapped, then shuffle and you gain 1 life."
)]
fn a_sacrifice_and_its_search_do_not_read_as_another_land_type_life_or_link(#[case] clause: &str) {
    assert!(!written(
        &land_search(&["Plains", "Island", "Swamp"], 1),
        clause
    ));
}

fn druid(most: u32, cost: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"Pay":{{"cost":[{{"{cost}":{{"filter":[{{"Type":"Land"}}]}}}}],"paid":[{{"Search":{{"filter":[{{"Supertype":"Basic"}},{{"Type":"Land"}}],"count":{{"min":0,"max":{most}}},"as":"lands"}}}},{{"Move":{{"what":{{"Bound":"lands"}},"to":{{"Battlefield":{{"tapped":true}}}}}}}},{{"Shuffle":{{}}}}]}}}}]}}}}"#
    )
}

#[test]
fn a_payment_may_buy_a_search_of_several_steps() {
    let clause = "When ~ enters, you may sacrifice a land. If you do, search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle.";
    assert!(written(&druid(2, "Sacrifice"), clause));
}

#[rstest]
#[case::another_number(
    "When ~ enters, you may sacrifice a land. If you do, search your library for up to three basic land cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::not_optional(
    "When ~ enters, sacrifice a land. If you do, search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::another_cost(
    "When ~ enters, you may sacrifice a creature. If you do, search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::no_link(
    "When ~ enters, you may sacrifice a land. Search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle."
)]
fn a_payment_and_its_search_do_not_read_as_another_number_or_cost(#[case] clause: &str) {
    assert!(!written(&druid(2, "Sacrifice"), clause));
}

fn heart(most: u32) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"if":{{"Compare":{{"left":{{"Largest":{{"who":"Opponents","of":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}}},"op":"Ge","right":3}}}},"effects":[{{"Sacrifice":{{"filter":["This"]}}}},{{"Search":{{"filter":[{{"Type":"Creature"}}],"count":{{"min":0,"max":{most}}},"as":"found"}}}},{{"Move":{{"what":{{"Bound":"found"}},"to":{{"Battlefield":{{}}}}}}}},{{"Shuffle":{{}}}}]}}}}"#
    )
}

#[rstest]
#[case::two(2, "two")]
#[case::three(3, "three")]
fn a_sacrifice_and_a_search_may_be_one_sentence(#[case] most: u32, #[case] word: &str) {
    let clause = format!(
        "At the beginning of your upkeep, if an opponent controls three or more creatures, sacrifice ~, search your library for up to {word} creature cards, put those cards onto the battlefield, then shuffle."
    );
    assert!(written(&heart(most), &clause), "{clause}");
}

#[rstest]
#[case::another_number(
    "At the beginning of your upkeep, if an opponent controls three or more creatures, sacrifice ~, search your library for up to three creature cards, put those cards onto the battlefield, then shuffle."
)]
#[case::another_step(
    "At the beginning of your end step, if an opponent controls three or more creatures, sacrifice ~, search your library for up to two creature cards, put those cards onto the battlefield, then shuffle."
)]
#[case::another_count(
    "At the beginning of your upkeep, if an opponent controls four or more creatures, sacrifice ~, search your library for up to two creature cards, put those cards onto the battlefield, then shuffle."
)]
fn a_sacrifice_and_a_search_do_not_read_as_another_number_step_or_count(#[case] clause: &str) {
    assert!(!written(&heart(2), clause));
}

fn bounty(step: &str, kind: &str, adds: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"{step}","whose":"You"}}}},"effects":[{{"RemoveCounters":{{"from":"This","counter":{{"Other":"{kind}"}},"count":{{"Counters":{{"on":"This","counter":{{"Other":"{kind}"}}}}}}}}}},{{"If":{{"condition":{{"Not":"Performed"}},"then":[{{"PutCounters":{{"on":"This","counter":{{"Other":"{kind}"}}}}}},{{"Draw":{{}}}}],"otherwise":[{{"AddMana":{{"mana":{{"Fixed":[{adds}]}}}}}}]}}}}]}}}}"#
    )
}

const MANA: &str = r#""Colorless","Green","Blue""#;

#[test]
fn a_removal_of_every_counter_may_branch_on_none_removed() {
    let clause = "At the beginning of your first main phase, remove all flood counters from ~. If no counters were removed this way, put a flood counter on ~ and draw a card. Otherwise, add {C}{G}{U}.";
    assert!(written(&bounty("Main1", "flood", MANA), clause));
}

#[rstest]
#[case::second_main(
    "At the beginning of your second main phase, remove all flood counters from ~. If no counters were removed this way, put a flood counter on ~ and draw a card. Otherwise, add {C}{G}{U}."
)]
#[case::another_kind(
    "At the beginning of your first main phase, remove all tide counters from ~. If no counters were removed this way, put a tide counter on ~ and draw a card. Otherwise, add {C}{G}{U}."
)]
#[case::another_mana(
    "At the beginning of your first main phase, remove all flood counters from ~. If no counters were removed this way, put a flood counter on ~ and draw a card. Otherwise, add {C}{G}{G}."
)]
#[case::branches_swapped(
    "At the beginning of your first main phase, remove all flood counters from ~. If counters were removed this way, put a flood counter on ~ and draw a card. Otherwise, add {C}{G}{U}."
)]
#[case::no_otherwise(
    "At the beginning of your first main phase, remove all flood counters from ~. If no counters were removed this way, put a flood counter on ~ and draw a card."
)]
fn a_branch_on_none_removed_does_not_read_as_another_step_kind_mana_or_branch(
    #[case] clause: &str,
) {
    assert!(!written(&bounty("Main1", "flood", MANA), clause));
}
