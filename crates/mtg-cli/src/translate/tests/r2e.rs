//! Goal: round 2 pack E (mana riders, mana kinds, small costs) translates each construct to the rule it names, one rstest table per rule.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn enchantment(lines: &str) -> String {
    format!("Name:T\nTypes:Enchantment\n{lines}\n")
}

fn artifact(lines: &str) -> String {
    format!("Name:T\nManaCost:2\nTypes:Artifact\n{lines}\n")
}

#[rstest]
#[case::mandatory_sacrifice(
    "T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | Execute$ TrigSac\nSVar:TrigSac:AB$ Draw | NumCards$ 3 | Cost$ Mandatory Sac<1/CARDNAME>",
    json!([{"Sacrifice": {"filter": ["This"]}}, {"If": {"condition": "Performed", "then": [{"Draw": {"count": 3}}]}}])
)]
#[case::exile_the_dead_card(
    "T:Mode$ ChangesZone | ValidCard$ Card.Self | Origin$ Battlefield | Destination$ Graveyard | Execute$ TrigImm\nSVar:TrigImm:AB$ ImmediateTrigger | Cost$ ExileAnyGrave<1/Card.TriggeredNewCard> | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw | NumCards$ 1",
    json!([{"Pay": {"cost": [{"ExileFromGraveyard": {"any_player": true, "filter": [{"Is": {"Arrived": {"of": "This", "zone": "Graveyard"}}}]}}],
        "paid": [{"Reflexive": {"effects": [{"Draw": {}}]}}]}}])
)]
#[case::draw_then_discard(
    "T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | Execute$ TrigLoot\nSVar:TrigLoot:AB$ Discard | Defined$ You | Mode$ TgtChoose | NumCards$ 1 | Cost$ Draw<1/You>",
    json!([{"Pay": {"cost": [{"Draw": 1}], "paid": [{"Discard": {}}]}}])
)]
fn a_cost_in_a_resolving_part_becomes_its_effects(#[case] lines: &str, #[case] effects: Value) {
    let text = enchantment(lines);
    let found = abilities(&text);
    assert!(!found.is_empty(), "{:?}", gaps(&text));
    assert_eq!(found[0]["Triggered"]["effects"], effects);
}

#[rstest]
#[case::persistent(
    "Produced$ R | PersistentMana$ True",
    json!({"AddMana": {"mana": {"Fixed": ["Red"]}, "lasts": "Turn"}})
)]
#[case::combat(
    "Produced$ R | Amount$ 2 | CombatMana$ True",
    json!({"AddMana": {"mana": {"Fixed": ["Red", "Red"]}, "lasts": "Combat"}})
)]
#[case::activated(
    "Produced$ C | RestrictValid$ Activated",
    json!({"AddMana": {"mana": {"Fixed": ["Colorless"]}, "only": [{"Ability": {}}]}})
)]
#[case::cost_with_x(
    "Produced$ C | RestrictValid$ CostContainsX",
    json!({"AddMana": {"mana": {"Fixed": ["Colorless"]}, "only": ["CostWithX"]}})
)]
#[case::artifact_spells_and_abilities(
    "Produced$ C | RestrictValid$ Spell.Artifact,Activated.Artifact+inZoneBattlefield",
    json!({"AddMana": {"mana": {"Fixed": ["Colorless"]}, "only": [
        {"Spell": {"types": ["Artifact"]}},
        {"Ability": {"types": ["Artifact"], "facts": ["OnBattlefield"]}}]}})
)]
#[case::subtype_creature_spells(
    "Produced$ R | RestrictValid$ Spell.Creature+Pirate",
    json!({"AddMana": {"mana": {"Fixed": ["Red"]}, "only": [{"Spell": {"types": ["Creature"], "subtypes": ["Pirate"]}}]}})
)]
#[case::cant_cast_nonartifact(
    "Produced$ C | RestrictValid$ CantCastNonArtifactSpells",
    json!({"AddMana": {"mana": {"Fixed": ["Colorless"]}, "only": [{"Spell": {"types": ["Artifact"]}}, {"Ability": {}}]}})
)]
#[case::multicolored(
    "Produced$ Any | RestrictValid$ Spell.MultiColor",
    json!({"AddMana": {"mana": {"AnyColor": 1}, "only": [{"Spell": {"multicolored": true}}]}})
)]
fn restricted_and_lasting_mana_reads_as_the_rule(#[case] line: &str, #[case] effect: Value) {
    let text = artifact(&format!("A:AB$ Mana | Cost$ T | {line}"));
    let found = abilities(&text);
    assert!(!found.is_empty(), "{:?}", gaps(&text));
    assert_eq!(found[0]["Activated"]["effects"][0], effect);
}

#[rstest]
#[case::unknown_subtype("Spell.Zorp", "key:Mana.RestrictValid=Spell.Zorp")]
#[case::two_subtypes("Spell.Pirate+Rogue", "key:Mana.RestrictValid=Spell.Pirate+Rogue")]
#[case::face_down("Static.isTurnFaceUp", "key:Mana.RestrictValid=Static.isTurnFaceUp")]
fn a_restriction_that_is_not_read_stays_a_gap(#[case] restriction: &str, #[case] gap: &str) {
    let text = artifact(&format!(
        "A:AB$ Mana | Cost$ T | Produced$ C | RestrictValid$ {restriction}"
    ));
    assert!(
        gaps(&text).iter().any(|found| found == gap),
        "{:?}",
        gaps(&text)
    );
}

#[rstest]
#[case::scry_for_a_creature(
    "TriggersWhenSpent$ TrigSpent",
    "SVar:TrigSpent:Mode$ SpellCast | ValidCard$ Creature | ValidActivatingPlayer$ You | Execute$ TrigScry\nSVar:TrigScry:DB$ Scry | ScryNum$ 1",
    json!([{"When": {"spell": {"types": ["Creature"]}, "effects": [{"Scry": {}}]}}])
)]
fn mana_that_triggers_when_spent_carries_a_rider(
    #[case] key: &str,
    #[case] svars: &str,
    #[case] only: Value,
) {
    let text = artifact(&format!(
        "A:AB$ Mana | Cost$ T | Produced$ C | {key}\n{svars}"
    ));
    let found = abilities(&text);
    assert!(!found.is_empty(), "{:?}", gaps(&text));
    assert_eq!(found[0]["Activated"]["effects"][0]["AddMana"]["only"], only);
}

#[rstest]
#[case::basic_cost_in_mana(
    "S:Mode$ RaiseCost | ValidCard$ Card.White | Activator$ You | Type$ Spell | Cost$ W | Description$ White spells you cast cost {W} more to cast.",
    json!("{W}")
)]
fn a_raise_cost_by_mana_is_an_increase(#[case] line: &str, #[case] mana: Value) {
    let text = enchantment(line);
    let found = abilities(&text);
    assert!(!found.is_empty(), "{:?}", gaps(&text));
    assert_eq!(found[0]["CostModifier"]["mana"], mana);
    assert_eq!(found[0]["CostModifier"]["change"], "Increase");
}

#[rstest]
#[case::discard_cards(
    "S:Mode$ RaiseCost | ValidCard$ Card.Self | Activator$ You | Type$ Spell | Cost$ Discard<2/Creature/creature card> | EffectZone$ All | Description$ As an additional cost to cast this spell, discard two creature cards.",
    true
)]
#[case::optional_exile_stays_a_gap(
    "S:Mode$ RaiseCost | ValidCard$ Card.Self | Activator$ You | Type$ Spell | Cost$ Discard<2/Creature/creature card> | EffectZone$ All | Description$ As an additional cost to cast this spell, you may discard two creature cards.",
    false
)]
#[case::other_spells_stay_a_gap(
    "S:Mode$ RaiseCost | ValidCard$ Card | Activator$ You | Type$ Spell | Cost$ Discard<2/Creature/creature card> | Description$ Spells you cast cost an additional discard.",
    false
)]
fn an_additional_cost_of_the_spell_itself_is_required(#[case] line: &str, #[case] read: bool) {
    let text = format!("Name:T\nManaCost:1 B\nTypes:Sorcery\n{line}\nA:SP$ Draw | NumCards$ 1\n");
    let found = abilities(&text);
    assert_eq!(
        found.iter().any(|a| a.get("AdditionalCost").is_some()),
        read,
        "{:?}",
        gaps(&text)
    );
}
