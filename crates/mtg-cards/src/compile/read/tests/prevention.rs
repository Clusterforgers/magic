//! Goal: damage that can't be prevented reads as a static ability and as an effect, "prevent all combat damage that would be dealt this turn" reads as a shield, each writes back as the same text, and a recipient, a target source, a number of damage or a duration of another kind stays with the table.
use super::*;

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

#[rstest]
#[case::questing_beast(
    "Combat damage that would be dealt by creatures you control can't be prevented.",
    r#"{"Rule":{"rule":{"DamageCantBePrevented":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"combat":true}}}}"#
)]
#[case::all_damage(
    "Damage can't be prevented.",
    r#"{"Rule":{"rule":{"DamageCantBePrevented":{}}}}"#
)]
#[case::all_combat_damage(
    "Combat damage can't be prevented.",
    r#"{"Rule":{"rule":{"DamageCantBePrevented":{"combat":true}}}}"#
)]
#[case::noncombat_damage(
    "Noncombat damage can't be prevented.",
    r#"{"Rule":{"rule":{"DamageCantBePrevented":{"combat":false}}}}"#
)]
#[case::the_card_itself(
    "Damage that would be dealt by this creature can't be prevented.",
    r#"{"Rule":{"rule":{"DamageCantBePrevented":{"source":["This"]}}}}"#
)]
#[case::the_card_by_name(
    "Damage that would be dealt by ~ can't be prevented.",
    r#"{"Rule":{"rule":{"DamageCantBePrevented":{"source":["This"]}}}}"#
)]
#[case::combat_by_the_card(
    "Combat damage that would be dealt by this creature can't be prevented.",
    r#"{"Rule":{"rule":{"DamageCantBePrevented":{"source":["This"],"combat":true}}}}"#
)]
#[case::noncombat_by_a_group(
    "Noncombat damage that would be dealt by creatures you control can't be prevented.",
    r#"{"Rule":{"rule":{"DamageCantBePrevented":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"combat":false}}}}"#
)]
#[case::any_creature(
    "Damage that would be dealt by creatures can't be prevented.",
    r#"{"Rule":{"rule":{"DamageCantBePrevented":{"source":[{"Type":"Creature"}]}}}}"#
)]
#[case::artifacts_you_control(
    "Damage that would be dealt by artifacts you control can't be prevented.",
    r#"{"Rule":{"rule":{"DamageCantBePrevented":{"source":[{"Type":"Artifact"},{"ControlledBy":"You"}]}}}}"#
)]
fn damage_that_cant_be_prevented_reads_as_a_static_ability(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::all_damage(
    "Damage can't be prevented this turn.",
    r#"{"Spell":{"effects":[{"Rule":{"rule":{"DamageCantBePrevented":{}},"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::combat_damage(
    "Combat damage can't be prevented this turn.",
    r#"{"Spell":{"effects":[{"Rule":{"rule":{"DamageCantBePrevented":{"combat":true}},"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::then_damage(
    "Damage can't be prevented this turn. ~ deals 3 damage to any target.",
    r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"Rule":{"rule":{"DamageCantBePrevented":{}},"duration":"UntilEndOfTurn"}},{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#
)]
#[case::fog(
    "Prevent all combat damage that would be dealt this turn.",
    r#"{"Spell":{"effects":[{"Shield":{"when":{"Damage":{"combat":true}},"by":"Prevent","duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::all_damage_shield(
    "Prevent all damage that would be dealt this turn.",
    r#"{"Spell":{"effects":[{"Shield":{"when":{"Damage":{}},"by":"Prevent","duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::noncombat_shield(
    "Prevent all noncombat damage that would be dealt this turn.",
    r#"{"Spell":{"effects":[{"Shield":{"when":{"Damage":{"combat":false}},"by":"Prevent","duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::shield_then_a_gain(
    "Prevent all combat damage that would be dealt this turn. You gain 2 life.",
    r#"{"Spell":{"effects":[{"Shield":{"when":{"Damage":{"combat":true}},"by":"Prevent","duration":"UntilEndOfTurn"}},{"GainLife":{"amount":2}}]}}"#
)]
fn prevention_in_a_spell_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::a_recipient("Damage that would be dealt to you can't be prevented.")]
#[case::a_recipient_group(
    "Combat damage that would be dealt to creatures you control can't be prevented."
)]
#[case::a_single_source(
    "Combat damage that would be dealt by a creature you control can't be prevented."
)]
#[case::a_target_source("Damage that would be dealt by target creature can't be prevented.")]
#[case::no_source_named("Combat damage that would be dealt by can't be prevented.")]
#[case::the_damage("The damage can't be prevented.")]
#[case::prevented_or_redirected("Damage can't be prevented or redirected.")]
#[case::this_turn_in_a_static("Damage can't be prevented this turn.")]
#[case::redirect("Damage can't be redirected.")]
#[case::a_shield("Prevent all combat damage that would be dealt this turn.")]
#[case::no_damage("Combat can't be prevented.")]
#[case::damage_to_a_player("Damage dealt to players can't be prevented.")]
fn prevention_wordings_the_table_keeps_stay_unread_as_statics(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}

#[rstest]
#[case::a_recipient("Prevent all damage that would be dealt to you this turn.")]
#[case::a_target_recipient("Prevent all damage that would be dealt to target creature this turn.")]
#[case::a_source("Prevent all combat damage that would be dealt by target creature this turn.")]
#[case::the_next("Prevent the next 3 damage that would be dealt to any target this turn.")]
#[case::until_your_next_turn("Prevent all combat damage that would be dealt until your next turn.")]
#[case::without_a_duration("Prevent all combat damage that would be dealt.")]
#[case::a_source_in_the_rule(
    "Damage that would be dealt by creatures you control can't be prevented this turn."
)]
#[case::not_all("Prevent some combat damage that would be dealt this turn.")]
#[case::other_than_damage("Prevent all combat that would be dealt this turn.")]
fn prevention_wordings_the_table_keeps_stay_unread_as_spells(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case("Whenever ~ deals combat damage to an opponent, it deals that much damage to any target.")]
#[case(
    "Whenever ~ deals combat damage to a player, it deals 2 damage to target creature that player controls."
)]
#[case(
    "Whenever ~ deals combat damage to an opponent, ~ deals 1 damage to target planeswalker that player controls."
)]
#[case(
    "Whenever ~ deals combat damage to an opponent, destroy target artifact that player controls."
)]
fn a_trigger_about_a_damaged_player_reads_what_that_player_controls(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::a_spell("Destroy target creature that player controls.")]
#[case::a_trigger_about_a_creature(
    "Whenever ~ attacks, destroy target creature that player controls."
)]
#[case::a_death(
    "Whenever a creature you control dies, destroy target creature that player controls."
)]
fn that_player_controls_needs_a_player_the_trigger_is_about(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}

#[test]
fn the_trigger_of_questing_beast_reads_in_the_shape_of_its_file() {
    reads_as(
        "Whenever ~ deals combat damage to an opponent, it deals that much damage to target planeswalker that player controls.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"Opponents","combat":true}},"targets":[{"what":{"Object":[{"Type":"Planeswalker"},{"ControlledBy":"Triggering"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Triggering"}}]}}"#,
    );
}
