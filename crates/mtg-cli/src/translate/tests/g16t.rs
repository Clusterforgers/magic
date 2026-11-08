//! Goal: round 16 pack g16t reads what a card remembers and what the turn recorded: the same name as the source, the
//! cards exiled with it (counted, totalled, played), the object a reflexive trigger sacrificed, the player of a pass over
//! players, a remembered creature a card becomes a copy of, the next spell of a kind, the creature that crewed the
//! source, the top of a graveyard and revolt; and keeps a near miss a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a script, as compact JSON, and its gaps.
fn read(script: &str) -> (String, Vec<String>) {
    let text = format!("{script}\nOracle:Subject.\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::same_name_in_a_group(
    "Name:Subject\nTypes:Creature Elf\nPT:2/2\nA:AB$ PumpAll | Cost$ R | NumAtt$ +3 | NumDef$ +3 | ValidCards$ Creature.sameName",
    r#"{"SharesName":"This"}"#
)]
#[case::same_name_as_a_target(
    "Name:Subject\nTypes:Creature Elf\nPT:2/2\nA:AB$ Destroy | Cost$ U B T | ValidTgts$ Creature.sameName",
    r#"{"SharesName":"This"}"#
)]
#[case::exiled_with_the_source_played(
    "Name:Subject\nTypes:Artifact\nA:AB$ Effect | Cost$ T | StaticAbilities$ MayCast\nSVar:MayCast:Mode$ Continuous | Affected$ Creature.ExiledWithEffectSource+nonLand | MayPlay$ True | AffectedZone$ Exile | MayPlayLimit$ 1",
    r#"{"ExiledWith":"This"}"#
)]
#[case::the_creature_that_crewed_it(
    "Name:Subject\nTypes:Artifact Vehicle\nPT:3/3\nT:Mode$ Attacks | ValidCard$ Card.Self | Execute$ Back | TriggerZones$ Battlefield\nSVar:Back:DB$ ChangeZone | ValidTgts$ Creature.CrewedThisTurn | TargetMin$ 0 | TargetMax$ 1 | Origin$ Battlefield | Destination$ Hand",
    r#"{"Crewed":"This"}"#
)]
#[case::top_of_the_graveyard_as_a_cost(
    "Name:Subject\nTypes:Enchantment\nA:AB$ Pump | Cost$ 1 ExileFromGrave<1/Card.TopGraveyard> | ValidTgts$ Creature | NumAtt$ +1 | NumDef$ +1",
    r#"{"ExileFromGraveyard":{"filter":["TopOfGraveyard"]}}"#
)]
fn a_word_of_the_source_reads_as_its_predicate(#[case] script: &str, #[case] expected: &str) {
    let (found, open) = read(script);
    assert!(found.contains(expected), "{found}");
    assert!(open.is_empty(), "{open:?}");
}

#[rstest]
#[case::count(
    "ExiledWith$Amount",
    r#""Count":[{"ExiledWith":"This"},{"In":"Exile"}]"#
)]
#[case::total_power(
    "ExiledWith$CardPower",
    r#""Total":{"of":[{"ExiledWith":"This"},{"In":"Exile"}],"stat":"Power"}"#
)]
#[case::total_mana_value(
    "ExiledWith$CardManaCost",
    r#""Total":{"of":[{"ExiledWith":"This"},{"In":"Exile"}],"stat":"ManaValue"}"#
)]
#[case::different_mana_values(
    "ExiledWith$DifferentCardManaCost",
    r#""Distinct":{"of":[{"ExiledWith":"This"},{"In":"Exile"}],"stat":"ManaValue"}"#
)]
fn the_cards_exiled_with_the_source_are_counted_and_totalled(
    #[case] word: &str,
    #[case] expected: &str,
) {
    let script = format!(
        "Name:Subject\nTypes:Artifact\nA:AB$ GainLife | Cost$ T | LifeAmount$ X\nSVar:X:{word}"
    );
    let (found, open) = read(&script);
    assert!(found.contains(expected), "{found}");
    assert!(open.is_empty(), "{open:?}");
}

#[rstest]
#[case::sacrificed_power(
    "SVar:Hit:DB$ DealDamage | ValidTgts$ Any | NumDmg$ X\nSVar:X:TriggerRemembered$CardPower",
    "RememberObjects$ Sacrificed | "
)]
#[case::spawner_sacrificed(
    "SVar:Hit:DB$ DealDamage | ValidTgts$ Any | NumDmg$ X\nSVar:X:Spawner>Sacrificed$CardPower",
    ""
)]
fn a_reflexive_trigger_reads_the_creature_that_paid_for_it(#[case] hit: &str, #[case] key: &str) {
    let script = format!(
        "Name:Subject\nTypes:Creature Dragon\nPT:2/2\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Pay\nSVar:Pay:AB$ ImmediateTrigger | Cost$ Sac<1/Creature.Other/another creature> | {key}Execute$ Hit\n{hit}"
    );
    let (found, open) = read(&script);
    assert!(
        found.contains(r#"{"Bind":{"as":"when","what":{"Bound":"sacrificed"#),
        "{found}"
    );
    assert!(found.contains(r#""stat":"Power""#), "{found}");
    assert!(open.is_empty(), "{open:?}");
}

#[rstest]
#[case::remembered_target(
    "A:AB$ ChangeZone | Cost$ 1 U B | Origin$ Graveyard | Destination$ Exile | ValidTgts$ Creature | RememberTargets$ True | SubAbility$ Copy\nSVar:Copy:DB$ Clone | Defined$ Remembered | GainThisAbility$ True",
    r#"{"BecomeCopy":{"duration":"Forever","except":["ThisAbility"],"of":{"Bound":"exiled"},"what":"This"}}"#
)]
#[case::found_by_a_dig(
    "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ Dig\nSVar:Dig:DB$ DigUntil | ValidTgts$ Opponent | Valid$ Creature | RevealedDestination$ Library | RevealedLibraryPosition$ -1 | RememberFound$ True | SubAbility$ Copy\nSVar:Copy:DB$ Clone | Defined$ Remembered | CloneTarget$ Self | Duration$ UntilEndOfTurn",
    r#"{"BecomeCopy":{"duration":"UntilEndOfTurn","of":{"Bound":"found"},"what":"This"}}"#
)]
fn a_card_becomes_a_copy_of_what_it_remembered(#[case] line: &str, #[case] expected: &str) {
    let script = format!("Name:Subject\nTypes:Creature Shapeshifter\nPT:1/1\n{line}");
    let (found, open) = read(&script);
    assert!(found.contains(expected), "{found}");
    assert!(open.is_empty(), "{open:?}");
}

#[rstest]
#[case::nonartifact_spell_of_yours(
    "ForgetOnCast$ Card.nonCreature+YouCtrl",
    "Mode$ CastWithFlash | ValidCard$ Card.nonCreature | ValidSA$ Spell | Caster$ You",
    r#""what":[{"Not":{"Type":"Creature"}}]"#
)]
#[case::creature_spell_of_yours_free(
    "ForgetOnCast$ Creature.Red+YouCtrl,Creature.Green+YouCtrl",
    "Mode$ Continuous | Affected$ Creature.Red,Creature.Green | ValidSA$ Spell | MayPlay$ True | MayPlayWithoutManaCost$ True | MayPlayDontGrantZonePermissions$ True | AffectedZone$ Hand,Graveyard,Library,Exile,Command",
    r#""benefit":"Free""#
)]
fn the_next_spell_of_a_kind_you_cast_gets_the_benefit(
    #[case] forget: &str,
    #[case] rule: &str,
    #[case] expected: &str,
) {
    let script = format!(
        "Name:Subject\nTypes:Sorcery\nA:SP$ Effect | StaticAbilities$ Grant | {forget}\nSVar:Grant:{rule}"
    );
    let (found, open) = read(&script);
    assert!(found.contains("NextSpell"), "{found}");
    assert!(found.contains(expected), "{found}");
    assert!(open.is_empty(), "{open:?}");
}

#[rstest]
#[case::spell_of_another_player(
    "ForgetOnCast$ Card.nonCreature",
    "key:Effect.ForgetOnCast-not-your-spells"
)]
#[case::a_zone_the_grant_narrows(
    "ForgetOnCast$ Creature.Red+YouCtrl",
    "static:MayPlay-next-spell-shape"
)]
fn a_grant_that_is_not_the_casters_stays_a_gap(#[case] forget: &str, #[case] gap: &str) {
    let rule = if gap.starts_with("static") {
        "Mode$ Continuous | Affected$ Creature.Red | ValidSA$ Spell | MayPlay$ True | MayPlayWithoutManaCost$ True | AffectedZone$ Hand"
    } else {
        "Mode$ CastWithFlash | ValidCard$ Card.nonCreature | ValidSA$ Spell | Caster$ You"
    };
    let script = format!(
        "Name:Subject\nTypes:Sorcery\nA:SP$ Effect | StaticAbilities$ Grant | {forget}\nSVar:Grant:{rule}"
    );
    let (_, open) = read(&script);
    assert_eq!(open, vec![gap.to_owned()]);
}

#[rstest]
#[case::pump(
    "A:SP$ Pump | ValidTgts$ Creature.YouDontCtrl | NumAtt$ -2 | RememberPumped$ True | SubAbility$ Must\nSVar:Must:DB$ Effect | StaticAbilities$ MustAttack | RememberObjects$ Remembered | ForgetOnMoved$ Battlefield\nSVar:MustAttack:Mode$ MustAttack | ValidCreature$ Card.IsRemembered",
    r#"{"Rule":{"duration":"UntilEndOfTurn","rule":{"MustAttack":{"Target":0}}}}"#
)]
#[case::goad(
    "T:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | Execute$ Goad\nSVar:Goad:DB$ Goad | Defined$ Valid Creature.OppCtrl | RememberGoaded$ True | SubAbility$ Grow\nSVar:Grow:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ X\nSVar:X:Remembered$Amount",
    r#""PutCounters""#
)]
fn a_verb_that_cannot_fail_remembers_what_it_acted_on(#[case] line: &str, #[case] expected: &str) {
    let script = format!("Name:Subject\nTypes:Creature Elf\nPT:2/2\n{line}");
    let (found, open) = read(&script);
    assert!(found.contains(expected), "{found}");
    assert!(open.is_empty(), "{open:?}");
}

#[test]
fn a_cast_card_leaves_the_remembered_cards_and_the_rest_go_to_the_bottom() {
    let script = "Name:Subject\nTypes:Creature Elf\nPT:2/2\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | Execute$ Reveal\nSVar:Reveal:DB$ PeekAndReveal | PeekAmount$ 1 | RevealValid$ Card | RememberRevealed$ True | SubAbility$ Cast\nSVar:Cast:DB$ Play | Defined$ Remembered | ValidSA$ Spell | ForgetRemembered$ True | WithoutManaCost$ True | Optional$ True | SubAbility$ Back\nSVar:Back:DB$ ChangeZone | Defined$ Remembered | Origin$ Library | Destination$ Library | LibraryPosition$ -1 | Shuffle$ False | ForgetChanged$ True";
    let (found, open) = read(script);
    assert!(found.contains("CastFromZone"), "{found}");
    assert!(found.contains(r#""place":"Bottom""#), "{found}");
    assert!(open.is_empty(), "{open:?}");
}

#[test]
fn each_opponent_picks_among_the_creatures_they_control() {
    let script = "Name:Subject\nTypes:Sorcery\nA:SP$ RepeatEach | RepeatPlayers$ Player.Opponent | RepeatSubAbility$ Pick | SubAbility$ Sac\nSVar:Pick:DB$ ChooseCard | Defined$ Player.IsRemembered | Choices$ Creature.greatestPowerControlledByRemembered | Mandatory$ True | RememberChosen$ True\nSVar:Sac:DB$ SacrificeAll | ValidCards$ Card.IsRemembered | SubAbility$ Clean\nSVar:Clean:DB$ Cleanup | ClearRemembered$ True";
    let (found, open) = read(script);
    assert!(
        found.contains(r#"{"Type":"Creature"},{"ControlledBy":{"Bound":"each"}},{"GreatestAmong""#),
        "{found}"
    );
    assert!(open.is_empty(), "{open:?}");
}

#[test]
fn a_dig_that_remembers_the_found_card_and_all_revealed_moves_them_together() {
    let script = "Name:Subject\nTypes:Artifact\nA:AB$ DigUntil | Cost$ T | ValidTgts$ Opponent | Valid$ Instant,Sorcery | FoundDestination$ Exile | RevealedDestination$ Exile | RememberFound$ True | RememberRevealed$ True | SubAbility$ Play\nSVar:Play:DB$ Play | Defined$ Remembered | ValidSA$ Spell.Instant,Spell.Sorcery | WithoutManaCost$ True | Optional$ True | ForgetPlayed$ True | SubAbility$ Rest\nSVar:Rest:DB$ ChangeZoneAll | ChangeType$ Card.IsRemembered | Origin$ Exile | Destination$ Library | LibraryPosition$ -1 | RandomOrder$ True | SubAbility$ Clean\nSVar:Clean:DB$ Cleanup | ClearRemembered$ True";
    let (found, open) = read(script);
    assert!(open.is_empty(), "{open:?}");
    // The found card is among the revealed ones: it moves once, with them.
    assert_eq!(found.matches(r#""Exile":"#).count(), 1, "{found}");
    assert!(found.contains(r#""order":"Random""#), "{found}");
    assert!(open.is_empty(), "{open:?}");
}

#[rstest]
#[case::revolt_amount("Count$Revolt.4.2")]
fn revolt_picks_between_two_amounts(#[case] word: &str) {
    let script = format!(
        "Name:Subject\nTypes:Instant\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ X\nSVar:X:{word}"
    );
    let (found, open) = read(&script);
    assert!(
        found.contains(r#""Tallied":{"what":"PermanentsLeftUnder","who":"You"}"#),
        "{found}"
    );
    assert!(found.contains(r#""otherwise":2"#) && found.contains(r#""then":4"#));
    assert!(open.is_empty(), "{open:?}");
}

#[test]
fn a_card_returned_by_its_direct_memory_is_the_one_the_step_remembered() {
    let script = "Name:Subject\nTypes:Creature Human\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Away\nSVar:Away:DB$ ChangeZone | Origin$ Battlefield | Destination$ Exile | RememberChanged$ True | ValidTgts$ Creature.Other\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Any | ValidCard$ Card.Self | Execute$ Back\nSVar:Back:DB$ ChangeZone | Origin$ Exile | Destination$ Battlefield | Defined$ DirectRemembered";
    let (found, open) = read(script);
    assert!(
        found.contains(r#"{"Move":{"to":{"Battlefield":{}},"what":{"Remembered""#),
        "{found}"
    );
    assert!(open.is_empty(), "{open:?}");
}
