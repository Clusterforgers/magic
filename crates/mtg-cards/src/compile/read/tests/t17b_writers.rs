//! Goal: the writers of the continuous effects of spells, activated and triggered abilities (durations other than the turn, several targets that each change, protection from a group) and of the conditions of statics (a count with "another", a choice of colors, an Equipment or an Aura attached, cards drawn) give the oracle wording of the data, and another count, color, duration, kind or number never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// A spell with one continuous effect on the target, the group or the card.
fn pump(affected: &str, targets: &str, mods: &str, duration: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Continuous":{{"affected":{affected},"duration":"{duration}","mods":[{mods}]}}}}]{targets}}}}}"#
    )
}

const TARGET: &str = r#"{"Target":0}"#;
const OPPONENTS: &str = r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}"#;
const YOURS: &str = r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#;
const PLUS_TWO: &str = r#"{"ModifyPT":{"power":2,"toughness":2}}"#;
const MINUS_ONE: &str = r#"{"ModifyPT":{"power":-1,"toughness":-1}}"#;

/// The target specification of creatures chosen between `min` and `max`.
fn creatures(min: u32, max: u32) -> String {
    format!(
        r#","targets":[{{"bounds":{{"min":{min},"max":{max}}},"what":{{"Object":[{{"Type":"Creature"}}]}}}}]"#
    )
}

fn one_creature() -> String {
    r#","targets":[{"what":{"Object":[{"Type":"Creature"}]}}]"#.to_owned()
}

#[rstest]
#[case::plus_two(TARGET, PLUS_TWO, "Target creature gets +2/+2 until end of turn.")]
#[case::minus_one(
    OPPONENTS,
    MINUS_ONE,
    "Creatures your opponents control get -1/-1 until end of turn."
)]
#[case::flying(
    TARGET,
    r#"{"AddKeyword":"Flying"}"#,
    "Target creature gains flying until end of turn."
)]
#[case::two_keywords(
    TARGET,
    r#"{"AddKeyword":"Flying"},{"AddKeyword":"Vigilance"}"#,
    "Target creature gains flying and vigilance until end of turn."
)]
#[case::leading(YOURS, PLUS_TWO, "Until end of turn, creatures you control get +2/+2.")]
fn the_plain_continuous_wordings_stay_written(
    #[case] affected: &str,
    #[case] mods: &str,
    #[case] clause: &str,
) {
    let targets = if affected == TARGET {
        one_creature()
    } else {
        String::new()
    };
    assert!(
        written(&pump(affected, &targets, mods, "UntilEndOfTurn"), clause),
        "{clause}"
    );
}

#[rstest]
#[case::until_end_of_combat(
    OPPONENTS,
    "UntilEndOfCombat",
    "Creatures your opponents control get -1/-1 until end of combat."
)]
#[case::until_end_of_combat_leading(
    OPPONENTS,
    "UntilEndOfCombat",
    "Until end of combat, creatures your opponents control get -1/-1."
)]
#[case::until_the_end_of_your_next_turn(
    OPPONENTS,
    "UntilEndOfYourNextTurn",
    "Creatures your opponents control get -1/-1 until the end of your next turn."
)]
#[case::until_the_end_of_your_next_turn_leading(
    OPPONENTS,
    "UntilEndOfYourNextTurn",
    "Until the end of your next turn, creatures your opponents control get -1/-1."
)]
#[case::while_the_card_remains(
    OPPONENTS,
    "WhileSourceOnBattlefield",
    "Creatures your opponents control get -1/-1 for as long as ~ remains on the battlefield."
)]
#[case::while_you_control_the_card(
    OPPONENTS,
    "WhileYouControlSource",
    "Creatures your opponents control get -1/-1 for as long as you control ~."
)]
fn a_continuous_effect_is_written_with_its_duration(
    #[case] affected: &str,
    #[case] duration: &str,
    #[case] clause: &str,
) {
    assert!(
        written(&pump(affected, "", MINUS_ONE, duration), clause),
        "{clause}"
    );
}

#[rstest]
#[case::combat_for_the_turn(
    "UntilEndOfCombat",
    "Creatures your opponents control get -1/-1 until end of turn."
)]
#[case::turn_for_combat(
    "UntilEndOfTurn",
    "Creatures your opponents control get -1/-1 until end of combat."
)]
#[case::your_next_turn_for_the_end_of_it(
    "UntilYourNextTurn",
    "Creatures your opponents control get -1/-1 until the end of your next turn."
)]
#[case::remains_for_control(
    "WhileSourceOnBattlefield",
    "Creatures your opponents control get -1/-1 for as long as you control ~."
)]
#[case::control_for_remains(
    "WhileYouControlSource",
    "Creatures your opponents control get -1/-1 for as long as ~ remains on the battlefield."
)]
#[case::combat_for_good("UntilEndOfCombat", "Creatures your opponents control get -1/-1.")]
#[case::remains_opening(
    "WhileSourceOnBattlefield",
    "For as long as ~ remains on the battlefield, creatures your opponents control get -1/-1."
)]
fn another_duration_does_not_read_as_the_effects(#[case] duration: &str, #[case] clause: &str) {
    assert!(!written(&pump(OPPONENTS, "", MINUS_ONE, duration), clause));
}

#[test]
fn a_duration_in_an_activated_and_a_triggered_ability_is_written() {
    let effect = r#""effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfCombat","mods":[{"ModifyPT":{"power":2,"toughness":0}}]}}]"#;
    let attack =
        format!(r#"{{"Triggered":{{{effect},"when":{{"Unblocked":{{"what":["This"]}}}}}}}}"#);
    assert!(written(
        &attack,
        "Whenever ~ attacks and isn't blocked, it gets +2/+0 until end of combat."
    ));
    let pay = format!(r#"{{"Activated":{{"cost":[{{"Mana":"{{R}}"}}],{effect}}}}}"#);
    assert!(written(&pay, "{R}: ~ gets +2/+0 until end of combat."));
    assert!(!written(&pay, "{R}: ~ gets +2/+0 until end of turn."));
}

#[rstest]
#[case::up_to_two_each(
    creatures(0, 2),
    PLUS_TWO,
    "Up to two target creatures each get +2/+2 until end of turn."
)]
#[case::up_to_three(
    creatures(0, 3),
    MINUS_ONE,
    "Up to three target creatures each get -1/-1 until end of turn."
)]
#[case::exactly_two(
    creatures(2, 2),
    PLUS_TWO,
    "Two target creatures each get +2/+2 until end of turn."
)]
#[case::and_gain(
    creatures(0, 2),
    r#"{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Trample"}"#,
    "Up to two target creatures each get +2/+2 and gain trample until end of turn."
)]
#[case::leading_duration(
    creatures(0, 2),
    PLUS_TWO,
    "Until end of turn, up to two target creatures each get +2/+2."
)]
#[case::without_each(
    creatures(0, 2),
    PLUS_TWO,
    "Up to two target creatures get +2/+2 until end of turn."
)]
fn several_targets_that_each_change_are_written_with_each(
    #[case] targets: String,
    #[case] mods: &str,
    #[case] clause: &str,
) {
    assert!(
        written(&pump(TARGET, &targets, mods, "UntilEndOfTurn"), clause),
        "{clause}"
    );
}

#[rstest]
#[case::one_target(one_creature(), "Target creature each gets +2/+2 until end of turn.")]
#[case::one_target_plural(one_creature(), "Target creatures each get +2/+2 until end of turn.")]
#[case::one_or_two(
    creatures(1, 2),
    "Up to two target creatures each get +2/+2 until end of turn."
)]
#[case::another_number(
    creatures(0, 3),
    "Up to two target creatures each get +2/+2 until end of turn."
)]
#[case::another_exact_number(
    creatures(2, 2),
    "Three target creatures each get +2/+2 until end of turn."
)]
#[case::another_stat(
    creatures(0, 2),
    "Up to two target creatures each get +1/+1 until end of turn."
)]
fn each_is_not_written_for_one_target_or_another_count(
    #[case] targets: String,
    #[case] clause: &str,
) {
    assert!(!written(
        &pump(TARGET, &targets, PLUS_TWO, "UntilEndOfTurn"),
        clause
    ));
}

fn protection(sources: &str) -> String {
    pump(
        YOURS,
        "",
        &format!(r#"{{"AddKeyword":{{"Protection":[{sources}]}}}}"#),
        "UntilEndOfTurn",
    )
}

#[rstest]
#[case::not_human(
    r#"{"Type":"Creature"},{"Not":{"Subtype":"Human"}}"#,
    "Creatures you control gain protection from non-Human creatures until end of turn."
)]
#[case::of_a_subtype(
    r#"{"Type":"Creature"},{"Subtype":"Vampire"}"#,
    "Creatures you control gain protection from Vampire creatures until end of turn."
)]
#[case::of_a_color(
    r#"{"Color":"Red"}"#,
    "Creatures you control gain protection from red until end of turn."
)]
#[case::of_creatures(
    r#"{"Type":"Creature"}"#,
    "Creatures you control gain protection from creatures until end of turn."
)]
fn protection_from_a_group_of_sources_is_written(#[case] sources: &str, #[case] clause: &str) {
    assert!(written(&protection(sources), clause), "{clause}");
}

#[rstest]
#[case::another_kind(
    r#"{"Type":"Creature"},{"Not":{"Subtype":"Human"}}"#,
    "Creatures you control gain protection from non-Elf creatures until end of turn."
)]
#[case::the_kind_itself(
    r#"{"Type":"Creature"},{"Not":{"Subtype":"Human"}}"#,
    "Creatures you control gain protection from Human creatures until end of turn."
)]
#[case::every_creature(
    r#"{"Type":"Creature"},{"Not":{"Subtype":"Human"}}"#,
    "Creatures you control gain protection from creatures until end of turn."
)]
#[case::another_color(
    r#"{"Type":"Creature"},{"Subtype":"Vampire"}"#,
    "Creatures you control gain protection from red until end of turn."
)]
fn protection_from_another_group_does_not_read_back(#[case] sources: &str, #[case] clause: &str) {
    assert!(!written(&protection(sources), clause));
}

/// A static ability of the card itself, with the stat bonus or keywords and a condition.
fn conditioned(mods: &str, condition: &str) -> String {
    format!(r#"{{"Static":{{"affected":"This","mods":[{mods}],"condition":{condition}}}}}"#)
}

fn control(preds: &str, op: &str, number: u32) -> String {
    format!(r#"{{"Compare":{{"left":{{"Count":[{preds}]}},"op":"{op}","right":{number}}}}}"#)
}

fn attached(kind: &str) -> String {
    format!(
        r#"{{"Matches":{{"selector":[{{"HasAttached":[{{"Subtype":"{kind}"}}]}}],"subject":"This"}}}}"#
    )
}

const BONUS: &str = r#"{"ModifyPT":{"power":1,"toughness":1}}"#;
const LIFELINK: &str = r#"{"AddKeyword":"Lifelink"}"#;

#[rstest]
#[case::another_creature(
    BONUS,
    control(
        r#""Other",{"Color":"Blue"},{"Type":"Creature"},{"ControlledBy":"You"}"#,
        "Ge",
        1
    ),
    "~ gets +1/+1 as long as you control another blue creature."
)]
#[case::another_creature_first(
    BONUS,
    control(
        r#""Other",{"Color":"Blue"},{"Type":"Creature"},{"ControlledBy":"You"}"#,
        "Ge",
        1
    ),
    "As long as you control another blue creature, ~ gets +1/+1."
)]
#[case::two_colors(
    LIFELINK,
    control(
        r#"{"AnyOf":[{"Color":"White"},{"Color":"Black"}]},{"ControlledBy":"You"}"#,
        "Ge",
        1
    ),
    "~ has lifelink as long as you control a white or black permanent."
)]
#[case::no_opponent(
    BONUS,
    control(
        r#"{"Type":"Creature"},{"AnyOf":[{"Color":"White"},{"Color":"Blue"}]},{"ControlledBy":"Opponents"}"#,
        "Eq",
        0
    ),
    "~ gets +1/+1 as long as no opponent controls a white or blue creature."
)]
#[case::equipped(BONUS, attached("Equipment"), "~ gets +1/+1 as long as it's equipped.")]
#[case::enchanted(
    LIFELINK,
    attached("Aura"),
    "~ has lifelink as long as it's enchanted."
)]
#[case::equipped_first_with_two_parts(
    r#"{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Flying"}"#,
    attached("Equipment"),
    "As long as ~ is equipped, it gets +1/+1 and has flying."
)]
#[case::drawn(
    r#"{"AddKeyword":"DoubleStrike"}"#,
    r#"{"Compare":{"left":{"Tallied":{"what":"Drawn","who":"You"}},"op":"Ge","right":2}}"#.to_owned(),
    "~ has double strike as long as you've drawn two or more cards this turn."
)]
fn the_condition_of_a_static_ability_is_written(
    #[case] mods: &str,
    #[case] condition: String,
    #[case] clause: &str,
) {
    assert!(written(&conditioned(mods, &condition), clause), "{clause}");
}

#[rstest]
#[case::without_another(
    control(
        r#"{"Color":"Blue"},{"Type":"Creature"},{"ControlledBy":"You"}"#,
        "Ge",
        1
    ),
    "~ gets +1/+1 as long as you control another blue creature."
)]
#[case::another_color(
    control(
        r#""Other",{"Color":"Blue"},{"Type":"Creature"},{"ControlledBy":"You"}"#,
        "Ge",
        1
    ),
    "~ gets +1/+1 as long as you control another red creature."
)]
#[case::another_player(
    control(
        r#""Other",{"Color":"Blue"},{"Type":"Creature"},{"ControlledBy":"Opponents"}"#,
        "Ge",
        1
    ),
    "~ gets +1/+1 as long as you control another blue creature."
)]
#[case::two_creatures(
    control(
        r#""Other",{"Color":"Blue"},{"Type":"Creature"},{"ControlledBy":"You"}"#,
        "Ge",
        2
    ),
    "~ gets +1/+1 as long as you control another blue creature."
)]
#[case::one_of_two_colors(
    control(
        r#"{"Type":"Creature"},{"AnyOf":[{"Color":"White"},{"Color":"Blue"}]},{"ControlledBy":"Opponents"}"#,
        "Eq",
        0
    ),
    "~ gets +1/+1 as long as no opponent controls a white creature."
)]
#[case::you_for_an_opponent(
    control(
        r#"{"Type":"Creature"},{"AnyOf":[{"Color":"White"},{"Color":"Blue"}]},{"ControlledBy":"Opponents"}"#,
        "Eq",
        0
    ),
    "~ gets +1/+1 as long as no player controls a white or blue creature."
)]
#[case::aura_for_equipment(attached("Aura"), "~ gets +1/+1 as long as it's equipped.")]
#[case::equipment_for_aura(attached("Equipment"), "~ gets +1/+1 as long as it's enchanted.")]
fn another_count_color_or_attachment_does_not_read_as_the_condition(
    #[case] condition: String,
    #[case] clause: &str,
) {
    assert!(!written(&conditioned(BONUS, &condition), clause));
}

#[rstest]
#[case::three_cards(
    3,
    "~ has double strike as long as you've drawn two or more cards this turn."
)]
#[case::one_card(
    1,
    "~ has double strike as long as you've drawn two or more cards this turn."
)]
#[case::another_phrase(
    2,
    "~ has double strike as long as you've cast two or more cards this turn."
)]
fn the_cards_drawn_count_reads_only_its_own_number(#[case] number: u32, #[case] clause: &str) {
    let condition = format!(
        r#"{{"Compare":{{"left":{{"Tallied":{{"what":"Drawn","who":"You"}}}},"op":"Ge","right":{number}}}}}"#
    );
    assert!(!written(
        &conditioned(r#"{"AddKeyword":"DoubleStrike"}"#, &condition),
        clause
    ));
}
