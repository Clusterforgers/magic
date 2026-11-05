//! Goal: round 7 piece 3 translates `Effect` lines with `ForgetOnCast$` to a rule for the next spell its controller casts (`NextSpell`): the cost change, flash, a free cast or a help keyword, for the spells the line names, and names a gap for what the engine does not play.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn artifact(forget: &str, rest: &str, static_line: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nA:AB$ Effect | Cost$ T | StaticAbilities$ S | ForgetOnCast$ {forget}{rest}\nSVar:S:{static_line}\n"
    )
}

#[rstest]
#[case::any_spell(
    "Card.YouCtrl", "",
    "Mode$ ReduceCost | Type$ Spell | Activator$ You | Amount$ 1",
    json!({"NextSpell": {"benefit": {"Cost": {"change": "Reduce", "mana": "{1}"}}}}), "UntilEndOfTurn"
)]
#[case::a_kind_for_good(
    "Creature.Elf.YouCtrl", " | Duration$ Permanent",
    "Mode$ ReduceCost | Type$ Spell | ValidCard$ Creature.Elf | Activator$ You | Amount$ 2",
    json!({"NextSpell": {"what": [{"Subtype": "Elf"}], "benefit": {"Cost": {"change": "Reduce", "mana": "{1}", "per": 2}}}}), "Forever"
)]
#[case::flash(
    "Sorcery.YouCtrl", "",
    "Mode$ CastWithFlash | ValidCard$ Sorcery | ValidSA$ Spell | Caster$ You",
    json!({"NextSpell": {"what": [{"Type": "Sorcery"}], "benefit": "Flash"}}), "UntilEndOfTurn"
)]
#[case::convoke(
    "Card.YouCtrl", "",
    "Mode$ Continuous | Affected$ Card.YouCtrl | AffectedZone$ Stack | AddKeyword$ Convoke",
    json!({"NextSpell": {"benefit": {"Keyword": "Convoke"}}}), "UntilEndOfTurn"
)]
fn the_next_spell_gets_what_the_line_gives(
    #[case] forget: &str,
    #[case] rest: &str,
    #[case] line: &str,
    #[case] rule: Value,
    #[case] duration: &str,
) {
    let text = artifact(forget, rest, line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    let effects = &abilities(&text)[0]["Activated"]["effects"];
    assert_eq!(effects[0]["Rule"]["rule"], rule);
    assert_eq!(effects[0]["Rule"]["duration"], json!(duration));
}

#[rstest]
#[case::variable_amount(
    "Card.YouCtrl",
    "Mode$ ReduceCost | Type$ Spell | Activator$ You | Amount$ X",
    "key:Effect.ForgetOnCast-variable-amount"
)]
#[case::cascade(
    "Card.YouCtrl",
    "Mode$ Continuous | Affected$ Card.YouCtrl | AffectedZone$ Stack | AddKeyword$ Cascade",
    "key:Effect.ForgetOnCast-keyword=\"Cascade\""
)]
#[case::not_the_casters_spells(
    "Card",
    "Mode$ ReduceCost | Type$ Spell | Activator$ You | Amount$ 1",
    "key:Effect.ForgetOnCast-not-your-spells"
)]
#[case::another_set_of_spells(
    "Creature.Elf.YouCtrl",
    "Mode$ ReduceCost | Type$ Spell | ValidCard$ Creature.Goblin | Activator$ You | Amount$ 1",
    "static:CostModifier-next-spell-shape"
)]
#[case::remembered_cards(
    "Card.IsRemembered",
    "Mode$ ReduceCost | Type$ Spell | Activator$ You | Amount$ 1",
    "key:Effect.ForgetOnCast=Card.IsRemembered"
)]
fn a_line_the_engine_does_not_play_is_a_gap(
    #[case] forget: &str,
    #[case] line: &str,
    #[case] gap: &str,
) {
    assert_eq!(gaps(&artifact(forget, "", line)), [gap]);
}
