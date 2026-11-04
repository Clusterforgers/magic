//! Goal: round 3 piece P3 (zones): a move from the stack names the spell itself or the spell a cast trigger is about.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// The effects of the first ability of the card, or of its first trigger.
fn effects(text: &str) -> Vec<Value> {
    let found = abilities(text);
    let ability = found.first().cloned().unwrap_or_default();
    let inner = ability.as_object().and_then(|o| o.values().next()).cloned();
    inner
        .and_then(|a| a.get("effects").cloned())
        .and_then(|e| e.as_array().cloned())
        .unwrap_or_default()
}

#[rstest]
#[case::green_suns_zenith_shuffle(
    "Sorcery",
    "A:SP$ Draw | NumCards$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Stack | Destination$ Library | Shuffle$ True | Defined$ Parent\n",
    json!({"Move": {"what": "This", "to": {"Library": {"place": "Shuffled"}}}})
)]
#[case::healing_technique_exile(
    "Sorcery",
    "A:SP$ Draw | NumCards$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Stack | Destination$ Exile\n",
    json!({"Exile": {"what": "This"}})
)]
#[case::bottom(
    "Instant",
    "A:SP$ Draw | NumCards$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Stack | Destination$ Library | LibraryPosition$ -1 | Defined$ Parent\n",
    json!({"Move": {"what": "This", "to": {"Library": {"place": "Bottom"}}}})
)]
#[case::hand(
    "Sorcery",
    "A:SP$ Draw | NumCards$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Stack | Destination$ Hand | Defined$ Parent\n",
    json!({"Move": {"what": "This", "to": "Hand"}})
)]
fn a_spell_moves_itself_from_the_stack(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] last: Value,
) {
    let text = format!("Name:T\nTypes:{types}\n{lines}");
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(effects(&text).last(), Some(&last));
}

#[test]
fn krark_returns_the_spell_that_was_cast() {
    let text = "Name:T\nTypes:Creature Goblin\nT:Mode$ SpellCast | ValidCard$ Instant,Sorcery | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | Execute$ B | TriggerDescription$ x\nSVar:B:DB$ ChangeZone | Defined$ TriggeredSpellAbility | Origin$ Stack | Destination$ Hand\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    assert_eq!(
        effects(text).last(),
        Some(&json!({"Move": {"what": "Triggering", "to": "Hand"}}))
    );
}

#[rstest]
#[case::remembered("Defined$ Remembered", "defined:Stack-Remembered")]
#[case::replaced("Defined$ ReplacedCard", "defined:Stack-ReplacedCard")]
fn another_name_for_a_spell_stays_a_gap(#[case] defined: &str, #[case] gap: &str) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Stack | Destination$ Hand | {defined}\n"
    );
    assert_eq!(gaps(&text), [gap]);
}
