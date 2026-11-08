//! Goal: targets named by combat or damage ("creature blocking it", "creature it's blocking", "creature that was dealt damage this turn", "creature that's attacking you") and by a player with battles and planeswalkers ("player or battle", "opponent, battle, or planeswalker") read as the rules and are written back as the same text; other words after them stay with the table.
use super::*;

#[rstest]
#[case::blocking_it("~ deals 1 damage to target creature blocking it.")]
#[case::it_is_blocking("~ deals 1 damage to target creature it's blocking.")]
#[case::either_way("~ deals 1 damage to target creature blocking or blocked by it.")]
#[case::dealt_damage("~ deals 1 damage to target creature that was dealt damage this turn.")]
#[case::attacking_you("~ deals 1 damage to target creature without flying that's attacking you.")]
#[case::player_or_battle("~ deals 2 damage to target player or battle.")]
#[case::player_or_planeswalker("~ deals 2 damage to target player or planeswalker.")]
#[case::opponent_battle_or_planeswalker(
    "~ deals 2 damage to target opponent, battle, or planeswalker."
)]
fn w19d_a_target_phrase_reads_and_is_written_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::blocking_a_creature("~ deals 1 damage to target creature blocking this turn.")]
#[case::attacking_them("~ deals 1 damage to target creature that's attacking them.")]
#[case::player_or_land("~ deals 2 damage to target player or land.")]
#[case::battle_alone_with_a_comma("~ deals 2 damage to target player, battle.")]
fn w19d_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::the_rule_lists_planeswalker_first(
    r#"{"Spell":{"targets":[{"what":{"Either":["Opponent",{"Object":[{"Type":"Planeswalker"}]},{"Object":[{"Type":"Battle"}]}]}}],"effects":[{"DealDamage":{"amount":2,"to":{"Target":0}}}]}}"#
)]
fn w19d_the_order_of_kinds_in_the_rule_does_not_change_the_text(#[case] json: &str) {
    let wordings = render_variants(&ability(json));
    assert!(
        wordings
            .iter()
            .any(|written| same_text(written, "~ deals 2 damage to target opponent, battle, or planeswalker.")),
        "{wordings:?}"
    );
}
