//! Goal: round 13 pack g13i reads cost modifiers that wait for a turn, a step or a count, statics that wait for the top
//! card of the library, copies that enter tapped or last a turn, tokens that are copies with a name, a type set, a trigger
//! or a lasting keyword, the permanents that become copies of a target, and a replacement that waits for the source's tapped
//! state; and leaves the neighbouring shapes a gap by name.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given type line, script lines and Oracle text, as compact JSON, and its gaps.
fn card(types: &str, lines: &str, oracle: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\nPT:2/2\n{lines}\nOracle:{oracle}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::reduced_by_a_printed_cost_on_your_turn(
    "Sorcery",
    "S:Mode$ ReduceCost | Condition$ PlayerTurn | ValidCard$ Card.Self | Amount$ 1 | Color$ 2 U U | Type$ Spell | EffectZone$ All | Description$ During your turn, CARDNAME costs {2}{U}{U} less to cast.\nA:SP$ Draw | NumCards$ 1",
    "During your turn, this spell costs {2}{U}{U} less to cast.\nDraw a card.",
    r#""condition":"YourTurn""#
)]
#[case::reduced_on_your_turn(
    "Instant",
    "S:Mode$ ReduceCost | ValidCard$ Card.Self | Type$ Spell | Amount$ 1 | PlayerTurn$ You | EffectZone$ All | Description$ This spell costs {1} less to cast during your turn.\nA:SP$ Draw | NumCards$ 1",
    "This spell costs {1} less to cast during your turn.\nDraw a card.",
    r#""condition":"YourTurn""#
)]
#[case::reduced_in_your_end_step(
    "Instant",
    "S:Mode$ ReduceCost | ValidCard$ Card.Self | Type$ Spell | Amount$ 2 | Phases$ End of Turn | PlayerTurn$ You | EffectZone$ All | Description$ This spell costs {2} less to cast during your end step.\nA:SP$ Draw | NumCards$ 1",
    "This spell costs {2} less to cast during your end step.\nDraw a card.",
    r#"{"All":["YourTurn",{"Step":"End"}]}"#
)]
#[case::the_first_instant_or_sorcery_each_turn(
    "Creature",
    "S:Mode$ ReduceCost | OnlyFirstSpell$ True | Type$ Spell | ValidSpell$ Spell.Instant,Spell.Sorcery | Activator$ You | Amount$ 3 | Description$ The first instant or sorcery spell you cast each turn costs {3} less to cast.",
    "The first instant or sorcery spell you cast each turn costs {3} less to cast.",
    r#""op":"Eq","right":0"#
)]
fn spells_wait_for_a_turn_or_a_step(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] part: &str,
) {
    let (json, gaps) = card(types, lines, oracle);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::printed_cost_with_a_count(
    "Instant",
    "S:Mode$ ReduceCost | ValidCard$ Card.Self | Type$ Spell | Amount$ 1 | Color$ 2 U | Phases$ Upkeep,Draw | Description$ x\nA:SP$ Draw | NumCards$ 1",
    "This spell costs {2}{U} less to cast during your upkeep or draw step.\nDraw a card.",
    r#""mana":"{2}{U}""#
)]
fn a_printed_cost_of_one_change_is_the_whole_amount(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] part: &str,
) {
    let (json, gaps) = card(types, lines, oracle);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::a_player_turn_of_someone_else(
    "Instant",
    "S:Mode$ ReduceCost | ValidCard$ Card.Self | Type$ Spell | Amount$ 1 | PlayerTurn$ Opponent | EffectZone$ All | Description$ x\nA:SP$ Draw | NumCards$ 1",
    "This spell costs {1} less to cast during an opponent's turn.\nDraw a card.",
    "key:Static.CostModifier.PlayerTurn=Opponent"
)]
#[case::an_equip_cost_that_waits_for_its_target(
    "Creature",
    "S:Mode$ ReduceCost | ValidTarget$ Card.Self | ValidSpell$ Activated.Equip | Activator$ You | Amount$ 2 | Description$ Equip abilities you activate that target this creature cost {2} less to activate.",
    "Equip abilities you activate that target this creature cost {2} less to activate.",
    "key:Static.CostModifier.ValidTarget-of-an-ability"
)]
#[case::the_first_kicked_spell(
    "Creature",
    "S:Mode$ ReduceCost | OnlyFirstSpell$ True | Type$ Spell | ValidSpell$ Spell.Kicked | Activator$ You | Amount$ 1 | Description$ The first kicked spell you cast each turn costs {1} less to cast.",
    "The first kicked spell you cast each turn costs {1} less to cast.",
    "key:Static.CostModifier.ValidSpell=Spell.Kicked"
)]
fn other_cost_shapes_stay_gaps(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] gap: &str,
) {
    let (_, gaps) = card(types, lines, oracle);
    assert!(gaps.iter().any(|found| found == gap), "{gaps:?}");
}

#[rstest]
#[case::top_card_is_black(
    "Creature",
    "S:Mode$ Continuous | Affected$ Creature.Elf+Other+YouCtrl,Card.Self | AddPower$ 2 | AddToughness$ 1 | AddKeyword$ Flying | TopCardOfLibraryIs$ Card.Black | Description$ As long as the top card of your library is black, CARDNAME and other Elf creatures you control get +2/+1 and have flying.",
    "As long as the top card of your library is black, Subject and other Elf creatures you control get +2/+1 and have flying.",
    r#""TopOfLibrary",{"Color":"Black"}"#
)]
#[case::top_card_is_a_creature(
    "Creature",
    "S:Mode$ Continuous | AffectedDefined$ Self | AddPower$ 3 | AddToughness$ 3 | TopCardOfLibraryIs$ Creature | Description$ As long as the top card of your library is a creature card, CARDNAME gets +3/+3.",
    "As long as the top card of your library is a creature card, Subject gets +3/+3.",
    r#""TopOfLibrary",{"Type":"Creature"}"#
)]
#[case::every_nonbasic_land_type(
    "Land",
    "S:Mode$ Continuous | Affected$ Card.Self | CharacteristicDefining$ True | AddType$ AllNonBasicLandType | Description$ CARDNAME is every nonbasic land type.",
    "Subject is every nonbasic land type.",
    r#"{"AddSubtype":"Urza's"}"#
)]
#[case::an_aura_that_keeps_a_creature_tapped(
    "Enchantment Aura",
    "K:Enchant:Creature\nS:Mode$ Continuous | Affected$ Creature.nonLegendary+EnchantedBy | AddHiddenKeyword$ This card doesn't untap during your next untap step. | Description$ As long as enchanted creature isn't legendary, it doesn't untap during its controller's untap step.",
    "Enchant creature\\nAs long as enchanted creature isn't legendary, it doesn't untap during its controller's untap step.",
    r#""mods":["DoesntUntap"]"#
)]
fn statics_wait_for_the_library_or_name_types(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] part: &str,
) {
    let (json, gaps) = card(types, lines, oracle);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[test]
fn nonbasic_land_types_leave_out_the_basic_ones() {
    let (json, _) = card(
        "Land",
        "S:Mode$ Continuous | Affected$ Card.Self | CharacteristicDefining$ True | AddType$ AllNonBasicLandType | Description$ x",
        "Subject is every nonbasic land type.",
    );
    for basic in ["Plains", "Island", "Swamp", "Mountain", "Forest"] {
        assert!(
            !json.contains(&format!(r#""AddSubtype":"{basic}""#)),
            "{json}"
        );
    }
}

#[rstest]
#[case::enters_tapped_as_a_copy(
    "Land",
    "K:ETBReplacement:Copy:DBCopy:Optional\nSVar:DBCopy:DB$ Clone | Choices$ Land.Other | IntoPlayTapped$ True | SpellDescription$ You may have CARDNAME enter tapped as a copy of any land on the battlefield.",
    "You may have Subject enter tapped as a copy of any land on the battlefield.",
    r#""tapped":true"#
)]
#[case::a_copy_until_end_of_turn(
    "Artifact",
    "K:ETBReplacement:Copy:DBCopy:Optional\nSVar:DBCopy:DB$ Clone | Choices$ Creature | AddKeywords$ Haste | Duration$ UntilEndOfTurn | SpellDescription$ As CARDNAME enters, you may have it become a copy of any creature on the battlefield until end of turn, except it has haste.",
    "As Subject enters, you may have it become a copy of any creature on the battlefield until end of turn, except it has haste.",
    r#""until_end_of_turn":true"#
)]
#[case::an_artifact_that_is_no_creature(
    "Artifact Creature",
    "K:ETBReplacement:Copy:DBCopy:Optional\nSVar:DBCopy:DB$ Clone | Choices$ Creature | AddTypes$ Artifact | RemoveCardTypes$ True | SpellDescription$ You may have CARDNAME enter as a copy of any creature on the battlefield, except it's an artifact.",
    "You may have Subject enter as a copy of any creature on the battlefield, except it's an artifact.",
    r#"{"SetTypes":["Artifact"]}"#
)]
#[case::a_vehicle_that_loses_its_creature_types(
    "Artifact Vehicle",
    "K:ETBReplacement:Copy:DBCopy:Optional\nSVar:DBCopy:DB$ Clone | Choices$ Creature.OppCtrl | AddTypes$ Vehicle & Artifact | AddKeywords$ Crew:3 | RemoveCardTypes$ True | RemoveSubTypes$ True | SpellDescription$ You may have this Vehicle enter as a copy of a creature an opponent controls, except it's a Vehicle artifact with crew 3 and it loses all other card types.",
    "You may have this Vehicle enter as a copy of a creature an opponent controls, except it's a Vehicle artifact with crew 3 and it loses all other card types.",
    r#"{"SetTypes":["Artifact"]}"#
)]
fn copies_as_a_permanent_enters(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] part: &str,
) {
    let (json, gaps) = card(types, lines, oracle);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::a_creature_stays_a_creature(
    "Creature",
    "K:ETBReplacement:Copy:DBCopy:Optional\nSVar:DBCopy:DB$ Clone | Choices$ Creature | AddTypes$ Creature | RemoveCardTypes$ True | SpellDescription$ x",
    "You may have Subject enter as a copy of any creature on the battlefield.",
    "key:Clone.RemoveCardTypes"
)]
#[case::subtypes_lost_without_the_types(
    "Creature",
    "K:ETBReplacement:Copy:DBCopy:Optional\nSVar:DBCopy:DB$ Clone | Choices$ Creature | AddTypes$ Artifact | RemoveSubTypes$ True | SpellDescription$ x",
    "You may have Subject enter as a copy of any creature on the battlefield.",
    "key:Clone.RemoveSubTypes"
)]
#[case::an_into_play_tapped_that_becomes_a_copy_later(
    "Creature",
    "A:AB$ Clone | Cost$ T | ValidTgts$ Creature | IntoPlayTapped$ True | SpellDescription$ x",
    "{T}: Subject becomes a copy of target creature.",
    "key:Clone.IntoPlayTapped"
)]
#[case::another_target_without_a_second_target_rule(
    "Planeswalker",
    "A:AB$ Pump | Cost$ T | ValidTgts$ Artifact | SubAbility$ C\nSVar:C:DB$ Clone | CloneTarget$ ParentTarget | ValidTgts$ Creature | TgtPrompt$ Select another target creature | Duration$ UntilEndOfTurn",
    "{T}: Target artifact becomes a copy of another target creature until end of turn.",
    "key:Clone.another-target"
)]
fn other_copy_shapes_stay_gaps(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] gap: &str,
) {
    let (_, gaps) = card(types, lines, oracle);
    assert!(gaps.iter().any(|found| found == gap), "{gaps:?}");
}

#[rstest]
#[case::the_token_is_sacrificed_by_its_own_trigger(
    "Sorcery",
    "A:SP$ CopyPermanent | ValidTgts$ Creature.YouCtrl | AddKeywords$ Haste | AtEOTTrig$ Sacrifice | SpellDescription$ Create a token that's a copy of target creature you control, except it has haste and \"At the beginning of the end step, sacrifice this token.\"",
    "Create a token that's a copy of target creature you control, except it has haste and \"At the beginning of the end step, sacrifice this token.\"",
    r#"{"GrantAbility":{"Triggered":{"effects":[{"Sacrifice":{"filter":["This"]}}],"when":{"StepBegins":{"step":"End"}}}}}"#
)]
#[case::the_token_is_exiled_by_its_own_trigger(
    "Sorcery",
    "A:SP$ CopyPermanent | ValidTgts$ Creature | AddKeywords$ Haste | AtEOTTrig$ Exile | SpellDescription$ Create a token that's a copy of target creature, except it has haste and \"At the beginning of the end step, exile this permanent.\"",
    "Create a token that's a copy of target creature, except it has haste and \"At the beginning of the end step, exile this permanent.\"",
    r#"{"Exile":{"what":"This"}}"#
)]
#[case::a_named_copy(
    "Sorcery",
    "A:SP$ CopyPermanent | ValidTgts$ Creature | NewName$ Prisoner Zero | SetCreatureTypes$ Wizard | AddTypes$ Legendary | SpellDescription$ Create a token that's a copy of target creature, except it's a legendary Wizard named Prisoner Zero.",
    "Create a token that's a copy of target creature, except it's a legendary Wizard named Prisoner Zero.",
    r#"{"SetName":"Prisoner Zero"}"#
)]
#[case::a_copy_that_loses_its_other_types(
    "Enchantment",
    "T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | Execute$ C | TriggerZones$ Battlefield | TriggerDescription$ x\nSVar:C:DB$ CopyPermanent | ValidTgts$ Creature.YouCtrl | AddTypes$ Enchantment | RemoveCardTypes$ True",
    "At the beginning of your upkeep, create a token that's a copy of target creature you control, except it's an enchantment and loses all other card types.",
    r#"{"SetTypes":["Enchantment"]}"#
)]
#[case::a_keyword_until_your_next_turn(
    "Enchantment",
    "T:Mode$ Phase | Phase$ BeginCombat | ValidPlayer$ You | Execute$ C | TriggerZones$ Battlefield | TriggerDescription$ x\nSVar:C:DB$ CopyPermanent | ValidTgts$ Creature.YouCtrl | PumpKeywords$ Haste | PumpDuration$ UntilYourNextTurn",
    "At the beginning of combat on your turn, create a token that's a copy of target creature you control. It gains haste until your next turn.",
    r#""duration":"UntilYourNextTurn""#
)]
#[case::each_opponent_makes_the_copy(
    "Creature",
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self+!token | Execute$ C | TriggerDescription$ When CARDNAME enters, if it's not a token, each opponent creates a token that's a copy of it.\nSVar:C:DB$ CopyPermanent | Defined$ TriggeredCardLKICopy | Controller$ Opponent",
    "When Subject enters, if it's not a token, each opponent creates a token that's a copy of it.",
    r#""who":"Opponents""#
)]
fn tokens_that_are_copies(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] part: &str,
) {
    let (json, gaps) = card(types, lines, oracle);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::a_trigger_that_is_not_known(
    "Sorcery",
    "A:SP$ CopyPermanent | ValidTgts$ Creature | AtEOTTrig$ Hand | SpellDescription$ x",
    "Create a token that's a copy of target creature.",
    "key:CopyPermanent.AtEOTTrig=Hand"
)]
#[case::a_colour_added_to_a_copy(
    "Sorcery",
    "A:SP$ CopyPermanent | ValidTgts$ Creature | AddColors$ Black | SpellDescription$ x",
    "Create a token that's a copy of target creature, except it's black in addition to its other colors.",
    "key:CopyPermanent.AddColors"
)]
#[case::counters_a_token_enters_with(
    "Sorcery",
    "A:SP$ CopyPermanent | ValidTgts$ Creature | WithCountersType$ P1P1 | SpellDescription$ x",
    "Create a token that's a copy of target creature, except it enters with an additional +1/+1 counter.",
    "key:CopyPermanent.WithCountersType"
)]
fn other_token_copies_stay_gaps(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] gap: &str,
) {
    let (_, gaps) = card(types, lines, oracle);
    assert!(gaps.iter().any(|found| found == gap), "{gaps:?}");
}

#[rstest]
#[case::a_target_becomes_a_copy_of_another(
    "Sorcery",
    "A:SP$ Pump | ValidTgts$ Artifact,Creature | SubAbility$ C\nSVar:C:DB$ Clone | CloneTarget$ ParentTarget | ValidTgts$ Artifact,Creature | TargetUnique$ True | TgtPrompt$ Select another target artifact or creature to copy",
    "Target artifact or creature becomes a copy of another target artifact or creature.",
    r#""BecomeCopy":{"duration":"Forever","of":{"Target":1},"what":{"Target":0}}"#
)]
#[case::each_other_creature_becomes_a_copy(
    "Sorcery",
    "A:SP$ Clone | Defined$ Targeted | CloneTarget$ Valid Creature.NotDefinedTargeted | Duration$ UntilEndOfTurn | ValidTgts$ Creature.nonLegendary",
    "Each other creature becomes a copy of target nonlegendary creature until end of turn.",
    r#""what":{"Each":[{"Type":"Creature"},{"Not":{"Is":{"Target":0}}}]}"#
)]
#[case::each_permanent_you_control_becomes_a_copy(
    "Sorcery",
    "A:SP$ Clone | Defined$ Targeted | CloneTarget$ Valid Permanent.nonLand+YouCtrl | ValidTgts$ Permanent.nonAura",
    "Each nonland permanent you control becomes a copy of target non-Aura permanent.",
    r#""what":{"Each":[{"Not":{"Type":"Land"}},{"ControlledBy":"You"}]}"#
)]
fn permanents_become_copies_of_a_target(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] part: &str,
) {
    let (json, gaps) = card(types, lines, oracle);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[test]
fn a_source_that_is_tapped_makes_other_permanents_enter_tapped() {
    let (json, gaps) = card(
        "Creature",
        "R:Event$ Moved | ValidCard$ Permanent | Destination$ Battlefield | IsPresent$ Card.Self+tapped | ReplaceWith$ ETBTapped | ReplacementResult$ Updated | ActiveZones$ Battlefield | Description$ As long as CARDNAME is tapped, other permanents enter tapped.\nSVar:ETBTapped:DB$ Tap | ETB$ True | Defined$ ReplacedCard",
        "As long as Subject is tapped, other permanents enter tapped.",
    );
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(
        json.contains(
            r#""by":"Tapped","condition":{"Matches":{"selector":["Tapped"],"subject":"This"}}"#
        ),
        "{json}"
    );
}

#[test]
fn another_state_of_the_source_stays_a_gap() {
    let (_, gaps) = card(
        "Creature",
        "R:Event$ Moved | ValidCard$ Permanent | Destination$ Battlefield | IsPresent$ Card.Self+attacking | ReplaceWith$ ETBTapped | ReplacementResult$ Updated | ActiveZones$ Battlefield | Description$ x\nSVar:ETBTapped:DB$ Tap | ETB$ True | Defined$ ReplacedCard",
        "As long as Subject is attacking, other permanents enter tapped.",
    );
    assert!(
        gaps.iter()
            .any(|found| found == "key:Replacement.Moved.IsPresent=Card.Self+attacking"),
        "{gaps:?}"
    );
}
