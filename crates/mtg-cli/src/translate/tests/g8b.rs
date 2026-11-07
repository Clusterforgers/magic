//! Goal: round 8 pack g8b reads what a card remembers and reads back: a memory as the base of a valid-string, the object of
//! a pass, the spell cast, the permanents tapped or manifested, the cards a cost exiled, picks that add up, the chosen
//! player and the number a reflexive trigger keeps; each reading that would mean something else stays a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given type line and lines as compact JSON, and its gaps.
fn card(types: &str, lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

/// Asserts that the card reads `expected` with no gap, or stays the gap `gap` when one is given.
fn reads(types: &str, lines: &str, expected: &str, gap: &str) {
    let (json, found) = card(types, lines);
    if gap.is_empty() {
        assert!(found.is_empty(), "{found:?}");
        assert!(json.contains(expected), "{json}");
    } else {
        assert!(found.iter().any(|g| g.contains(gap)), "{found:?} {json}");
    }
}

#[rstest]
#[case::same_name_as_the_target(
    "A:SP$ Pump | ValidTgts$ Creature | RememberTargets$ True | NumAtt$ -3 | NumDef$ -3 | SubAbility$ All\nSVar:All:DB$ PumpAll | ValidCards$ Remembered.sameName+Other+Creature | NumAtt$ -3 | NumDef$ -3",
    r#"{"SharesName":{"Target":0}},{"Not":{"Is":{"Target":0}}}"#,
    ""
)]
#[case::the_remembered_cards_themselves(
    "A:SP$ Mill | NumCards$ 3 | Defined$ You | RememberMilled$ True | SubAbility$ Back\nSVar:Back:DB$ ChangeZoneAll | Origin$ Graveyard | Destination$ Hand | ChangeType$ Remembered.Creature",
    r#"{"Is":{"Bound":"#,
    ""
)]
#[case::nothing_remembered(
    "A:SP$ ChangeZoneAll | Origin$ Graveyard | Destination$ Exile | ChangeType$ Remembered.sameName",
    "",
    "valid-base:Remembered"
)]
#[case::the_player_of_a_pass_is_no_object(
    "A:SP$ RepeatEach | RepeatPlayers$ Player | RepeatSubAbility$ X\nSVar:X:DB$ ChangeZoneAll | Origin$ Graveyard | Destination$ Exile | ChangeType$ Remembered.sameName",
    "",
    "valid-base:Remembered"
)]
fn a_memory_is_the_base_of_a_valid_string(
    #[case] lines: &str,
    #[case] expected: &str,
    #[case] gap: &str,
) {
    reads("Instant", lines, expected, gap);
}

#[rstest]
#[case::each_creature_and_its_controller(
    "A:SP$ RepeatEach | RepeatCards$ Creature | RepeatSubAbility$ D\nSVar:D:DB$ DealDamage | Defined$ RememberedController | DamageSource$ Remembered | NumDmg$ 1",
    r#""to":{"ControllerOf":{"Bound":"each"}}"#,
    ""
)]
#[case::a_pass_over_players_has_no_controller(
    "A:SP$ RepeatEach | RepeatPlayers$ Player | RepeatSubAbility$ D\nSVar:D:DB$ Draw | Defined$ RememberedController",
    "",
    "defined:RememberedController"
)]
fn the_object_of_a_pass_names_its_controller(
    #[case] lines: &str,
    #[case] expected: &str,
    #[case] gap: &str,
) {
    reads("Sorcery", lines, expected, gap);
}

#[rstest]
#[case::a_spell_cast_if_you_do("ValidSA$ Spell", r#""CastFromZone":{"as":"cast"#, "")]
#[case::a_card_played_may_be_a_land("", "", "key:Play.RememberPlayed")]
fn the_spell_cast_is_remembered(#[case] valid: &str, #[case] expected: &str, #[case] gap: &str) {
    let lines = format!(
        "A:SP$ Dig | DigNum$ 1 | ChangeNum$ 1 | DestinationZone$ Exile | RememberChanged$ True | SubAbility$ P\nSVar:P:DB$ Play | Defined$ Remembered | WithoutManaCost$ True | Optional$ True | RememberPlayed$ True | {valid} | SubAbility$ L\nSVar:L:DB$ LoseLife | LifeAmount$ 2 | ConditionDefined$ Remembered | ConditionPresent$ Card"
    );
    reads("Sorcery", &lines.replace("|  |", "|"), expected, gap);
}

#[rstest]
#[case::tapped_this_way(
    "A:SP$ TapAll | ValidCards$ Creature.OppCtrl | RememberTapped$ True | SubAbility$ S\nSVar:S:DB$ PutCounter | Defined$ Remembered | CounterType$ STUN | CounterNum$ 1",
    r#""Tap":{"as":"tapped""#
)]
#[case::manifested(
    "A:SP$ Manifest | RememberManifested$ True | SubAbility$ S\nSVar:S:DB$ PutCounter | Defined$ Remembered | CounterType$ P1P1 | CounterNum$ 2",
    r#""Manifest":{"as":"manifest""#
)]
fn a_verb_binds_what_it_acted_on(#[case] lines: &str, #[case] expected: &str) {
    reads("Sorcery", lines, expected, "");
}

#[rstest]
#[case::exiled_alone(
    "1 B ExileFromGrave<1/Creature>",
    r#"{"Stat":{"of":"Paid","stat":"Power"}}"#,
    ""
)]
#[case::exiled_beside_a_sacrifice(
    "1 Sac<1/Creature> ExileFromGrave<1/Creature>",
    "",
    "Exiled-beside-other-cost-picks"
)]
fn the_card_a_cost_exiled_is_read(#[case] cost: &str, #[case] expected: &str, #[case] gap: &str) {
    let lines = format!(
        "A:AB$ LoseLife | Cost$ {cost} | ValidTgts$ Opponent | LifeAmount$ X\nSVar:X:Exiled$CardPower"
    );
    reads("Artifact", &lines, expected, gap);
}

#[rstest]
#[case::picks_add_up(
    "A:SP$ ChooseCard | Defined$ You | Choices$ Creature.YouOwn+cmcEQ1 | ChoiceZone$ Graveyard | Mandatory$ True | RememberChosen$ True | SubAbility$ Two\nSVar:Two:DB$ ChooseCard | Defined$ You | Choices$ Creature.YouOwn+cmcEQ2 | ChoiceZone$ Graveyard | Mandatory$ True | RememberChosen$ True | SubAbility$ Back\nSVar:Back:DB$ ChangeZoneAll | Origin$ Graveyard | Destination$ Battlefield | ChangeType$ Card.IsRemembered",
    r#"{"AnyOf":[{"Is":{"Bound":"card"}},{"Is":{"Bound":"card2"}}]}"#,
    ""
)]
#[case::a_pick_beside_cards_moved(
    "A:SP$ Mill | NumCards$ 3 | Defined$ You | RememberMilled$ True | SubAbility$ Two\nSVar:Two:DB$ ChooseCard | Defined$ You | Choices$ Creature.YouOwn | ChoiceZone$ Graveyard | RememberChosen$ True | SubAbility$ Back\nSVar:Back:DB$ ChangeZoneAll | Origin$ Graveyard | Destination$ Hand | ChangeType$ Card.IsRemembered",
    "",
    "key:ChooseCard.RememberChosen-with-remembered"
)]
#[case::a_pick_in_a_pass_over_players(
    "A:SP$ RepeatEach | RepeatPlayers$ Player | RepeatSubAbility$ Pick\nSVar:Pick:DB$ ChooseCard | Defined$ You | Choices$ Card.RememberedPlayerCtrl | ChoiceZone$ Graveyard | RememberChosen$ True",
    "",
    "key:ChooseCard.RememberChosen-with-remembered"
)]
fn chosen_cards_add_up_in_the_memory(
    #[case] lines: &str,
    #[case] expected: &str,
    #[case] gap: &str,
) {
    reads("Sorcery", lines, expected, gap);
}

#[rstest]
#[case::the_player_chosen(
    "A:SP$ ChoosePlayer | Defined$ You | Choices$ Player.Opponent | SubAbility$ G\nSVar:G:DB$ GainControl | ValidTgts$ Creature.YouCtrl | NewController$ Player.Chosen",
    r#"{"SetController":{"Bound":"player"}}"#,
    ""
)]
#[case::a_card_chosen_is_no_player(
    "A:SP$ ChooseCard | Defined$ You | Choices$ Creature.YouCtrl | SubAbility$ G\nSVar:G:DB$ GainControl | Defined$ ChosenCard | NewController$ Player.Chosen",
    "",
    "defined:Player.Chosen"
)]
fn the_chosen_player_is_read(#[case] lines: &str, #[case] expected: &str, #[case] gap: &str) {
    reads("Sorcery", lines, expected, gap);
}

#[rstest]
#[case::in_a_pass_over_players(
    "A:SP$ RepeatEach | RepeatPlayers$ Player | RepeatSubAbility$ M\nSVar:M:DB$ Mill | Defined$ Remembered | NumCards$ X\nSVar:X:Count$ValidLibrary Card.RememberedPlayerOwn/HalfDown",
    r#"{"OwnedBy":{"Bound":"each"}}"#,
    ""
)]
#[case::cards_are_no_owner(
    "A:SP$ Mill | NumCards$ 3 | Defined$ You | RememberMilled$ True | SubAbility$ M\nSVar:M:DB$ Draw | NumCards$ X\nSVar:X:Count$ValidGraveyard Card.RememberedPlayerOwn",
    "",
    "valid:RememberedPlayerOwn"
)]
#[case::the_player_and_the_cards_of_a_pass(
    "A:SP$ Discard | Mode$ TgtChoose | Defined$ Player.Opponent | NumCards$ 1 | RememberDiscarded$ True | SubAbility$ R\nSVar:R:DB$ RepeatEach | RepeatPlayers$ Player.Opponent | RepeatSubAbility$ L\nSVar:L:DB$ LoseLife | Defined$ Remembered | LifeAmount$ X\nSVar:X:Remembered$Valid Card.nonLand+RememberedPlayerOwn",
    "",
    "amount:Remembered-player-and-cards"
)]
fn the_remembered_player_owns(#[case] lines: &str, #[case] expected: &str, #[case] gap: &str) {
    reads("Sorcery", lines, expected, gap);
}

#[rstest]
#[case::kept("RememberSVarAmount$ Y | ", r#"{"Let":{"amount":"#, "")]
#[case::not_kept("", "", "amount:TriggerRememberAmount")]
fn a_reflexive_trigger_reads_the_number_kept(
    #[case] keep: &str,
    #[case] expected: &str,
    #[case] gap: &str,
) {
    let lines = format!(
        "A:SP$ ImmediateTrigger | {keep}Execute$ D\nSVar:D:DB$ DealDamage | ValidTgts$ Creature | NumDmg$ X\nSVar:X:Count$TriggerRememberAmount\nSVar:Y:Count$CardsInYourHand"
    );
    reads("Sorcery", &lines, expected, gap);
    if gap.is_empty() {
        reads("Sorcery", &lines, r#""amount":{"Bound":"number"}"#, "");
    }
}

/// The targets of the trigger are chosen before its resolution holds the number, so a target that reads it is a gap.
#[test]
fn a_target_of_the_reflexive_trigger_cannot_read_the_number_kept() {
    let lines = "A:SP$ ImmediateTrigger | RememberSVarAmount$ Y | Execute$ D\nSVar:D:DB$ Destroy | ValidTgts$ Artifact.cmcLEX\nSVar:X:Count$TriggerRememberAmount\nSVar:Y:Count$CardsInYourHand";
    reads(
        "Sorcery",
        lines,
        "",
        "key:ImmediateTrigger.RememberSVarAmount-in-a-target",
    );
}

#[rstest]
#[case::power("Imprinted$CardPower", r#""stat":"Power""#)]
#[case::matching("Imprinted$Valid Creature", r#"{"Type":"Creature"}"#)]
fn an_imprinted_card_is_counted(#[case] word: &str, #[case] expected: &str) {
    let lines = format!(
        "A:SP$ ChangeZone | Origin$ Graveyard | Destination$ Exile | ValidTgts$ Card | Imprint$ True | SubAbility$ G\nSVar:G:DB$ GainLife | LifeAmount$ X\nSVar:X:{word}"
    );
    reads("Sorcery", &lines, expected, "");
}
