//! Goal: round 12 pack g12f reads what an `Effect` fixes as it begins (`SetChosenNumber`, the chosen card name), a memory
//! nobody writes as empty, a name a condition or a "may" bound where a later move or "when you do" names it, the source a
//! delayed trigger waits for, picks gathered over a repeat for each player and the cards a dig revealed counted before
//! they move; the shapes beside them that mean something else stay gaps.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// Asserts that the card reads every part of `expected` with no gap, or stays the gap `gap` when one is given.
fn reads(types: &str, lines: &str, oracle: &str, expected: &[&str], gap: &str) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\nOracle:{oracle}\n");
    let found = gaps(&text);
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    if gap.is_empty() {
        assert!(found.is_empty(), "{found:?} {json}");
        for part in expected {
            assert!(json.contains(part), "{part} in {json}");
        }
    } else {
        assert!(found.iter().any(|g| g.contains(gap)), "{found:?} {json}");
    }
}

#[rstest]
#[case::war_cadence_x_fixed_as_the_rule_begins(
    "Enchantment",
    "A:AB$ Effect | Cost$ X R | StaticAbilities$ S | SetChosenNumber$ X\nSVar:S:Mode$ CantBlockUnless | Cost$ XChosen\nSVar:X:Count$xPaid\nSVar:XChosen:Count$ChosenNumber",
    "{X}{R}: This turn, creatures can't block unless their controller pays {X} for each blocking creature they control.",
    &[r#"{"BlockTax":{"mana":"{1}","per":"X"}}"#, r#""duration":"UntilEndOfTurn""#],
    ""
)]
#[case::war_tax_no_one_named_is_anyone(
    "Enchantment",
    "A:AB$ Effect | Cost$ X U | StaticAbilities$ S | SetChosenNumber$ X\nSVar:S:Mode$ CantAttackUnless | ValidCard$ Creature | Cost$ XChosen\nSVar:X:Count$xPaid\nSVar:XChosen:Count$ChosenNumber",
    "{X}{U}: This turn, creatures can't attack unless their controller pays {X} for each attacking creature they control.",
    &[r#"{"AttackTax":{"attacked":"Both","mana":"{1}","per":"X","who":"EachPlayer"}}"#],
    ""
)]
#[case::a_chosen_number_outside_an_effect_stays_a_gap(
    "Sorcery",
    "A:SP$ GainLife | LifeAmount$ X\nSVar:X:Count$ChosenNumber",
    "You gain life.",
    &[],
    "amount:ChosenNumber-without-a-choice"
)]
#[case::a_chosen_source_remembers_nothing(
    "Enchantment",
    "A:AB$ ChooseSource | Cost$ W | Choices$ Card,Emblem | SubAbility$ E\nSVar:E:DB$ Effect | ReplacementEffects$ R | RememberObjects$ Remembered | ConditionDefined$ ChosenCard | ConditionPresent$ Card,Emblem\nSVar:R:Event$ DamageDone | ValidSource$ Card.ChosenCardStrict,Emblem.ChosenCard | ValidTarget$ You | ReplaceWith$ X | PreventionEffect$ True\nSVar:X:DB$ ChangeZone | Defined$ Self | Origin$ Command | Destination$ Exile",
    "{W}: The next time a source of your choice would deal damage to you this turn, prevent that damage.",
    &[r#""by":"PreventNext""#, r#""to":"You""#],
    ""
)]
#[case::an_unwritten_memory_as_a_recipient_stays_a_gap(
    "Enchantment",
    "A:AB$ ChooseSource | Cost$ W | Choices$ Card,Emblem | SubAbility$ E\nSVar:E:DB$ Effect | ReplacementEffects$ R | RememberObjects$ Remembered\nSVar:R:Event$ DamageDone | ValidSource$ Card.ChosenCardStrict | ValidTarget$ Card.IsRemembered | ReplaceWith$ X | PreventionEffect$ True\nSVar:X:DB$ ChangeZone | Defined$ Self | Origin$ Command | Destination$ Exile",
    "{W}: Prevent the next damage.",
    &[],
    "replacement:DamageDone-shield-recipient"
)]
#[case::exiled_if_then_returned_transformed(
    "Creature",
    "A:AB$ Draw | Cost$ T | SubAbility$ E\nSVar:E:DB$ ChangeZone | Origin$ Battlefield | Destination$ Exile | RememberChanged$ True | ConditionCheckSVar$ X | ConditionSVarCompare$ GE5 | SubAbility$ R\nSVar:R:DB$ ChangeZone | Defined$ Remembered | Origin$ Exile | Destination$ Battlefield | Transformed$ True | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Count$ValidGraveyard Card.YouOwn",
    "{T}: Draw a card. If there are five or more cards in your graveyard, exile this creature, then return it to the battlefield transformed.",
    &[r#""then":[{"Exile":{"as":"exiled","what":"This"}}]"#, r#"{"Move":{"to":{"Battlefield":{"transformed":true}},"what":{"Bound":"exiled"}}}"#],
    ""
)]
#[case::a_scoped_move_read_from_two_zones_stays_a_gap(
    "Creature",
    "A:AB$ Draw | Cost$ T | SubAbility$ E\nSVar:E:DB$ ChangeZone | Origin$ Battlefield | Destination$ Exile | RememberChanged$ True | ConditionCheckSVar$ X | ConditionSVarCompare$ GE5 | SubAbility$ R\nSVar:R:DB$ ChangeZone | Defined$ Remembered | Origin$ Exile,Graveyard | Destination$ Hand\nSVar:X:Count$ValidGraveyard Card.YouOwn",
    "{T}: Draw a card.",
    &[],
    "Remembered-without-a-change"
)]
#[case::discard_a_card_when_you_do(
    "Creature",
    "A:AB$ Draw | Cost$ T | SubAbility$ D\nSVar:D:DB$ Discard | Defined$ You | Mode$ TgtChoose | Optional$ True | NumCards$ 1 | RememberDiscarded$ True | SubAbility$ I\nSVar:I:DB$ ImmediateTrigger | ConditionDefined$ Remembered | ConditionPresent$ Card.nonLand | ConditionCompare$ EQ1 | Execute$ T | RememberObjects$ Remembered | SubAbility$ C\nSVar:T:DB$ DealDamage | NumDmg$ X | ValidTgts$ Creature\nSVar:C:DB$ Cleanup | ClearRemembered$ True\nSVar:X:TriggerRemembered$CardManaCost",
    "{T}: Draw a card, then you may discard a card. When you discard a nonland card this way, this creature deals damage equal to that card's mana value to target creature.",
    &[r#"{"Bind":{"as":"when2","what":{"Bound":"discarded"}}}"#, r#"{"Stat":{"of":{"Bound":"when2"},"stat":"ManaValue"}}"#],
    ""
)]
#[case::when_this_dies_this_turn_return_it(
    "Creature",
    "A:AB$ Pump | Cost$ 2 | NumAtt$ +1 | SubAbility$ D\nSVar:D:DB$ DelayedTrigger | Mode$ ChangesZone | RememberObjects$ Self | ValidCard$ Card.IsTriggerRemembered | Origin$ Battlefield | Destination$ Graveyard | ThisTurn$ True | Execute$ R\nSVar:R:DB$ ChangeZone | Defined$ DelayTriggerRemembered | Origin$ Graveyard | Destination$ Battlefield",
    "{2}: This creature gets +1/+0 until end of turn. When it dies this turn, return it to the battlefield.",
    &[r#"{"Move":{"to":{"Battlefield":{}},"what":"Triggering"}}"#, r#""what":[{"Is":"This"}]"#],
    ""
)]
#[case::for_each_player_a_card_from_their_graveyard(
    "Sorcery",
    "A:SP$ RepeatEach | RepeatPlayers$ Player | RepeatSubAbility$ C | SubAbility$ M\nSVar:C:DB$ ChooseCard | Defined$ You | Choices$ Creature.RememberedPlayerCtrl | ChoiceZone$ Graveyard | Mandatory$ True | Amount$ 1 | ImprintChosen$ True\nSVar:M:DB$ ChangeZone | Defined$ Imprinted | GainControl$ True | Origin$ Graveyard | Destination$ Battlefield",
    "For each player, choose a creature card in that player's graveyard. Put those cards onto the battlefield under your control.",
    &[r#"{"Bind":{"adds":true,"as":"picks3","what":{"Bound":"card2"}}}"#, r#""what":{"Bound":"picks3"}"#],
    ""
)]
#[case::each_player_keeps_three_destroy_the_rest(
    "Sorcery",
    "A:SP$ RepeatEach | RepeatPlayers$ Player | RepeatSubAbility$ C | SubAbility$ D\nSVar:C:DB$ ChooseCard | Defined$ Remembered | Choices$ Permanent.nonLand+RememberedPlayerCtrl | Amount$ 3 | Mandatory$ True | RememberChosen$ True\nSVar:D:DB$ DestroyAll | ValidCards$ Permanent.nonLand+!IsRemembered",
    "Each player chooses three nonland permanents they control. Destroy all other nonland permanents.",
    &[r#""adds":true"#, r#"{"Not":{"Is":{"Bound":"picks3"}}}"#],
    ""
)]
#[case::the_rest_of_what_opponents_chose_stays_a_gap(
    "Creature",
    "A:AB$ RepeatEach | Cost$ T | RepeatPlayers$ Player.Opponent | RepeatSubAbility$ C | SubAbility$ X\nSVar:C:DB$ ChooseCard | Defined$ Remembered | Amount$ 2 | Choices$ Card.RememberedPlayerCtrl | ChoiceZone$ Graveyard | RememberChosen$ True | Mandatory$ True\nSVar:X:DB$ ChangeZoneAll | Origin$ Graveyard | Destination$ Exile | ChangeType$ Card.!IsRemembered",
    "{T}: Each opponent chooses two cards in their graveyard and exiles the rest.",
    &[],
    "key:ChooseCard.RememberChosen-of-some-players-read-as-the-rest"
)]
#[case::a_pick_read_again_in_its_pass_stays_a_gap(
    "Sorcery",
    "A:SP$ RepeatEach | RepeatPlayers$ Player | RepeatSubAbility$ C\nSVar:C:DB$ ChooseCard | Defined$ You | Choices$ Creature.RememberedPlayerCtrl | Mandatory$ True | RememberChosen$ True | SubAbility$ S\nSVar:S:DB$ Sacrifice | Defined$ Remembered",
    "For each player, choose a creature they control and sacrifice it.",
    &[],
    "key:ChooseCard.RememberChosen-with-remembered"
)]
#[case::damage_equal_to_the_cards_revealed_this_way(
    "Sorcery",
    "A:SP$ DigUntil | Valid$ Artifact | FoundDestination$ Battlefield | RevealedDestination$ Library | RevealedLibraryPosition$ -1 | RevealRandomOrder$ True | RememberRevealed$ True | SubAbility$ D\nSVar:D:DB$ DealDamage | Defined$ You | NumDmg$ X | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Remembered$Amount",
    "Reveal cards from the top of your library until you reveal an artifact card. Put that card onto the battlefield and the rest on the bottom of your library in a random order. This spell deals damage to you equal to the number of cards revealed this way.",
    &[r#"{"Let":{"amount":{"Count":[{"Is":{"Bound":"revealed2"}},{"In":"Library"}]},"as":"seen3"}}"#, r#"{"DealDamage":{"amount":{"Bound":"seen3"},"to":"You"}}"#],
    ""
)]
#[case::life_for_each_of_the_exiled_cards(
    "Sorcery",
    "A:SP$ DigUntil | Valid$ Creature | FoundDestination$ Hand | RevealedDestination$ Exile | RememberRevealed$ True | SubAbility$ L\nSVar:L:DB$ LoseLife | LifeAmount$ X\nSVar:X:Remembered$Amount",
    "Reveal cards from the top of your library until you reveal a creature card, then put that card into your hand. Exile all other cards revealed this way, and you lose 1 life for each of the exiled cards.",
    &[r#"{"LoseLife":{"amount":{"Bound":"revealed3~moved"}}}"#],
    ""
)]
#[case::revealed_cards_with_neither_wording_stay_a_gap(
    "Sorcery",
    "A:SP$ DigUntil | Valid$ Creature | FoundDestination$ Hand | RevealedDestination$ Graveyard | RememberRevealed$ True | SubAbility$ L\nSVar:L:DB$ LoseLife | LifeAmount$ X\nSVar:X:Remembered$Amount",
    "Reveal cards until you reveal a creature card. Put it into your hand and the rest into your graveyard. You lose life.",
    &[],
    "key:DigUntil.RememberRevealed-with-a-found-destination"
)]
#[case::spells_with_the_chosen_name_cost_less(
    "Creature",
    "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ N\nSVar:N:DB$ NameCard | Defined$ You | SubAbility$ E\nSVar:E:DB$ Effect | StaticAbilities$ R\nSVar:R:Mode$ ReduceCost | ValidCard$ Card.NamedCard | Type$ Spell | Amount$ 1",
    "Whenever this creature attacks, choose a card name. Spells with the chosen name cost {1} less to cast this turn.",
    &[r#"{"ChooseValue":{"as":"name","kind":{"CardName":[]}}}"#, r#""applies_to":["Spell",{"ChosenAs":"name"}]"#],
    ""
)]
#[case::a_chosen_name_a_later_trigger_reads_stays_a_gap(
    "Enchantment",
    "A:AB$ NameCard | Cost$ 1 | Defined$ You | SubAbility$ E\nSVar:E:DB$ Effect | Triggers$ T\nSVar:T:Mode$ SpellCast | ValidCard$ Card.NamedCard | ValidActivatingPlayer$ You | Execute$ D\nSVar:D:DB$ Draw | NumCards$ 2",
    "{1}: Choose a card name. When you cast a spell with the chosen name, draw two cards.",
    &[],
    "valid:NamedCard-without-a-choice"
)]
#[case::its_activated_abilities_cant_be_activated(
    "Artifact",
    "A:AB$ Tap | Cost$ T | ValidTgts$ Creature | SubAbility$ E\nSVar:E:DB$ Effect | RememberObjects$ ParentTarget | StaticAbilities$ S | ForgetOnMoved$ Battlefield\nSVar:S:Mode$ CantBeActivated | ValidCard$ Card.IsRemembered",
    "{T}: Tap target creature. Its activated abilities can't be activated this turn.",
    &[r#"{"ActivateKind":{"kind":"Every","sources":[{"Is":{"Target":0}}]}}"#],
    ""
)]
fn g12f_memory_rows(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] expected: &[&str],
    #[case] gap: &str,
) {
    reads(types, lines, oracle, expected, gap);
}
