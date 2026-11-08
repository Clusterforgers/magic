//! Goal: an `Effect` with a cost modifier of other spells is a `SpellCost` rule for its duration, a choice of "color or
//! colors" binds a set the animation sets, a loyalty flag in lower case is a loyalty ability and a target that the
//! Oracle text limits to "you control" but the script does not stays a gap; any other shape stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn effects(script: &str) -> Value {
    let found = abilities(script);
    let first = found.first().cloned().unwrap_or_default();
    first
        .get("Spell")
        .or_else(|| first.get("Activated"))
        .and_then(|body| body.get("effects"))
        .cloned()
        .unwrap_or_default()
}

const COST_SPELL: &str = "Name:T\nTypes:Sorcery\nA:SP$ Effect | StaticAbilities$ Cost";

#[rstest]
#[case::cheaper_artifacts_this_turn(
    "",
    "ReduceCost | ValidCard$ Artifact | Type$ Spell | Activator$ You | Amount$ 1",
    json!([{"Rule": {"rule": {"SpellCost": {"applies_to": ["Spell", {"Type": "Artifact"}, {"ControlledBy": "You"}], "change": "Reduce", "mana": "{1}"}}, "duration": "UntilEndOfTurn"}}])
)]
#[case::dearer_for_opponents_until_your_next_turn(
    " | Duration$ UntilYourNextTurn",
    "RaiseCost | ValidCard$ Card.nonCreature | Activator$ Opponent | Type$ Spell | Amount$ 2",
    json!([{"Rule": {"rule": {"SpellCost": {"applies_to": ["Spell", {"Not": {"Type": "Creature"}}, {"ControlledBy": "Opponents"}], "change": "Increase", "mana": "{2}"}}, "duration": "UntilYourNextTurn"}}])
)]
fn a_cost_modifier_for_a_time_is_a_spell_cost_rule(
    #[case] duration: &str,
    #[case] line: &str,
    #[case] expected: Value,
) {
    let script = format!("{COST_SPELL}{duration}\nSVar:Cost:Mode$ {line}\n");
    assert_eq!(effects(&script), expected, "{:?}", gaps(&script));
}

#[rstest]
#[case::of_abilities("ReduceCost | ValidCard$ Card | Type$ Ability | Activator$ You | Amount$ 1")]
#[case::of_itself("ReduceCost | ValidCard$ Card.Self | Type$ Spell | Amount$ 1")]
#[case::counted("ReduceCost | ValidCard$ Artifact | Type$ Spell | Activator$ You | Amount$ X")]
fn another_cost_modifier_is_no_spell_cost_rule(#[case] line: &str) {
    let script =
        format!("{COST_SPELL}\nSVar:Cost:Mode$ {line}\nSVar:X:Count$Valid Artifact.YouCtrl\n");
    assert_ne!(gaps(&script), Vec::<String>::new());
    assert!(!effects(&script).to_string().contains("SpellCost"));
}

#[rstest]
#[case::or_colors(" | OrColors$ True", Some("Colors"))]
#[case::one_color("", Some("Color"))]
#[case::other_or_colors(" | OrColors$ False", None)]
fn a_choice_of_colors_binds_what_the_animation_sets(
    #[case] extra: &str,
    #[case] kind: Option<&str>,
) {
    let script = format!(
        "Name:T\nTypes:Instant\nA:SP$ ChooseColor | Defined$ You{extra} | SubAbility$ Animate\nSVar:Animate:DB$ Animate | ValidTgts$ Creature | Colors$ ChosenColor | OverwriteColors$ True\nOracle:Target creature becomes the color or colors of your choice until end of turn.\n"
    );
    match kind {
        Some(kind) => assert_eq!(
            effects(&script),
            json!([
                {"ChooseValue": {"kind": kind, "as": "chosen"}},
                {"Continuous": {"affected": {"Target": 0}, "duration": "UntilEndOfTurn", "mods": [{"SetColorsChosen": "chosen"}]}}
            ]),
            "{:?}",
            gaps(&script)
        ),
        None => assert!(
            gaps(&script)
                .iter()
                .any(|gap| gap == "key:ChooseColor.OrColors=False")
        ),
    }
}

#[rstest]
#[case::lower_case("true")]
#[case::capital("True")]
fn a_loyalty_flag_in_any_case_is_a_loyalty_ability(#[case] flag: &str) {
    let script = format!(
        "Name:T\nTypes:Legendary Planeswalker Kiora\nLoyalty:7\nA:AB$ Untap | Cost$ SubCounter<1/LOYALTY> | Planeswalker$ {flag} | ValidTgts$ Permanent\n"
    );
    let found = abilities(&script);
    assert!(
        found.iter().any(|ability| ability.get("Loyalty").is_some()),
        "{found:?}"
    );
}

#[rstest]
#[case::wider(
    "Creature",
    "Target creature you control gains flying until end of turn.",
    true
)]
#[case::as_printed(
    "Creature.YouCtrl",
    "Target creature you control gains flying until end of turn.",
    false
)]
#[case::any_creature("Creature", "Target creature gains flying until end of turn.", false)]
#[case::another_target(
    "Creature",
    "Target creature you control gains flying until end of turn. Tap target creature.",
    false
)]
fn a_target_the_oracle_limits_to_your_permanents_stays_a_gap(
    #[case] valid: &str,
    #[case] oracle: &str,
    #[case] gapped: bool,
) {
    let script = format!(
        "Name:T\nTypes:Instant\nA:SP$ Pump | ValidTgts$ {valid} | KW$ Flying\nOracle:{oracle}\n"
    );
    assert_eq!(
        gaps(&script)
            .iter()
            .any(|gap| gap == "target:script-wider-than-oracle"),
        gapped
    );
}
