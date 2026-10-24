//! Goal: the writers of continuous effects on a group or a chosen permanent give the oracle wording of a control taken from every object of a selector for good, a permanent chosen, held silent and unable to fight for as long as the source remains, a random creature taken for the turn with the rest destroyed, a creature or a permanent taken, untapped and hasted for the turn and each creature's power and toughness doubled, and a wrong group, duration, noun, mode or step never reads back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn subjugate(max: u32, duration: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Continuous":{{"affected":{{"Each":[{{"Type":"Creature"}},{{"Not":"Commander"}},{{"Compare":{{"stat":"ManaValue","op":"Le","to":{max}}}}}]}},"mods":[{{"SetController":"You"}}],"duration":"{duration}"}}}}]}}}}"#
    )
}

#[rstest]
#[case::three(
    3,
    "Gain control of each noncommander creature with mana value 3 or less."
)]
#[case::five(
    5,
    "Gain control of each noncommander creature with mana value 5 or less."
)]
fn control_of_every_small_noncommander_creature_is_written_with_its_limit(
    #[case] max: u32,
    #[case] clause: &str,
) {
    assert!(written(&subjugate(max, "Forever"), clause), "{clause}");
}

#[rstest]
#[case::another_limit("Gain control of each noncommander creature with mana value 4 or less.")]
#[case::commanders_too("Gain control of each creature with mana value 3 or less.")]
#[case::target("Gain control of target noncommander creature with mana value 3 or less.")]
#[case::for_the_turn(
    "Gain control of each noncommander creature with mana value 3 or less until end of turn."
)]
fn control_of_a_group_does_not_read_as_another_limit_group_or_duration(#[case] clause: &str) {
    assert!(!written(&subjugate(3, "Forever"), clause));
}

fn dragon(mods: &str, duration: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"targets":[{{"what":{{"Object":[{{"AnyOf":[{{"Subtype":"Human"}},{{"Type":"Artifact"}}]}},{{"ControlledBy":"Opponents"}}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{mods}],"duration":"{duration}"}}}}]}}}}"#
    )
}

const SILENCED: &str = r#"{"SetController":"You"},"LoseAbilities","CantAttack","CantBlock""#;

#[test]
fn a_permanent_chosen_taken_silenced_and_held_back_is_written_as_the_oracle_does() {
    let clause = "When ~ enters, choose target Human or artifact an opponent controls. For as long as ~ remains on the battlefield, gain control of that permanent, it loses all abilities, and it can't attack or block.";
    assert!(written(
        &dragon(SILENCED, "WhileSourceOnBattlefield"),
        clause
    ));
}

#[rstest]
#[case::while_you_control_it(SILENCED, "WhileYouControlSource")]
#[case::forever(SILENCED, "Forever")]
#[case::may_block(
    r#"{"SetController":"You"},"LoseAbilities","CantAttack""#,
    "WhileSourceOnBattlefield"
)]
#[case::keeps_abilities(
    r#"{"SetController":"You"},"CantAttack","CantBlock""#,
    "WhileSourceOnBattlefield"
)]
fn a_silenced_permanent_does_not_read_as_another_duration_or_fewer_restrictions(
    #[case] mods: &str,
    #[case] duration: &str,
) {
    let clause = "When ~ enters, choose target Human or artifact an opponent controls. For as long as ~ remains on the battlefield, gain control of that permanent, it loses all abilities, and it can't attack or block.";
    assert!(!written(&dragon(mods, duration), clause));
}

const LOTTERY: &str = r#"{"Spell":{"effects":[{"ChooseAtRandom":{"from":{"Each":[{"Type":"Creature"}]},"as":"pick"}},{"Continuous":{"affected":{"Bound":"pick"},"mods":[{"SetController":"You"},{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}},{"Untap":{"what":{"Bound":"pick"}}},{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Not":{"Is":{"Bound":"pick"}}}]}}}]}}"#;

#[test]
fn a_random_creature_taken_for_the_turn_with_the_rest_destroyed_is_written() {
    let clause = "Choose a creature at random. You gain control of that creature until end of turn. Untap it. It gains haste until end of turn. Then destroy all other creatures.";
    assert!(written(LOTTERY, clause));
}

#[rstest]
#[case::lands(
    "Choose a land at random. You gain control of that land until end of turn. Untap it. It gains haste until end of turn. Then destroy all other lands."
)]
#[case::no_haste(
    "Choose a creature at random. You gain control of that creature until end of turn. Untap it. Then destroy all other creatures."
)]
#[case::keeps_them(
    "Choose a creature at random. You gain control of that creature until end of turn. Untap it. It gains haste until end of turn."
)]
fn the_lottery_does_not_read_as_another_kind_step_or_keyword(#[case] clause: &str) {
    assert!(!written(LOTTERY, clause));
}

fn doubling(over: &str, stat: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"BeginCombat"}}}},"effects":[{{"RepeatEach":{{"over":{{"Each":[{over}]}},"as":"creature","effects":[{{"Continuous":{{"affected":{{"Bound":"creature"}},"mods":[{{"ModifyPT":{{"power":{{"Stat":{{"of":{{"Bound":"creature"}},"stat":"Power"}}}},"toughness":{{"Stat":{{"of":{{"Bound":"creature"}},"stat":"{stat}"}}}}}}}}],"duration":"UntilEndOfTurn"}}}}]}}}}]}}}}"#
    )
}

#[test]
fn each_creatures_power_and_toughness_doubled_is_written() {
    let clause = "At the beginning of each combat, double the power and toughness of each creature you control until end of turn.";
    assert!(written(
        &doubling(r#"{"Type":"Creature"},{"ControlledBy":"You"}"#, "Toughness"),
        clause
    ));
}

#[rstest]
#[case::power_twice(r#"{"Type":"Creature"},{"ControlledBy":"You"}"#, "Power")]
#[case::all_creatures(r#"{"Type":"Creature"}"#, "Toughness")]
fn a_doubling_does_not_read_as_a_different_stat_or_group(#[case] over: &str, #[case] stat: &str) {
    let clause = "At the beginning of each combat, double the power and toughness of each creature you control until end of turn.";
    assert!(!written(&doubling(over, stat), clause));
}

fn conscripts(noun: &str) -> String {
    let what = if noun == "permanent" {
        String::new()
    } else {
        format!(r#"{{"Type":"{noun}"}}"#)
    };
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"targets":[{{"what":{{"Object":[{what}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"SetController":"You"}}],"duration":"UntilEndOfTurn"}}}},{{"Untap":{{"what":{{"Target":0}}}}}},{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"AddKeyword":"Haste"}}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
    )
}

#[rstest]
#[case::a_permanent(
    "permanent",
    "When ~ enters, gain control of target permanent until end of turn. Untap that permanent. It gains haste until end of turn."
)]
#[case::a_creature(
    "Creature",
    "When ~ enters, gain control of target creature until end of turn. Untap that creature. It gains haste until end of turn."
)]
fn a_target_taken_untapped_and_hasted_names_its_noun(#[case] noun: &str, #[case] clause: &str) {
    assert!(written(&conscripts(noun), clause), "{clause}");
}

#[rstest]
#[case::wrong_noun_after(
    "permanent",
    "When ~ enters, gain control of target permanent until end of turn. Untap that creature. It gains haste until end of turn."
)]
#[case::artifact_for_a_permanent(
    "permanent",
    "When ~ enters, gain control of target artifact until end of turn. Untap that artifact. It gains haste until end of turn."
)]
#[case::no_haste(
    "Creature",
    "When ~ enters, gain control of target creature until end of turn. Untap that creature."
)]
fn a_taken_target_does_not_read_with_another_noun_or_a_missing_step(
    #[case] noun: &str,
    #[case] clause: &str,
) {
    assert!(!written(&conscripts(noun), clause), "{clause}");
}
