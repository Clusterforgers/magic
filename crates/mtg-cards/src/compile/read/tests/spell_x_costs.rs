//! Goal: the effects of a spell whose required additional cost counts X read that X ("-X/-X", "draw X cards and lose X life") and write back as the same text, and read nothing of an X that no cost of the spell announces.
use super::*;

#[rstest]
#[case::shrink_all("All creatures get -X/-X until end of turn.")]
#[case::shrink_yours("Creatures you control get -X/-X until end of turn.")]
#[case::shrink_theirs("Creatures your opponents control get -X/-X until end of turn.")]
#[case::grow_all("All creatures get +X/+X until end of turn.")]
#[case::draw_and_lose("You draw X cards and you lose X life.")]
#[case::draw("Draw X cards.")]
fn effects_that_read_the_x_of_the_cost_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, true);
}

#[test]
fn a_negative_pump_counted_by_a_where_clause_needs_no_cost() {
    golden(
        "Target creature gets -X/-X until end of turn, where X is the number of creatures you control.",
        ClauseKind::Spell,
        false,
    );
}

#[rstest]
#[case::shrink_all("All creatures get -X/-X until end of turn.")]
#[case::shrink_one("Target creature gets -X/-X until end of turn.")]
#[case::draw_and_lose("You draw X cards and you lose X life.")]
fn without_a_cost_that_announces_x_they_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None);
}

#[test]
fn a_negative_x_is_the_x_announced_times_minus_one() {
    reads_as(
        "All creatures get -X/-X until end of turn.",
        ClauseKind::Spell,
        true,
        r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":{"Times":{"by":-1,"of":"X"}},"toughness":{"Times":{"by":-1,"of":"X"}}}}],"duration":"UntilEndOfTurn"}}]}}"#,
    );
}
