//! Goal: round 12 pack g12e reads target slots of verbs: a colour change of a target spell or permanent, a copy of up to
//! one target, a cast of a target card, a choice made by the targeted player, counters of typed spells and of up to one,
//! the permanents and graveyards of target players, the target players of tokens, and a target another player chooses,
//! and leaves the other shapes a gap by name.
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
#[case::spell_or_permanent_becomes_red(
    "Instant",
    "A:SP$ Animate | Colors$ Red | OverwriteColors$ True | ValidTgts$ Card | TgtZone$ Stack,Battlefield | Duration$ Permanent | SpellDescription$ Target spell or permanent becomes red.",
    "Target spell or permanent becomes red.",
    r#"SetColors":["Red"]"#
)]
#[case::spell_becomes_colorless(
    "Artifact Creature",
    "A:AB$ Animate | Cost$ T | Colors$ Colorless | OverwriteColors$ True | ValidTgts$ Card | TargetType$ Spell | TgtZone$ Stack | Duration$ Permanent | SpellDescription$ Target spell becomes colorless.",
    "{T}: Target spell becomes colorless.",
    r#""Object":["Spell"]"#
)]
#[case::copy_of_up_to_one(
    "Creature",
    "T:Mode$ Phase | Phase$ Main1 | ValidPlayer$ You | Execute$ C | TriggerZones$ Battlefield | TriggerDescription$ At the beginning of your first main phase, CARDNAME becomes a copy of up to one target land until end of turn.\nSVar:C:DB$ Clone | ValidTgts$ Land | TargetMin$ 0 | TargetMax$ 1 | Duration$ UntilEndOfTurn",
    "At the beginning of your first main phase, Subject becomes a copy of up to one target land until end of turn.",
    r#""min":0"#
)]
#[case::target_opponent_chooses(
    "Creature",
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ G | TriggerDescription$ When CARDNAME enters, target opponent may have you draw three cards. If the player doesn't, you gain 3 life.\nSVar:G:DB$ GenericChoice | ValidTgts$ Opponent | Choices$ D,L\nSVar:D:DB$ Draw | Defined$ You | NumCards$ 3 | SpellDescription$ You draw three cards.\nSVar:L:DB$ GainLife | Defined$ You | LifeAmount$ 3 | SpellDescription$ You gain 3 life.",
    "When Subject enters, target opponent may have you draw three cards. If the player doesn't, you gain 3 life.",
    r#""who":{"Target":0}"#
)]
#[case::cast_target_card(
    "Creature",
    "T:Mode$ SpellCast | ValidCard$ Instant | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | Execute$ P | TriggerDescription$ Whenever you cast an instant spell, you may cast up to one target sorcery card from your graveyard.\nSVar:P:DB$ Play | TgtZone$ Graveyard | ValidTgts$ Sorcery.YouOwn | ValidSA$ Spell | TargetMin$ 0 | TargetMax$ 1 | Optional$ True",
    "Whenever you cast an instant spell, you may cast up to one target sorcery card from your graveyard.",
    r#""CastFromZone":{"what":{"Target":0}}"#
)]
#[case::counter_a_colorless_spell_or_a_trigger(
    "Instant",
    "A:SP$ Counter | TargetType$ Spell.Colorless,Triggered | ValidTgts$ Card,Emblem | SpellDescription$ Counter target triggered ability or colorless spell.",
    "Counter target triggered ability or colorless spell.",
    r#"{"AnyOf":[{"Not":{"AnyOf":[{"Not":"Spell"},{"Not":"Colorless"}]}},{"Ability":"Triggered"}]}"#
)]
#[case::counter_an_instant_spell(
    "Instant",
    "A:SP$ Counter | TargetType$ Instant,Sorcery,Triggered | ValidTgts$ Card,Emblem | SpellDescription$ Counter target instant spell, sorcery spell, or triggered ability.",
    "Counter target instant spell, sorcery spell, or triggered ability.",
    r#"{"Not":{"AnyOf":[{"Not":"Spell"},{"Not":{"Type":"Sorcery"}}]}}"#
)]
#[case::counter_up_to_one_spell(
    "Instant",
    "A:SP$ Counter | TargetType$ Spell | TargetMin$ 0 | TargetMax$ 1 | ValidTgts$ Creature | SpellDescription$ Counter up to one target creature spell.",
    "Counter up to one target creature spell.",
    r#""bounds":{"max":1,"min":0}"#
)]
#[case::lands_target_player_controls(
    "Sorcery",
    "A:SP$ AnimateAll | ValidTgts$ Player | ValidCards$ Land | Power$ 3 | Toughness$ 3 | Types$ Creature | SpellDescription$ All lands target player controls become 3/3 creatures until end of turn.",
    "All lands target player controls become 3/3 creatures until end of turn.",
    r#""affected":{"Each":[{"Type":"Land"},{"ControlledBy":{"Target":0}}]}"#
)]
#[case::graveyards_of_any_number_of_players(
    "Sorcery",
    "A:SP$ ChangeZoneAll | Origin$ Graveyard | Destination$ Exile | ValidTgts$ Player | TargetMin$ 0 | TargetMax$ 4 | ChangeType$ Card | SpellDescription$ Exile any number of target players' graveyards.",
    "Exile any number of target players' graveyards.",
    r#"{"OwnedBy":{"Target":0}}"#
)]
#[case::target_player_creates(
    "Sorcery",
    "A:SP$ Token | ValidTgts$ Player | TokenScript$ w_1_1_soldier | SpellDescription$ Target player creates a 1/1 white Soldier creature token.",
    "Target player creates a 1/1 white Soldier creature token.",
    r#""who":{"Target":0}"#
)]
#[case::tokens_counted_from_the_target(
    "Sorcery",
    "A:SP$ Token | ValidTgts$ Player | TokenOwner$ You | TokenAmount$ X | TokenScript$ w_1_1_soldier | SpellDescription$ Choose target player. Create a 1/1 white Soldier creature token for each card in that player's hand.\nSVar:X:Count$ValidHand Card.TargetedPlayerOwn",
    "Choose target player. Create a 1/1 white Soldier creature token for each card in that player's hand.",
    r#""targets":[{"what":"Player"}]"#
)]
#[case::counter_up_to_two_spells(
    "Instant",
    "A:SP$ Counter | TargetType$ Spell | ValidTgts$ Card | TargetMin$ 0 | TargetMax$ 2 | SpellDescription$ Counter up to two target spells.",
    "Counter up to two target spells.",
    r#""bounds":{"max":2,"min":0}"#
)]
#[case::permanents_target_player_controls(
    "Sorcery",
    "A:SP$ ChangeZoneAll | ValidTgts$ Player | ChangeType$ Permanent.nonLand | Origin$ Battlefield | Destination$ Hand | SpellDescription$ Return all nonland permanents target player controls to their owner's hand.",
    "Return all nonland permanents target player controls to their owner's hand.",
    r#"{"ControlledBy":{"Target":0}}"#
)]
#[case::clash_then_the_target(
    "Creature",
    "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ C | TriggerDescription$ Whenever CARDNAME attacks, clash with an opponent. If you win, target creature gains double strike until end of turn.\nSVar:C:DB$ Clash | ValidTgts$ Creature | WinSubAbility$ P\nSVar:P:DB$ Pump | Defined$ Targeted | KW$ Double Strike",
    "Whenever Subject attacks, clash with an opponent. If you win, target creature gains double strike until end of turn.",
    r#""win":[{"Continuous":{"affected":{"Target":0}"#
)]
#[case::target_of_an_opponents_choice(
    "Creature",
    "A:AB$ Destroy | Cost$ T | ValidTgts$ Creature | TargetingPlayer$ Player.Opponent | SpellDescription$ Destroy target creature of an opponent's choice.",
    "{T}: Destroy target creature of an opponent's choice.",
    r#"{"ChosenBy":{"what":{"Object":[{"Type":"Creature"}]},"who":"Opponents"}}"#
)]
#[case::target_of_the_upkeep_players_choice(
    "Enchantment",
    "T:Mode$ Phase | Phase$ Upkeep | TriggerZones$ Battlefield | Execute$ D | TriggerDescription$ At the beginning of each player's upkeep, destroy target creature that player controls of their choice.\nSVar:D:DB$ Destroy | ValidTgts$ Creature.ActivePlayerCtrl | TargetingPlayer$ TriggeredPlayer",
    "At the beginning of each player's upkeep, destroy target creature that player controls of their choice.",
    r#""who":"Triggering""#
)]
fn target_slots_are_read(
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
#[case::spell_gains_a_keyword(
    "Instant",
    "A:SP$ Animate | Keywords$ Flying | ValidTgts$ Card | TgtZone$ Stack | SpellDescription$ Target spell gains flying.",
    "Target spell gains flying.",
    "key:Animate.TgtZone=Stack"
)]
#[case::copy_of_up_to_two(
    "Creature",
    "A:AB$ Clone | Cost$ T | ValidTgts$ Creature | TargetMin$ 0 | TargetMax$ 2 | SpellDescription$ CARDNAME becomes a copy of up to two target creatures.",
    "{T}: Subject becomes a copy of up to two target creatures.",
    "key:Clone.TargetMin"
)]
#[case::target_creature_chooses(
    "Sorcery",
    "A:SP$ GenericChoice | ValidTgts$ Creature | Choices$ D,L | SpellDescription$ Target creature's controller chooses.\nSVar:D:DB$ Draw | Defined$ You | NumCards$ 3 | SpellDescription$ You draw three cards.\nSVar:L:DB$ GainLife | Defined$ You | LifeAmount$ 3 | SpellDescription$ You gain 3 life.",
    "Target creature's controller chooses.",
    "key:GenericChoice.ValidTgts-chooser"
)]
#[case::chosen_by_the_controller_of_another_target(
    "Creature",
    "A:AB$ Pump | Cost$ T | ValidTgts$ Creature | TargetingPlayer$ ParentTargetedController | NumAtt$ -1 | SpellDescription$ Target creature of that player's choice gets -1/-0 until end of turn.",
    "{T}: Target creature of that player's choice gets -1/-0 until end of turn.",
    "key:TargetingPlayer=ParentTargetedController"
)]
#[case::chosen_among_the_choosers_creatures(
    "Creature",
    "A:AB$ Tap | Cost$ T | TargetingPlayer$ Player.Opponent | TargetingPlayerControls$ True | ValidTgts$ Creature | SpellDescription$ Tap target creature of an opponent's choice they control.",
    "{T}: Tap target creature of an opponent's choice they control.",
    "key:Tap.TargetingPlayerControls"
)]
fn other_target_slots_stay_gaps(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] gap: &str,
) {
    let (_, gaps) = card(types, lines, oracle);
    assert!(gaps.iter().any(|found| found == gap), "{gaps:?}");
}
