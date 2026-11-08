//! Goal: round 16 pack e16b reads "As an additional cost to cast this spell, you may ..." (`Mode$ OptionalCost` of the
//! spell itself) into an optional `AdditionalCost`, "if this spell's additional cost was paid" into `AdditionalCostPaid`,
//! a target only the paid spell declares into a bound on it, "if you revealed a Dragon card or controlled a Dragon as you
//! cast this spell" into the reveal paid or a Dragon held as it was cast, and leaves an optional cost of other spells a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// An instant with the optional cost line `line` and the spell line `spell`.
fn card(line: &str, spell: &str) -> String {
    format!("Name:Subject\nManaCost:1 R\nTypes:Instant\nS:{line}\n{spell}\n")
}

const DAMAGE: &str = "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 2";

#[rstest]
#[case::sacrifice_an_artifact("Sac<1/Artifact>", json!([{"Sacrifice": {"filter": [{"Type": "Artifact"}]}}]))]
#[case::mana("2 R", json!("{2}{R}"))]
#[case::blight("Blight<1>", json!([{"Blight": 1}]))]
fn an_optional_cost_of_the_spell_itself_is_an_additional_cost_it_may_skip(
    #[case] cost: &str,
    #[case] expected: Value,
) {
    let line = format!(
        "Mode$ OptionalCost | EffectZone$ All | ValidCard$ Card.Self | ValidSA$ Spell | Cost$ {cost} | Description$ As an additional cost to cast this spell, you may pay."
    );
    let text = card(&line, DAMAGE);
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let found = abilities(&text);
    let extra = found.iter().find_map(|a| a.get("AdditionalCost")).unwrap();
    assert_eq!(extra["cost"], expected);
    assert!(extra.get("required").is_none(), "{extra}");
}

#[rstest]
#[case::of_green_permanent_spells(
    "Mode$ OptionalCost | ValidCard$ Permanent.Green | ValidSA$ Spell | Cost$ PayLife<2>"
)]
#[case::of_abilities(
    "Mode$ OptionalCost | ValidCard$ Card.Self | ValidSA$ Activated | Cost$ PayLife<2>"
)]
fn an_optional_cost_of_other_spells_or_abilities_is_a_gap(#[case] line: &str) {
    assert!(
        gaps(&card(line, DAMAGE))
            .iter()
            .any(|gap| gap.starts_with("static:OptionalCost")),
    );
}

const OPTIONAL: &str = "Mode$ OptionalCost | EffectZone$ All | ValidCard$ Card.Self | ValidSA$ Spell | Cost$ Sac<1/Artifact>";

#[rstest]
#[case::paid("True", json!("AdditionalCostPaid"))]
#[case::not_paid("False", json!({"Not": "AdditionalCostPaid"}))]
fn the_condition_on_the_optional_cost_reads_whether_it_was_paid(
    #[case] paid: &str,
    #[case] expected: Value,
) {
    let spell = format!(
        "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 2 | SubAbility$ DBGain\nSVar:DBGain:DB$ GainLife | LifeAmount$ 2 | Condition$ OptionalCost | ConditionOptionalPaid$ {paid}"
    );
    let text = card(OPTIONAL, &spell);
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let found = serde_json::to_string(&abilities(&text)).unwrap();
    assert!(found.contains(&expected.to_string()), "{found}");
}

#[test]
fn an_amount_by_the_optional_cost_picks_between_two_amounts() {
    let spell =
        "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ X\nSVar:X:Count$OptionalGenericCostPaid.4.2";
    let text = card(OPTIONAL, spell);
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let found = serde_json::to_string(&abilities(&text)).unwrap();
    assert!(found.contains(r#""AdditionalCostPaid""#), "{found}");
    assert!(found.contains("4") && found.contains("2"), "{found}");
}

#[test]
fn another_paid_word_is_a_gap() {
    let spell = "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 2 | Condition$ OptionalCost | ConditionOptionalPaid$ Maybe";
    assert!(
        gaps(&card(OPTIONAL, spell))
            .iter()
            .any(|gap| gap.contains("ConditionOptionalPaid")),
    );
}

#[test]
fn a_target_only_the_paid_spell_declares_is_chosen_only_when_it_was_paid() {
    let spell = "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 2 | SubAbility$ DBDestroy\nSVar:DBDestroy:DB$ Destroy | ValidTgts$ Artifact | Condition$ OptionalCost | ConditionOptionalPaid$ True";
    let text = card(OPTIONAL, spell);
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let found = serde_json::to_string(&abilities(&text)).unwrap();
    let bound = r#"{"When":{"condition":"AdditionalCostPaid","otherwise":0,"then":1}}"#;
    assert!(
        found.contains(&format!(r#""bounds":{{"max":{bound},"min":{bound}}}"#)),
        "{found}"
    );
}

const REVEAL: &str = "Mode$ OptionalCost | EffectZone$ All | ValidCard$ Card.Self | ValidSA$ Spell | Cost$ Reveal<1/Goblin>";
const PRESENCE: &str = r#"{"Any":["AdditionalCostPaid",{"AsCast":{"Compare":{"left":{"Count":[{"Subtype":"Goblin"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}]}"#;

/// A Dragonlord-like spell whose second line checks `Count$Presence_Goblin.1.0`, with the Oracle text `oracle`.
fn dragonlord(oracle: &str) -> String {
    format!(
        "Name:Subject\nManaCost:1 R\nTypes:Instant\nS:{REVEAL}\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 2 | SubAbility$ DBGain\nSVar:DBGain:DB$ GainLife | LifeAmount$ 2 | ConditionCheckSVar$ Count$Presence_Goblin.1.0\nOracle:{oracle}\n"
    )
}

#[test]
fn a_revealed_or_controlled_dragon_is_the_reveal_paid_or_a_dragon_held_as_the_spell_was_cast() {
    let text = dragonlord(
        "Subject deals 2 damage to any target. If you revealed a Goblin card or controlled a Goblin as you cast this spell, you gain 2 life.",
    );
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let found = serde_json::to_string(&abilities(&text)).unwrap();
    let presence: Value = serde_json::from_str(PRESENCE).unwrap();
    assert!(
        found.contains(&format!(r#""condition":{presence}"#)),
        "{found}"
    );
}

#[test]
fn a_presence_the_oracle_does_not_word_is_a_gap() {
    let text = dragonlord("Subject deals 2 damage to any target. You gain 2 life.");
    assert!(
        gaps(&text)
            .iter()
            .any(|gap| gap.contains("Presence_Goblin")),
        "{:?}",
        gaps(&text)
    );
}
