//! Goal: `ControlPlayer`, `ControlSpell`, `ChangeText` (one kind of word, or a `GenericChoice` between a color word and a basic land type) and the search-control static map to their engine nodes, and the forms they cannot say (more than one extra turn after the controlled one, a spell control that is not a gain, a restart of the game) stay named gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn change(target: Value, from: &str, to: Value, duration: &str) -> Value {
    json!({"Spell": {"targets": [{"what": target}], "effects": [
        {"ChooseValue": {"kind": from, "as": "from"}},
        {"ChooseValue": {"kind": to, "as": "to"}},
        {"Continuous": {"affected": {"Target": 0},
            "mods": [{"ChangeTextChosen": {"from": "from", "to": "to"}}], "duration": duration}}
    ]}})
}

fn spell_or_permanent() -> Value {
    json!({"Object": [{"AnyOf": ["Spell", {"In": "Battlefield"}]}]})
}

#[rstest]
#[case::worst_fears(
    "Name:T\nTypes:Sorcery\nA:SP$ ControlPlayer | ValidTgts$ Player\n",
    json!({"Spell": {"targets": [{"what": "Player"}], "effects": [{"ControlPlayer": {"who": {"Target": 0}}}]}})
)]
#[case::emrakul_the_promised_end(
    "Name:T\nTypes:Sorcery\nA:SP$ ControlPlayer | ValidTgts$ Opponent | SubAbility$ DBDelTrig\nSVar:DBDelTrig:DB$ DelayedTrigger | Mode$ Phase | DelayedTriggerDefinedPlayer$ Targeted | RememberObjects$ Targeted | Phase$ Cleanup | Execute$ DBAddTurn | Static$ True\nSVar:DBAddTurn:DB$ AddTurn | Defined$ DelayTriggerRemembered | NumTurns$ 1\n",
    json!({"Spell": {"targets": [{"what": "Opponent"}], "effects": [{"ControlPlayer": {"who": {"Target": 0}, "extra_turn": true}}]}})
)]
#[case::aethersnatch(
    "Name:T\nTypes:Instant\nA:SP$ ControlSpell | ValidTgts$ Card | TargetType$ Spell | Mode$ Gain | SubAbility$ DBChooseTargets\nSVar:DBChooseTargets:DB$ ChangeTargets | Defined$ Targeted | Optional$ True\n",
    json!({"Spell": {"targets": [{"what": {"Object": ["Spell"]}}], "effects": [
        {"Continuous": {"affected": {"Target": 0}, "mods": [{"SetController": "You"}], "duration": "Forever"}},
        {"ChangeTargets": {"what": {"Target": 0}, "to": "Choose"}}
    ]}})
)]
#[case::sleight_of_mind(
    "Name:T\nTypes:Instant\nA:SP$ ChangeText | ValidTgts$ Card | TgtZone$ Stack,Battlefield | TgtPrompt$ Choose target spell or permanent | Duration$ Permanent | ChangeColorWord$ Choose Choose\n",
    change(spell_or_permanent(), "Color", json!({"LikeChosen": "from"}), "Forever")
)]
#[case::magical_hack(
    "Name:T\nTypes:Instant\nA:SP$ ChangeText | ValidTgts$ Card | TgtZone$ Stack,Battlefield | ChangeTypeWord$ ChooseBasicLandType ChooseBasicLandType | Duration$ Permanent\n",
    change(spell_or_permanent(), "BasicLandType", json!({"LikeChosen": "from"}), "Forever")
)]
#[case::artificial_evolution(
    "Name:T\nTypes:Instant\nA:SP$ ChangeText | ValidTgts$ Card | TgtZone$ Stack,Battlefield | ChangeTypeWord$ ChooseCreatureType ChooseCreatureType | ForbiddenNewTypes$ Wall | Duration$ Permanent\n",
    change(spell_or_permanent(), "CreatureType", json!({"CreatureTypeExcept": ["Wall"]}), "Forever")
)]
#[case::mind_bend(
    "Name:T\nTypes:Instant\nA:SP$ GenericChoice | ValidTgts$ Permanent | Choices$ ChangeColor,ChangeType | Defined$ You\nSVar:ChangeColor:DB$ ChangeText | Defined$ ParentTarget | ChangeColorWord$ Choose Choose | Duration$ Permanent\nSVar:ChangeType:DB$ ChangeText | Defined$ ParentTarget | ChangeTypeWord$ ChooseBasicLandType ChooseBasicLandType | Duration$ Permanent\n",
    change(json!({"Object": []}), "ColorOrBasicLandType", json!({"LikeChosen": "from"}), "Forever")
)]
fn a_line_maps_to_its_nodes(#[case] script: &str, #[case] want: Value) {
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    assert_eq!(normal(found[0].clone()), normal(want), "{}", found[0]);
}

#[test]
fn crystal_spray_draws_after_the_change_until_end_of_turn() {
    let script = "Name:T\nTypes:Instant\nA:SP$ GenericChoice | ValidTgts$ Card | TgtZone$ Stack,Battlefield | Choices$ ChangeColor,ChangeType | Defined$ You\nSVar:ChangeColor:DB$ ChangeText | Defined$ ParentTarget | ChangeColorWord$ Choose Choose | SubAbility$ DBDraw\nSVar:ChangeType:DB$ ChangeText | Defined$ ParentTarget | ChangeTypeWord$ ChooseBasicLandType ChooseBasicLandType | SubAbility$ DBDraw\nSVar:DBDraw:DB$ Draw\n";
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    let effects = found[0]["Spell"]["effects"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(effects.len(), 4, "{}", found[0]);
    assert_eq!(
        effects[2]["Continuous"]["duration"],
        json!("UntilEndOfTurn")
    );
    assert_eq!(effects[3], json!({"Draw": {}}));
}

#[test]
fn opposition_agent_controls_searching_opponents() {
    let script = "Name:T\nTypes:Creature Human Rogue\nPT:3/2\nS:Mode$ Continuous | Affected$ Opponent | ControlOpponentsSearchingLibrary$ You | Description$ You control your opponents while they're searching their libraries.\n";
    let found = abilities(script);
    assert!(
        found.contains(&json!({"Rule": {"rule": {"ControlSearches": "Opponents"}}})),
        "{found:?} {:?}",
        gaps(script)
    );
}

#[rstest]
#[case::two_extra_turns_after(
    "Name:T\nTypes:Sorcery\nA:SP$ ControlPlayer | ValidTgts$ Opponent | SubAbility$ DBDelTrig\nSVar:DBDelTrig:DB$ DelayedTrigger | Mode$ Phase | DelayedTriggerDefinedPlayer$ Targeted | RememberObjects$ Targeted | Phase$ Cleanup | Execute$ DBAddTurn | Static$ True\nSVar:DBAddTurn:DB$ AddTurn | Defined$ DelayTriggerRemembered | NumTurns$ 2\n",
    "key:ControlPlayer.extra-turn"
)]
#[case::an_extra_turn_at_the_end_step(
    "Name:T\nTypes:Sorcery\nA:SP$ ControlPlayer | ValidTgts$ Opponent | SubAbility$ DBDelTrig\nSVar:DBDelTrig:DB$ DelayedTrigger | Mode$ Phase | DelayedTriggerDefinedPlayer$ Targeted | RememberObjects$ Targeted | Phase$ End of Turn | Execute$ DBAddTurn | Static$ True\nSVar:DBAddTurn:DB$ AddTurn | Defined$ DelayTriggerRemembered | NumTurns$ 1\n",
    "key:ControlPlayer.extra-turn"
)]
#[case::a_spell_lost(
    "Name:T\nTypes:Instant\nA:SP$ ControlSpell | ValidTgts$ Card | TargetType$ Spell | Mode$ Lose\n",
    "key:ControlSpell.Mode"
)]
#[case::two_new_types_left_out(
    "Name:T\nTypes:Instant\nA:SP$ ChangeText | ValidTgts$ Card | ChangeTypeWord$ ChooseCreatureType ChooseCreatureType | ForbiddenNewTypes$ Wall,Elf | Duration$ Permanent\n",
    "key:ChangeText.ForbiddenNewTypes"
)]
#[case::a_named_word(
    "Name:T\nTypes:Instant\nA:SP$ ChangeText | ValidTgts$ Card | ChangeColorWord$ Black Choose | Duration$ Permanent\n",
    "key:ChangeText.words"
)]
#[case::a_restart("Name:T\nTypes:Sorcery\nA:SP$ RestartGame\n", "RestartGame")]
fn a_form_beyond_the_nodes_stays_a_gap(#[case] script: &str, #[case] gap: &str) {
    assert_eq!(abilities(script), Vec::<Value>::new(), "{script}");
    assert!(
        gaps(script).iter().any(|found| found.contains(gap)),
        "{:?}",
        gaps(script)
    );
}
