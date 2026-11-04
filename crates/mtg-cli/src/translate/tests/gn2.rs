//! Goal: `Animate` lines that give or take away abilities and `Effect` objects of rules map to the nodes that carry
//! them, and every shape the nodes cannot carry stays a gap.
use rstest::rstest;
use serde_json::Value;

use super::{abilities, gaps};

/// The abilities of a script as one string, so a case names the fragments its result must hold.
fn shown(script: &str) -> String {
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    Value::Array(found).to_string()
}

#[rstest]
#[case::granted_trigger(
    "Name:T\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ Creature | NumAtt$ +2 | SubAbility$ DBA\nSVar:DBA:DB$ Animate | Triggers$ Dies | Defined$ ParentTarget\nSVar:Dies:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | Execute$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ 1\n",
    &["GrantAbility", "Triggered", "GainLife"]
)]
#[case::loses_all_abilities(
    "Name:T\nTypes:Sorcery\nA:SP$ Animate | ValidTgts$ Creature | RemoveAllAbilities$ True | Duration$ Permanent\n",
    &["LoseAbilities", "Forever"]
)]
#[case::flash_for_creatures(
    "Name:T\nTypes:Enchantment\nS:Mode$ CastWithFlash | ValidCard$ Creature | ValidSA$ Spell | Caster$ You | Description$ x\n",
    &["MayCast", "\"flash\":true", "Creature"]
)]
#[case::flash_for_any_player(
    "Name:T\nTypes:Enchantment\nS:Mode$ CastWithFlash | ValidCard$ Creature,Enchantment | ValidSA$ Spell | Caster$ Player | Description$ x\n",
    &["MayCast", "EachPlayer", "Enchantment"]
)]
#[case::opponents_cast_nothing_on_your_turn(
    "Name:T\nTypes:Enchantment\nS:Mode$ CantBeCast | ValidCard$ Card | Condition$ PlayerTurn | Caster$ Opponent | Description$ x\n",
    &["CantCastSpells", "Opponents", "YourTurn"]
)]
#[case::an_effect_stops_all_casting(
    "Name:T\nTypes:Sorcery\nA:SP$ Effect | StaticAbilities$ S\nSVar:S:Mode$ CantBeCast | ValidCard$ Card | Caster$ Player | Description$ x\n",
    &["CantCastSpells", "EachPlayer", "UntilEndOfTurn"]
)]
fn animate_and_casting_statics_map(#[case] script: &str, #[case] holds: &[&str]) {
    let text = shown(script);
    for part in holds {
        assert!(text.contains(part), "{part} missing in {text}");
    }
}

#[rstest]
#[case::only_some_cards("Mode$ CantBeCast | ValidCard$ Card.nonCreature | Caster$ You")]
#[case::a_limit_each_turn("Mode$ CantBeCast | ValidCard$ Card | Caster$ Player | NumLimitEachTurn$ 1")]
#[case::loyalty_abilities("Mode$ CastWithFlash | ValidCard$ Card.Self | ValidSA$ Activated.Loyalty | Caster$ You")]
#[case::the_spell_itself("Mode$ CastWithFlash | ValidCard$ Card.Self | ValidSA$ Spell | Caster$ You")]
fn casting_statics_beyond_the_nodes_stay_gaps(#[case] line: &str) {
    let script = format!("Name:T\nTypes:Sorcery\nA:SP$ Effect | StaticAbilities$ S\nSVar:S:{line}\n");
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert_ne!(gaps(&script), Vec::<String>::new());
}
