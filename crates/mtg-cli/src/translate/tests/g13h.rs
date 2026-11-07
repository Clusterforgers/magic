//! Goal: round 13 pack g13h reads casts that exile the spell instead of a graveyard, casts of a remembered card by mana
//! value, delayed steps that need the card still exiled, copies of the spell itself with the controller of a target
//! copying, a non-legendary copy, a payer that is the target's controller, a choice the targeted opponent makes, and
//! leaves the shapes that read looser or other than the Oracle text a gap by name.
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
#[case::cast_from_a_graveyard_exiles_instead(
    "Sorcery",
    "A:SP$ Play | TgtZone$ Graveyard | WithoutManaCost$ True | ValidTgts$ Instant.OppOwn,Sorcery.OppOwn | ValidSA$ Spell | Optional$ True | ReplaceGraveyard$ Exile | SpellDescription$ You may cast target instant or sorcery card from an opponent's graveyard without paying its mana cost. If that spell would be put into their graveyard, exile it instead.",
    "You may cast target instant or sorcery card from an opponent's graveyard without paying its mana cost. If that spell would be put into their graveyard, exile it instead.",
    r#""Shield":{"by":{"MoveTo":"Exile"}"#
)]
#[case::cast_exiles_instead_of_a_graveyard_only_for_the_types(
    "Creature",
    "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ C | TriggerDescription$ Whenever this creature attacks, you may cast an artifact, instant, or sorcery spell from your graveyard. If an instant or sorcery spell cast this way would be put into your graveyard, exile it instead.\nSVar:C:DB$ Play | ValidZone$ Graveyard | Valid$ Card.YouOwn | ValidSA$ Spell.Artifact,Spell.Instant,Spell.Sorcery | Controller$ You | Optional$ True | ReplaceGraveyard$ Exile | ReplaceGraveyardValid$ Instant,Sorcery",
    "Whenever this creature attacks, you may cast an artifact, instant, or sorcery spell from your graveyard. If an instant or sorcery spell cast this way would be put into your graveyard, exile it instead.",
    r#""what":[{"In":"Stack"},{"Is":{"Bound":"cast"}},{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]"#
)]
#[case::cast_the_exiled_card_when_its_value_is_low(
    "Enchantment",
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ D | TriggerDescription$ When this enchantment enters, exile cards from the top of your library until you exile a nonland card. You may cast it without paying its mana cost if that spell's mana value is 8 or less. If you don't, put that card into your hand.\nSVar:D:DB$ DigUntil | Valid$ Card.nonLand | FoundDestination$ Exile | RevealedDestination$ Exile | RememberFound$ True | SubAbility$ P\nSVar:P:DB$ Play | Defined$ Remembered | ValidSA$ Spell.cmcLE8 | WithoutManaCost$ True | Optional$ True | ForgetPlayed$ True | SubAbility$ H\nSVar:H:DB$ ChangeZone | Origin$ Exile | Destination$ Hand | Defined$ Remembered",
    "When this enchantment enters, exile cards from the top of your library until you exile a nonland card. You may cast it without paying its mana cost if that spell's mana value is 8 or less. If you don't, put that card into your hand.",
    r#""Compare":{"op":"Le","stat":"ManaValue","to":8}"#
)]
#[case::pump_until_your_next_upkeep(
    "Creature",
    "T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ P | TriggerDescription$ At the beginning of your upkeep, target creature an opponent controls gains flying until your next upkeep.\nSVar:P:DB$ Pump | ValidTgts$ Creature.OppCtrl | KW$ Flying | Duration$ UntilYourNextUpkeep",
    "At the beginning of your upkeep, target creature an opponent controls gains flying until your next upkeep.",
    r#""duration":"UntilYourNextTurn""#
)]
#[case::return_the_card_if_it_remains_exiled(
    "Sorcery",
    "A:SP$ ChangeZone | Origin$ Hand | Destination$ Exile | ChangeType$ Card | ChangeNum$ 1 | RememberChanged$ True | SubAbility$ D | SpellDescription$ Exile a card from your hand. At the beginning of the next end step, if it remains exiled, return it to your hand.\nSVar:D:DB$ DelayedTrigger | Mode$ Phase | Phase$ End of Turn | Execute$ R | IsPresent$ Card.IsTriggerRemembered | PresentZone$ Exile | RememberObjects$ Remembered\nSVar:R:DB$ ChangeZone | Defined$ DelayTriggerRememberedLKI | Origin$ Exile | Destination$ Hand",
    "Exile a card from your hand. At the beginning of the next end step, if it remains exiled, return it to your hand.",
    r#""StepBegins":{"step":"End""#
)]
#[case::blocker_leaves_at_end_of_combat(
    "Creature",
    "T:Mode$ Blocks | ValidCard$ Card.Self | Execute$ D | TriggerDescription$ When this blocks, return it to its owner's hand at end of combat.\nSVar:D:DB$ DelayedTrigger | Mode$ Phase | Phase$ EndCombat | ValidPlayer$ Opponent | Execute$ B\nSVar:B:DB$ ChangeZone | Origin$ Battlefield | Destination$ Hand | Defined$ Self",
    "When this blocks, return it to its owner's hand at end of combat.",
    r#""StepBegins":{"step":"EndCombat""#
)]
#[case::target_controller_copies_the_spell_after_paying(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 3 | SubAbility$ C | SpellDescription$ Deals 3 damage to any target. Then that player or that permanent's controller may pay {R}{R}. If the player does, they may copy this spell and may choose a new target for that copy.\nSVar:C:DB$ CopySpellAbility | Defined$ Parent | Controller$ TargetedOrController | UnlessPayer$ TargetedOrController | UnlessCost$ R R | UnlessSwitched$ True | MayChooseTarget$ True",
    "Deals 3 damage to any target. Then that player or that permanent's controller may pay {R}{R}. If the player does, they may copy this spell and may choose a new target for that copy.",
    r#""CopySpell":{"new_targets":true,"what":"This","who":{"OrController":{"Target":0}}}"#
)]
#[case::target_controller_may_copy_the_spell(
    "Sorcery",
    "A:SP$ Destroy | ValidTgts$ Permanent.nonCreature | SubAbility$ C | SpellDescription$ Destroy target noncreature permanent. Then that permanent's controller may copy this spell and may choose a new target for that copy.\nSVar:C:DB$ CopySpellAbility | Defined$ Parent | Controller$ TargetedController | Optional$ True | MayChooseTarget$ True",
    "Destroy target noncreature permanent. Then that permanent's controller may copy this spell and may choose a new target for that copy.",
    r#""May":{"effects":[{"CopySpell":{"new_targets":true,"what":"This","who":{"ControllerOf":{"Target":0}}}}],"who":{"ControllerOf":{"Target":0}}}"#
)]
#[case::copy_of_a_spell_that_is_not_legendary(
    "Instant",
    "A:SP$ CopySpellAbility | ValidTgts$ Creature.YouCtrl | TargetType$ Spell | NonLegendary$ True | SpellDescription$ Copy target creature spell you control, except it isn't legendary if the spell is legendary.",
    "Copy target creature spell you control, except it isn't legendary if the spell is legendary.",
    r#""except":[{"RemoveSupertype":"Legendary"}]"#
)]
#[case::controller_of_the_spell_pays_or_the_target_changes(
    "Instant",
    "A:SP$ ChangeTargets | TargetType$ Spell.singleTarget | ValidTgts$ Card | UnlessCost$ 2 | SpellDescription$ Change the target of target spell with a single target unless that spell's controller pays {2}.",
    "Change the target of target spell with a single target unless that spell's controller pays {2}.",
    r#""who":{"ControllerOf":{"Target":0}}"#
)]
#[case::lose_life_unless_sacrifice_or_discard(
    "Creature",
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ G | TriggerDescription$ When this creature enters, target opponent loses 3 life unless they sacrifice a nonland permanent or discard a card.\nSVar:G:DB$ GenericChoice | ValidTgts$ Opponent | Choices$ S,D | FallbackAbility$ F\nSVar:S:DB$ LoseLife | LifeAmount$ 3 | Defined$ Targeted | UnlessCost$ Sac<1/Permanent.nonLand/nonland permanent> | UnlessPayer$ Targeted\nSVar:D:DB$ LoseLife | LifeAmount$ 3 | Defined$ Targeted | UnlessCost$ Discard<1/Card> | UnlessPayer$ Targeted\nSVar:F:DB$ LoseLife | Defined$ Targeted | LifeAmount$ 3",
    "When this creature enters, target opponent loses 3 life unless they sacrifice a nonland permanent or discard a card.",
    r#""OneOf":{"options""#
)]
#[case::target_opponent_may_draw(
    "Creature",
    "A:AB$ Pump | Cost$ W T | ValidTgts$ Creature | KW$ Flying | SubAbility$ D | SpellDescription$ Target creature gains flying until end of turn. Target opponent may draw a card.\nSVar:D:DB$ Draw | ValidTgts$ Opponent | OptionalDecider$ Opponent",
    "{W}, {T}: Target creature gains flying until end of turn. Target opponent may draw a card.",
    r#""May":{"effects":[{"Draw":{"who":{"Target":1}}}],"who":{"Target":1}}"#
)]
#[case::copy_the_exiled_card_three_times(
    "Sorcery",
    "A:SP$ ChangeZone | ValidTgts$ Instant,Sorcery | Origin$ Graveyard | Destination$ Exile | RememberChanged$ True | SubAbility$ P | SpellDescription$ Exile target instant or sorcery card from a graveyard. Copy that card three times. You may cast the copies without paying their mana costs.\nSVar:P:DB$ Play | Defined$ Remembered | CopyCard$ True | Amount$ 3 | AllowRepeats$ True | Controller$ You | Optional$ True | WithoutManaCost$ True | ValidSA$ Spell",
    "Exile target instant or sorcery card from a graveyard. Copy that card three times. You may cast the copies without paying their mana costs.",
    r#""CopyCard":{"as":"copy4""#
)]
fn reads(#[case] types: &str, #[case] lines: &str, #[case] oracle: &str, #[case] part: &str) {
    let (json, gaps) = card(types, lines, oracle);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::exile_to_another_zone(
    "Sorcery",
    "A:SP$ Play | TgtZone$ Graveyard | ValidTgts$ Instant,Sorcery | ValidSA$ Spell | Optional$ True | ReplaceGraveyard$ Hand | SpellDescription$ You may cast target instant or sorcery card from a graveyard.",
    "You may cast target instant or sorcery card from a graveyard.",
    "key:Play.ReplaceGraveyard"
)]
#[case::less_than_is_not_at_most(
    "Sorcery",
    "A:SP$ DigUntil | Valid$ Card.nonLand | FoundDestination$ Exile | RevealedDestination$ Exile | RememberFound$ True | SubAbility$ P | SpellDescription$ Exile cards until you exile a nonland card. You may cast it if the spell's mana value is less than the number of creatures you control.\nSVar:P:DB$ Play | Defined$ Remembered | ValidSA$ Spell.cmcLEX | WithoutManaCost$ True | Optional$ True\nSVar:X:Count$Valid Creature.YouCtrl",
    "Exile cards until you exile a nonland card. You may cast it if the spell's mana value is less than the number of creatures you control.",
    "key:Play.Defined-with-ValidSA-filter"
)]
#[case::otherwise_may_mean_a_declined_cast(
    "Sorcery",
    "A:SP$ DigUntil | Valid$ Card.nonLand | FoundDestination$ Exile | RevealedDestination$ Exile | RememberFound$ True | SubAbility$ P | SpellDescription$ Exile cards until you exile a nonland card. You may cast it if its mana value is 3 or less. Otherwise, put it into your hand.\nSVar:P:DB$ Play | Defined$ Remembered | ValidSA$ Spell.cmcLE3 | WithoutManaCost$ True | Optional$ True",
    "Exile cards until you exile a nonland card. You may cast it if its mana value is 3 or less. Otherwise, put it into your hand.",
    "key:Play.Defined-with-ValidSA-filter"
)]
#[case::present_without_a_move_from_exile(
    "Sorcery",
    "A:SP$ ChangeZone | Origin$ Hand | Destination$ Exile | ChangeType$ Card | ChangeNum$ 1 | RememberChanged$ True | SubAbility$ D | SpellDescription$ Exile a card from your hand. At the beginning of the next end step, if it remains exiled, draw a card.\nSVar:D:DB$ DelayedTrigger | Mode$ Phase | Phase$ End of Turn | Execute$ R | IsPresent$ Card.IsTriggerRemembered | PresentZone$ Exile | RememberObjects$ Remembered\nSVar:R:DB$ Draw",
    "Exile a card from your hand. At the beginning of the next end step, if it remains exiled, draw a card.",
    "key:DelayedTrigger.IsPresent"
)]
#[case::copy_of_an_ability_of_the_trigger(
    "Artifact",
    "T:Mode$ AbilityCast | ValidActivatingPlayer$ You | ValidSA$ SpellAbility.!ManaAbility | TriggerZones$ Battlefield | Execute$ C | OptionalDecider$ You | TriggerDescription$ Whenever you activate an ability, if it isn't a mana ability, you may pay {2}. If you do, copy that ability.\nSVar:C:AB$ CopySpellAbility | Cost$ 2 | Defined$ TriggeredSpellAbility | MayChooseTarget$ True",
    "Whenever you activate an ability, if it isn't a mana ability, you may pay {2}. If you do, copy that ability. You may choose new targets for the copy.",
    "key:CopySpellAbility.Defined"
)]
#[case::another_players_look_at_their_cards(
    "Sorcery",
    "A:SP$ DigMultiple | Defined$ Player | DigNum$ 5 | Optional$ True | ChangeValid$ Land,Spell.!Aura | RestRandomOrder$ True | SpellDescription$ Each player looks at the top five cards of their library and may reveal a land card and/or an instant or sorcery card from among them.",
    "Each player looks at the top five cards of their library and may reveal a land card and/or an instant or sorcery card from among them.",
    "key:DigMultiple.Defined-not-you"
)]
fn stays_a_gap(#[case] types: &str, #[case] lines: &str, #[case] oracle: &str, #[case] gap: &str) {
    let (_, gaps) = card(types, lines, oracle);
    assert!(gaps.iter().any(|found| found == gap), "{gaps:?}");
}

#[test]
fn an_instant_or_sorcery_among_the_top_cards_is_a_card_not_a_spell_on_the_stack() {
    let (json, _) = card(
        "Sorcery",
        "A:SP$ DigMultiple | Defined$ You | DigNum$ 5 | Optional$ True | ChangeValid$ Land,Spell.!Aura | RestRandomOrder$ True | SpellDescription$ Look at the top five cards of your library and take a land card and an instant or sorcery card.",
        "Look at the top five cards of your library and take a land card and an instant or sorcery card.",
    );
    assert!(
        json.contains(r#"{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}"#),
        "{json}"
    );
    assert!(!json.contains(r#","Spell","#), "{json}");
}
