//! Goal: targets named by combat or damage ("creature blocking this creature", "creature that was dealt damage this turn", "creature that's attacking you") and by a player with battles and planeswalkers ("player or battle", "opponent, battle, or planeswalker") read as the rules and are written back as the same text; "it" names the card where an earlier word does; other words after them stay with the table.
use super::*;

#[rstest]
#[case::blocking_it("~ deals 1 damage to target creature blocking this creature.")]
#[case::blocked_by("~ deals 1 damage to target creature blocked by this creature.")]
#[case::either_way("~ deals 1 damage to target creature blocking or blocked by this creature.")]
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
#[case::blocking_it("~ deals 1 damage to target creature blocking this creature.", "~ deals 1 damage to target creature blocking it.")]
#[case::blocked_by("~ deals 1 damage to target creature blocked by this creature.", "~ deals 1 damage to target creature it's blocking.")]
#[case::either_way("~ deals 1 damage to target creature blocking or blocked by this creature.", "~ deals 1 damage to target creature blocking or blocked by it.")]
fn w19d_the_card_is_it_after_its_own_name(#[case] read: &str, #[case] oracle: &str) {
    let rule = spell(read).unwrap_or_else(|| panic!("not read: {read}"));
    let wordings = crate::compile::read::render::written_variants(&rule);
    assert!(
        wordings.iter().any(|written| same_text(written, oracle)),
        "{oracle}: {wordings:?}"
    );
}

#[rstest]
#[case::blocking_a_creature("~ deals 1 damage to target creature blocking this turn.")]
#[case::attacking_them("~ deals 1 damage to target creature that's attacking them.")]
#[case::player_or_land("~ deals 2 damage to target player or land.")]
#[case::battle_alone_with_a_comma("~ deals 2 damage to target player, battle.")]
fn w19d_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[test]
fn w19d_a_counter_put_on_the_attacker_has_the_older_choose_wording() {
    let rule = ability(
        r#"{"Triggered":{"when":{"GroupAttacksAt":{"at":"You","what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"AttackingPlayer":"You"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":{"Other":"stun"}}}]}}"#,
    );
    let wordings = crate::compile::read::render::written_variants(&rule);
    assert!(
        wordings.iter().any(|written| same_text(
            written,
            "Whenever an opponent attacks you, choose target creature attacking you. Put a stun counter on that creature."
        )),
        "{wordings:?}"
    );
}

#[rstest]
#[case::the_rule_lists_planeswalker_first(
    r#"{"Spell":{"targets":[{"what":{"Either":["Opponent",{"Object":[{"Type":"Planeswalker"}]},{"Object":[{"Type":"Battle"}]}]}}],"effects":[{"DealDamage":{"amount":2,"to":{"Target":0}}}]}}"#
)]
fn w19d_the_order_of_kinds_in_the_rule_does_not_change_the_text(#[case] json: &str) {
    let wordings = render_variants(&ability(json));
    assert!(
        wordings.iter().any(|written| same_text(
            written,
            "~ deals 2 damage to target opponent, battle, or planeswalker."
        )),
        "{wordings:?}"
    );
}
