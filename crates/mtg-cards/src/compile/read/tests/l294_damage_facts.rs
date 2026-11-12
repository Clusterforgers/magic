//! Goal: "if it has dealt N or more damage to that player this turn" writes back from `DamageDealtByTo { This, Triggering }` after a damage trigger, for any N; another dealer, another recipient, another comparison and zero write nothing.
use super::*;

/// Hidetsugu's Vessel with the threshold and the amount as JSON.
fn vessel(amount: &str, op: &str, n: i64) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":["This"],"to":"EachPlayer"}}}},"if":{{"Compare":{{"left":{amount},"op":"{op}","right":{n}}}}},"effects":[{{"LoseGame":{{"who":"Triggering"}}}}]}}}}"#
    )
}

const BY_THIS: &str = r#"{"DamageDealtByTo":{"by":"This","to":"Triggering"}}"#;

#[rstest]
#[case::ten(
    10,
    "Whenever ~ deals damage to a player, if it has dealt 10 or more damage to that player this turn, they lose the game."
)]
#[case::three(
    3,
    "Whenever ~ deals damage to a player, if it has dealt three or more damage to that player this turn, that player loses the game."
)]
fn l294_damage_facts_write_back(#[case] n: i64, #[case] text: &str) {
    let written = render_variants(&ability(&vessel(BY_THIS, "Ge", n)));
    assert!(written.iter().any(|w| w == text), "{written:?}");
}

#[rstest]
#[case::another_dealer(r#"{"DamageDealtByTo":{"by":"Attached","to":"Triggering"}}"#, "Ge", 10)]
#[case::another_recipient(r#"{"DamageDealtByTo":{"by":"This","to":"You"}}"#, "Ge", 10)]
#[case::at_most(BY_THIS, "Le", 10)]
#[case::zero(BY_THIS, "Ge", 0)]
fn l294_damage_facts_near_misses_write_nothing(
    #[case] amount: &str,
    #[case] op: &str,
    #[case] n: i64,
) {
    let written = render_variants(&ability(&vessel(amount, op, n)));
    assert!(
        written
            .iter()
            .all(|w| !w.contains("damage to that player this turn")),
        "{written:?}"
    );
}

#[rstest]
#[case::that_player(
    "Whenever ~ deals combat damage to a player, that player loses the game.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"LoseGame":{"who":"Triggering"}}]}}"#
)]
#[case::target_opponent(
    "When ~ enters, target opponent loses the game.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"LoseGame":{"who":{"Target":0}}}]}}"#
)]
fn l294_losing_the_game_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::life("When ~ enters, target opponent loses 2 life.")]
#[case::all_counters("When ~ enters, target opponent loses all rad counters.")]
fn l294_losing_life_is_no_loss_of_the_game(#[case] text: &str) {
    let read = clause(text, ClauseKind::Triggered, false);
    assert!(
        !format!("{read:?}").contains("LoseGame"),
        "{text}: {read:?}"
    );
}
