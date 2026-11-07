//! Goal: round 13 pack e reads the small forms of the tail: an empty spell line, who untaps, a cost worded per ability,
//! each player's own draw choice, an optional investigate and the animate keys that change types, combat and leaving.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given header and lines as compact JSON, and its gaps.
fn card(types: &str, lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[test]
fn a_spell_line_that_does_nothing_leaves_the_keywords_alone() {
    let (json, found) = card("Sorcery", "A:SP$ Pump | StackDescription$ None\nK:Cascade");
    assert!(found.is_empty(), "{found:?}");
    assert!(!json.contains("Spell"), "{json}");
    assert!(json.contains("Cascade"), "{json}");
}

#[test]
fn a_spell_line_with_a_target_is_still_a_spell() {
    let (json, found) = card(
        "Instant",
        "A:SP$ Pump | ValidTgts$ Creature | NumAtt$ +1 | NumDef$ +1",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains("Spell"), "{json}");
}

#[test]
fn the_player_who_untaps_changes_nothing() {
    let (json, found) = card(
        "Sorcery",
        "A:SP$ UntapAll | ControllerUntaps$ True | ValidTgts$ Player | ValidCards$ Land.Basic",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""ControlledBy":{"Target":0}"#), "{json}");
}

#[test]
fn a_card_that_words_two_sacrifices_differently_reads_each_by_its_own_mana() {
    let text = "Name:Subject\nTypes:Creature Imp\nA:AB$ DealDamage | Cost$ 1 R Sac<1/Creature.Other/another creature> | ValidTgts$ Player | NumDmg$ 2\nA:AB$ ChangeZone | Cost$ 1 B Sac<1/Creature> | Origin$ Graveyard | Destination$ Hand | Defined$ Self | ActivationZone$ Graveyard\nOracle:{1}{R}, Sacrifice another creature: Subject deals 2 damage to target player.\\n{1}{B}, Sacrifice a creature: Return Subject from your graveyard to your hand.\n";
    let found = gaps(text);
    assert!(found.is_empty(), "{found:?}");
}

#[rstest]
#[case::each_other_player("Player.Other", "Opponents")]
#[case::each_player("Player", "EachPlayer")]
fn each_player_chooses_their_own_up_to_number_of_cards(#[case] who: &str, #[case] over: &str) {
    let (json, found) = card(
        "Sorcery",
        &format!("A:SP$ Draw | Defined$ {who} | NumCards$ 3 | Upto$ True"),
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(&format!(r#""over":"{over}""#)), "{json}");
    assert!(json.contains(r#""Number":[0,3]"#), "{json}");
}

#[test]
fn an_optional_investigate_is_a_may() {
    let (json, found) = card(
        "Creature Detective",
        "T:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | Execute$ TrigInvestigate | TriggerDescription$ When this dies, you may investigate.\nSVar:TrigInvestigate:DB$ Investigate | Optional$ True",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""May""#), "{json}");
}

#[rstest]
#[case::a_card_type("Artifact", r#""RemoveType":"Artifact""#)]
#[case::the_creature_type_llama_a_land_becomes("Llama", "Llama")]
fn an_animate_line_removes_a_card_type_or_adds_a_type_no_card_prints(
    #[case] word: &str,
    #[case] expected: &str,
) {
    let line = if word == "Llama" {
        "A:AB$ Animate | Cost$ 2 G W | Defined$ Self | Power$ 3 | Toughness$ 3 | Types$ Creature,Llama | RemoveCreatureTypes$ True".to_owned()
    } else {
        format!(
            "A:AB$ Animate | Cost$ U | ValidTgts$ Creature | RemoveTypes$ {word} | Colors$ Blue | OverwriteColors$ True"
        )
    };
    let (json, found) = card("Land", &line);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn an_animate_line_that_removes_a_subtype_is_still_a_gap() {
    let (_, found) = card(
        "Artifact Equipment",
        "A:AB$ Animate | Cost$ 0 | Defined$ Self | Types$ Creature | RemoveTypes$ Equipment",
    );
    assert_eq!(found, vec!["key:Animate.RemoveTypes".to_owned()]);
}

#[test]
fn an_animated_land_must_be_blocked_if_able() {
    let (json, found) = card(
        "Sorcery",
        "A:SP$ Animate | ValidTgts$ Land.YouCtrl | Power$ 4 | Toughness$ 4 | Types$ Creature | HiddenKeywords$ CARDNAME must be blocked if able.",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""MustBeBlocked":"AtLeastOne""#), "{json}");
}

#[test]
fn a_returned_creature_is_exiled_if_it_would_leave() {
    let (json, found) = card(
        "Artifact",
        "A:AB$ ChangeZone | Cost$ T | ValidTgts$ Creature.YouOwn | Origin$ Graveyard | Destination$ Battlefield | RememberChanged$ True | SubAbility$ DBUnearthed\nSVar:DBUnearthed:DB$ Animate | Defined$ Remembered | Keywords$ Haste | LeaveBattlefield$ Exile | sVars$ MustAttack | Duration$ Permanent | AtEOT$ Exile | SubAbility$ DBCleanup\nSVar:MustAttack:SVar:MustAttack:True\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""Is":{"Bound":"moved"}"#), "{json}");
    assert!(json.contains(r#""MoveTo":"Exile""#), "{json}");
}

#[test]
fn a_sub_ability_variable_that_is_not_a_hint_is_a_gap() {
    let (_, found) = card(
        "Artifact",
        "A:AB$ Animate | Cost$ 1 | Defined$ Self | Types$ Creature | Power$ 2 | Toughness$ 2 | sVars$ Count\nSVar:Count:Count$Valid Creature",
    );
    assert_eq!(found, vec!["key:Animate.sVars".to_owned()]);
}

#[rstest]
#[case::hexproof_from_the_color(
    "ChooseColor",
    "Hexproof:ChosenColor",
    "ChosenColor",
    r#""HexproofFrom":[{"ChosenAs":"chosen"}]"#
)]
#[case::landwalk_of_the_land_type(
    "ChooseType | Type$ Land",
    "Landwalk:ChosenType",
    "ChosenType",
    r#""Landwalk":[{"ChosenAs":"chosen"}]"#
)]
fn a_keyword_of_the_choice_the_ability_made_is_read_by_name(
    #[case] choose: &str,
    #[case] keyword: &str,
    #[case] defined: &str,
    #[case] expected: &str,
) {
    let (json, found) = card(
        "Creature Lizard",
        &format!(
            "A:AB$ {choose} | Cost$ 1 | Defined$ You | SubAbility$ DBPump\nSVar:DBPump:DB$ Pump | Defined$ Self | KW$ {keyword} | DefinedKW$ {defined}"
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn a_defined_keyword_with_nothing_chosen_is_a_gap() {
    let (_, found) = card(
        "Creature Lizard",
        "A:AB$ Pump | Cost$ 1 | Defined$ Self | KW$ Hexproof:ChosenColor | DefinedKW$ ChosenColor",
    );
    assert_eq!(found, vec!["valid-base:ChosenColor".to_owned()]);
}

#[test]
fn radiance_reaches_the_target_and_the_creatures_sharing_a_color_with_it() {
    let (json, found) = card(
        "Instant",
        "A:SP$ Pump | ValidTgts$ Creature | Radiance$ True | NumAtt$ +1 | NumDef$ +1",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(r#"{"Is":{"Target":0}},{"SharesColor":{"Target":0}}"#),
        "{json}"
    );
}

#[test]
fn a_pump_valid_for_the_card_itself_is_the_default_subject() {
    let (json, found) = card(
        "Creature Horse",
        "A:AB$ Pump | Cost$ 1 R | ValidCard$ Card.Self | NumAtt$ +1",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""affected":"This""#), "{json}");
}

#[test]
fn energy_paid_after_a_chosen_number_is_paid_that_many_times() {
    let (json, found) = card(
        "Sorcery",
        "A:SP$ PutCounter | Defined$ You | CounterType$ ENERGY | SubAbility$ DBChoose\nSVar:DBChoose:DB$ ChooseNumber | Max$ Count$YourCountersEnergy | SubAbility$ DBDraw\nSVar:DBDraw:DB$ Draw | ConditionCheckSVar$ X | UnlessCost$ Mandatory PayEnergy<X> | UnlessPayer$ You | UnlessSwitched$ True | NumCards$ X\nSVar:X:Count$ChosenNumber",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""times":{"ChosenAs":"number"}"#), "{json}");
}

#[test]
fn a_mandatory_payment_that_is_not_switched_is_still_a_gap() {
    let (_, found) = card(
        "Creature Elephant",
        "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Check | TriggerDescription$ When this enters, pay {E}{E}.\nSVar:Check:DB$ Pump | UnlessCost$ Mandatory PayEnergy<2> | UnlessPayer$ You",
    );
    assert_eq!(found, vec!["cost:Mandatory".to_owned()]);
}

#[rstest]
#[case::at_least_four("Spell.Creature+cmcGE4", r#""min_value":4"#)]
#[case::at_least_six("Spell.cmcGE6", r#""min_value":6"#)]
fn mana_spent_only_on_spells_of_a_least_mana_value_is_read(
    #[case] restriction: &str,
    #[case] expected: &str,
) {
    let (json, found) = card(
        "Artifact",
        &format!("A:AB$ Mana | Cost$ T | Produced$ Any | RestrictValid$ {restriction}"),
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn a_may_shuffle_of_the_targeted_library_is_a_may() {
    let (json, found) = card(
        "Sorcery",
        "A:SP$ PeekAndReveal | ValidTgts$ Player | PeekAmount$ 5 | NoReveal$ True | SubAbility$ DBShuffle\nSVar:DBShuffle:DB$ Shuffle | Defined$ Targeted | Optional$ True",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#"{"May":{"effects":[{"Shuffle""#), "{json}");
}

#[rstest]
#[case::the_mode_of_choosing("Mode$ TgtChoose", true)]
#[case::the_milled_cards_shown("ShowMilledCards$ True", true)]
#[case::another_mode("Mode$ Random", false)]
fn a_mill_with_a_hint_beside_it_is_the_same_mill(#[case] extra: &str, #[case] read: bool) {
    let (json, found) = card(
        "Sorcery",
        &format!("A:SP$ Mill | Defined$ Player.Opponent | NumCards$ 1 | {extra}"),
    );
    assert_eq!(found.is_empty(), read, "{found:?}");
    assert_eq!(json.contains("Mill"), read, "{json}");
}

#[test]
fn creatures_of_the_targeted_player_that_the_card_names_again_are_pumped_by_defined() {
    let (json, found) = card(
        "Sorcery",
        "A:SP$ TapAll | ValidTgts$ Player | ValidCards$ Creature | SubAbility$ DBPumpAll\nSVar:DBPumpAll:DB$ PumpAll | Defined$ Targeted | ValidCards$ Creature | KW$ HIDDEN This card doesn't untap during your next untap step. | Duration$ Permanent\nOracle:Tap all creatures target player controls. Those creatures don't untap during that player's next untap step.",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains("NextUntapStep"), "{json}");
}

#[test]
fn all_colors_is_the_five() {
    let (json, found) = card(
        "Artifact Creature Shapeshifter",
        "A:AB$ Animate | Cost$ 1 | Defined$ Self | Colors$ All",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(r#""SetColors":["White","Blue","Black","Red","Green"]"#),
        "{json}"
    );
}

#[test]
fn a_pump_that_is_optional_is_a_may_and_two_of_them_stay_two() {
    let (json, found) = card(
        "Creature Elf",
        "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigPump | TriggerDescription$ When this enters, you may have Allies you control gain lifelink until end of turn, and you may put a +1/+1 counter on it.\nSVar:TrigPump:DB$ Pump | Defined$ Valid Creature.YouCtrl | KW$ Lifelink | Optional$ True | SubAbility$ DBCounter\nSVar:DBCounter:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1 | Optional$ True",
    );
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(json.matches(r#""May""#).count(), 2, "{json}");
}

#[test]
fn a_mana_line_of_a_sub_ability_repeats_the_cost_of_its_ability_and_is_not_paid_again() {
    let (_, found) = card(
        "Creature Elemental",
        "A:AB$ ChooseColor | Cost$ 1 | Defined$ You | SubAbility$ DBMana\nSVar:DBMana:DB$ Mana | Cost$ 1 | Produced$ Chosen",
    );
    assert_eq!(found, vec!["key:Mana.Cost-on-a-sub-ability".to_owned()]);
}

#[test]
fn a_may_with_a_target_chosen_before_the_mana_is_spent_is_a_gap() {
    let (_, found) = card(
        "Instant",
        "A:SP$ Pump | ValidTgts$ Creature | Optional$ True | NumAtt$ -3 | NumDef$ -3 | ConditionManaSpent$ B",
    );
    assert_eq!(found, vec!["condition:ManaSpent-with-a-target".to_owned()]);
}

#[test]
fn a_remembered_card_that_is_in_exile_gates_the_steps_after_its_move() {
    let (json, found) = card(
        "Creature Human",
        "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigChange\nSVar:TrigChange:DB$ ChangeZone | Origin$ Graveyard | Destination$ Exile | ValidTgts$ Creature | TargetMin$ 0 | TargetMax$ 1 | SubAbility$ DBDig | RememberChanged$ True\nSVar:DBDig:DB$ Dig | ConditionDefined$ Remembered | ConditionPresent$ Card | ConditionZone$ Exile | DigNum$ 3 | DestinationZone2$ Graveyard | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""selector":[{"In":"Exile"}]"#), "{json}");
}

#[rstest]
#[case::after_choosing_a_color("ChooseColor", true)]
#[case::with_no_color_chosen("ChooseType | Type$ Land", false)]
fn clearing_the_chosen_color_is_nothing_to_do_after_the_ability_chose_one(
    #[case] choose: &str,
    #[case] read: bool,
) {
    let (_, found) = card(
        "Creature Lizard",
        &format!(
            "A:AB$ {choose} | Cost$ 1 | Defined$ You | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearChosenColor$ True"
        ),
    );
    assert_eq!(found.is_empty(), read, "{found:?}");
}

#[test]
fn a_flip_until_the_first_loss_is_a_repeating_flip() {
    let (json, found) = card(
        "Creature Goblin",
        "T:Mode$ Phase | Phase$ BeginCombat | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigFlip | TriggerDescription$ At the beginning of combat on your turn, flip a coin until you lose a flip.\nSVar:TrigFlip:DB$ FlipCoin | FlipUntilYouLose$ True",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""FlipCoin":{"until":true}"#), "{json}");
}
