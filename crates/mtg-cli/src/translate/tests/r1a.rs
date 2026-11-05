//! Goal: the lasting-effect reader and the cast-permission words: an `Effect` object holds a permission or a prohibition for
//! the objects or players it remembers, mana may be spent as any type, and the continuous words that change land plays,
//! types and goad read; a word no model covers stays a named gap.
use rstest::rstest;

use super::{abilities, gaps};

const PLAY: &str = "SVar:STPlay:Mode$ Continuous | MayPlay$ True | Affected$ Card.IsRemembered+nonLand | AffectedZone$ Exile";

fn impulse(effect_keys: &str, play_keys: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Dig | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True | SubAbility$ Eff\nSVar:Eff:DB$ Effect | RememberObjects$ Remembered | StaticAbilities$ STPlay | {effect_keys}\n{PLAY} | {play_keys}\n"
    )
}

#[rstest]
#[case::any_type("MayPlayIgnoreType$ True", "\"spend_as\":\"AnyType\"")]
#[case::any_color("MayPlayIgnoreColor$ True", "\"spend_as\":\"AnyColor\"")]
#[case::free("MayPlayWithoutManaCost$ True", "\"free\":true")]
fn a_permission_to_cast_carries_how_mana_is_spent(#[case] words: &str, #[case] found: &str) {
    let script = impulse("ForgetOnMoved$ Exile", words);
    let written = serde_json::to_string(&abilities(&script)).unwrap();
    assert!(written.contains(found), "{written}");
    assert_eq!(gaps(&script), Vec::<String>::new());
}

#[rstest]
#[case::owner_of_a_target("TargetedOwner")]
#[case::the_player_who_triggered("TriggeredTarget")]
fn the_effect_owner_is_who_may_cast(#[case] owner: &str) {
    let script = format!(
        "Name:T\nTypes:Creature\nT:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Player | CombatDamage$ True | Execute$ Ex\nSVar:Ex:DB$ ChangeZone | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Permanent.nonLand | SubAbility$ Eff\nSVar:Eff:DB$ Effect | RememberObjects$ Targeted | StaticAbilities$ STPlay | ForgetOnMoved$ Exile | EffectOwner$ {owner} | Duration$ Permanent\n{PLAY}\n"
    );
    let written = serde_json::to_string(&abilities(&script)).unwrap();
    assert!(written.contains("\"who\":"), "{written}");
    assert!(!written.contains("\"who\":\"You\""), "{written}");
    assert_eq!(gaps(&script), Vec::<String>::new());
}

#[test]
fn an_effect_owner_outside_a_permission_stays_a_gap() {
    let script = "Name:T\nTypes:Sorcery\nA:SP$ Effect | ValidTgts$ Player | RememberObjects$ Targeted | StaticAbilities$ Cant | EffectOwner$ Opponent\nSVar:Cant:Mode$ CantAttack | ValidCard$ Creature.IsRemembered\n";
    assert_eq!(abilities(script), Vec::<serde_json::Value>::new());
    assert_eq!(gaps(script), vec!["key:Effect.EffectOwner".to_owned()]);
}

#[rstest]
#[case::spells(
    "Mode$ CantBeCast | ValidCard$ Card | Caster$ Player.IsRemembered",
    "\"Cast\":{}"
)]
#[case::creature_spells(
    "Mode$ CantBeCast | ValidCard$ Creature | Caster$ Player.IsRemembered",
    "\"Cast\":{\"filter\""
)]
#[case::abilities(
    "Mode$ CantBeActivated | ValidCard$ Card | ValidSA$ Activated.!ManaAbility | Activator$ Player.IsRemembered",
    "\"Activate\":[]"
)]
fn a_prohibition_on_the_remembered_player_lasts_the_turn(#[case] line: &str, #[case] found: &str) {
    let script = format!(
        "Name:T\nTypes:Instant\nA:SP$ Effect | ValidTgts$ Player | StaticAbilities$ S | RememberObjects$ Targeted\nSVar:S:{line}\n"
    );
    let written = serde_json::to_string(&abilities(&script)).unwrap();
    assert!(written.contains(found), "{written}");
    assert!(
        written.contains("\"duration\":\"UntilEndOfTurn\""),
        "{written}"
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
}

#[test]
fn statics_and_triggers_of_one_effect_that_remembers_read_together() {
    let script = "Name:T\nTypes:Sorcery\nA:SP$ Dig | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True | SubAbility$ Eff\nSVar:Eff:DB$ Effect | RememberObjects$ Remembered | StaticAbilities$ STPlay | Triggers$ Cast | ForgetOnMoved$ Exile\nSVar:STPlay:Mode$ Continuous | MayPlay$ True | Affected$ Card.IsRemembered | AffectedZone$ Exile\nSVar:Cast:Mode$ SpellCast | ValidCard$ Card.IsRemembered | ValidActivatingPlayer$ You | TriggerZones$ Command | Execute$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ 2\n";
    let written = serde_json::to_string(&abilities(script)).unwrap();
    assert!(
        written.contains("MayCast") && written.contains("DelayedTrigger"),
        "{written}"
    );
    assert_eq!(gaps(script), Vec::<String>::new());
}

#[rstest]
#[case::one("1", Some("\"AdditionalLandPlays\":{}"))]
#[case::two("2", Some("\"count\":2"))]
#[case::unlimited("Unlimited", None)]
fn extra_land_plays(#[case] count: &str, #[case] found: Option<&str>) {
    let script = format!(
        "Name:T\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ You | AdjustLandPlays$ {count} | Description$ x\n"
    );
    match found {
        Some(found) => {
            let written = serde_json::to_string(&abilities(&script)).unwrap();
            assert!(written.contains(found), "{written}");
        }
        None => assert_eq!(
            gaps(&script),
            vec!["key:Static.Continuous.AdjustLandPlays=Unlimited".to_owned()]
        ),
    }
}

#[rstest]
#[case::land_types(
    "Affected$ Land.nonBasic | AddType$ Island | RemoveLandTypes$ True",
    "\"SetLandTypes\":[\"Island\"]"
)]
#[case::no_land_types("Affected$ Land | RemoveLandTypes$ True", "\"SetLandTypes\":[]")]
#[case::card_types(
    "Affected$ Creature | AddType$ Enchantment | RemoveCardTypes$ True",
    "\"SetTypes\":[\"Enchantment\"]"
)]
#[case::goad("Affected$ Creature.OppCtrl | Goad$ True", "\"Goaded\"")]
fn continuous_words_that_change_types_and_goad(#[case] line: &str, #[case] found: &str) {
    let script =
        format!("Name:T\nTypes:Enchantment\nS:Mode$ Continuous | {line} | Description$ x\n");
    let written = serde_json::to_string(&abilities(&script)).unwrap();
    assert!(written.contains(found), "{written}");
    assert_eq!(gaps(&script), Vec::<String>::new());
}
