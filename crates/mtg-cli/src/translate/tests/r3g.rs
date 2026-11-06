//! Goal: `RepeatEach` over `DefinedCards$` reads the objects an earlier step named, each pass reads "its controller" as the
//! controller the object had before it moved (CR 400.7, 608.2h), and a pass whose permanent did not reach the graveyard
//! does nothing; a list the translator cannot trace stays a named gap.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::{abilities, gaps};

const RAMPAGE: &str = "Name:Rampage\nTypes:Sorcery\nManaCost:R\nA:SP$ DestroyAll | ValidCards$ Artifact,Enchantment | RememberDestroyed$ True | SubAbility$ DBRepeat\nSVar:DBRepeat:DB$ RepeatEach | UseImprinted$ True | DefinedCards$ DirectRemembered | RepeatSubAbility$ DBToken | SubAbility$ DBCleanup | ChangeZoneTable$ True\nSVar:DBToken:DB$ Token | TokenOwner$ ImprintedController | TokenScript$ w_1_1_soldier\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n";

const TERASTODON: &str = "Name:Tera\nTypes:Sorcery\nManaCost:R\nA:SP$ Destroy | TargetMin$ 0 | TargetMax$ 3 | ValidTgts$ Permanent.nonCreature | SubAbility$ MakeTokens\nSVar:MakeTokens:DB$ RepeatEach | RepeatSubAbility$ DBToken | DefinedCards$ Targeted | ChangeZoneTable$ True\nSVar:DBToken:DB$ Token | ConditionDefined$ Remembered | ConditionPresent$ Card.inRealZoneGraveyard | TokenScript$ w_1_1_soldier | TokenOwner$ RememberedController\n";

fn card(script: &str, kind: CardType) -> Arc<CardDefinition> {
    let name = script
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("Name:"))
        .unwrap()
        .to_owned();
    let mut card = CardDefinition::new(&name)
        .with_type(kind)
        .with_mana_cost(cost(0, &[Color::Red]));
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn permanent(name: &str, kind: CardType) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(kind))
}

fn settle(driver: &mut Driver) {
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle && matches!(decision.kind, DecisionKind::Priority(_)) {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

/// How many permanents named `name` seat `seat` controls.
fn controlled(driver: &Driver, name: &str, seat: usize) -> usize {
    driver
        .state()
        .objects()
        .filter(|o| o.card().name == name && o.zone() == mtg_engine::zones::Zone::Battlefield)
        .filter(|o| o.controller() == PlayerId::from_index(seat))
        .count()
}

#[test]
fn each_destroyed_permanents_controller_gets_a_token() {
    // CR 701.8, 400.7: a destroyed permanent's controller is the one it had on the battlefield, and the token is theirs.
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [card(RAMPAGE, CardType::Sorcery)])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(permanent("Aura", CardType::Enchantment)),
            ],
        )
        .battlefield(
            1,
            [
                ready(permanent("Relic", CardType::Artifact)),
                ready(permanent("Idol", CardType::Artifact)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Rampage").unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    driver.pass().unwrap();
    settle(&mut driver);
    assert_eq!(controlled(&driver, "Relic", 1), 0);
    assert_eq!(controlled(&driver, "Soldier", 0), 1);
    assert_eq!(controlled(&driver, "Soldier", 1), 2);
}

#[test]
fn only_a_permanent_that_reached_the_graveyard_makes_a_token() {
    // The indestructible permanent was targeted but not destroyed, so its pass makes nothing.
    let sturdy = Arc::new(
        CardDefinition::new("Sturdy")
            .with_type(CardType::Artifact)
            .with_rules(mtg_cards::ability::Ability::Keyword(
                mtg_cards::keyword::Keyword::Indestructible,
            )),
    );
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [card(TERASTODON, CardType::Sorcery)])
        .battlefield(0, [ready(mountain())])
        .battlefield(
            1,
            [ready(permanent("Relic", CardType::Artifact)), ready(sturdy)],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Tera").unwrap();
    let relic = driver.target_named("Relic").unwrap();
    let sturdy = driver.target_named("Sturdy").unwrap();
    driver.choose_targets(&[relic, sturdy]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    driver.pass().unwrap();
    settle(&mut driver);
    assert_eq!(controlled(&driver, "Soldier", 1), 1);
    assert_eq!(controlled(&driver, "Sturdy", 1), 1);
}

#[rstest]
#[case::the_word_is_unknown(
    "Name:T\nTypes:Sorcery\nA:SP$ Draw | SubAbility$ R\nSVar:R:DB$ RepeatEach | DefinedCards$ Sideways | RepeatSubAbility$ D\nSVar:D:DB$ Draw\n",
    "key:RepeatEach.DefinedCards=Sideways"
)]
#[case::an_order_is_chosen(
    "Name:T\nTypes:Sorcery\nA:SP$ Destroy | ValidTgts$ Creature | SubAbility$ R\nSVar:R:DB$ RepeatEach | DefinedCards$ Targeted | ChooseOrder$ True | RepeatSubAbility$ D\nSVar:D:DB$ Draw\n",
    "key:RepeatEach.ChooseOrder"
)]
fn a_list_the_translator_cannot_trace_stays_a_gap(#[case] script: &str, #[case] gap: &str) {
    assert!(
        gaps(script).iter().any(|found| found == gap),
        "{:?}",
        gaps(script)
    );
}

const SINS: &str = "Name:Sins\nTypes:Sorcery\nManaCost:R\nA:SP$ Effect | ValidTgts$ Instant.YouCtrl,Sorcery.YouCtrl | TgtZone$ Graveyard | RememberObjects$ Targeted | StaticAbilities$ Play | SubAbility$ DBEffect | ExileOnMoved$ Graveyard\nSVar:Play:Mode$ Continuous | MayPlay$ True | MayPlayWithoutManaCost$ True | Affected$ Card.IsRemembered | AffectedZone$ Graveyard\nSVar:DBEffect:DB$ Effect | RememberObjects$ Targeted | ExileOnMoved$ Stack | ReplacementEffects$ ReplaceGraveyard | SubAbility$ DBChange\nSVar:ReplaceGraveyard:Event$ Moved | ValidCard$ Card.IsRemembered | Origin$ Stack | Destination$ Graveyard | ReplaceWith$ MoveExile\nSVar:MoveExile:DB$ ChangeZone | Defined$ ReplacedCard | Origin$ Stack | Destination$ Exile\nSVar:DBChange:DB$ ChangeZone | Origin$ Stack | Destination$ Exile\n";

const PEEK: &str = "Name:Peek\nTypes:Sorcery\nManaCost:R\nA:SP$ Draw | NumCards$ 1\n";

fn zone_of(driver: &Driver, name: &str) -> mtg_engine::zones::Zone {
    driver
        .state()
        .objects()
        .find(|o| o.card().name == name)
        .unwrap()
        .zone()
}

#[test]
fn a_card_cast_with_a_remembered_permission_is_exiled_instead_of_put_into_the_graveyard() {
    // CR 614.1a, 400.7: the shield was made for the card in the graveyard and covers the spell it becomes when cast.
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [card(SINS, CardType::Sorcery)])
        .graveyard(0, [card(PEEK, CardType::Sorcery)])
        .battlefield(0, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Sins").unwrap();
    let id = driver
        .state()
        .objects()
        .find(|o| o.card().name == "Peek")
        .unwrap()
        .id();
    let peek = mtg_engine::choose::Target::Object(driver.state().object(id).unwrap().reference());
    driver.choose_targets(&[peek]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    driver.pass().unwrap();
    settle(&mut driver);
    driver.begin_cast("Peek").unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    driver.pass().unwrap();
    settle(&mut driver);
    assert_eq!(zone_of(&driver, "Peek"), mtg_engine::zones::Zone::Exile);
}

const WISH: &str = "Name:Wish\nTypes:Sorcery\nManaCost:R\nA:SP$ ChangeZone | Reveal$ True | Origin$ Sideboard | Destination$ Hand | ChangeType$ Sorcery.YouOwn | ChangeNum$ 1 | SubAbility$ DBChange | Hidden$ True\nSVar:DBChange:DB$ ChangeZone | Origin$ Stack | Destination$ Exile\n";

#[test]
fn a_card_from_outside_the_game_is_found_nowhere_in_a_game_without_a_sideboard() {
    // CR 400.10 and the owner's ruling: Commander has no sideboard, so the wish finds nothing and the game goes on.
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [card(WISH, CardType::Sorcery)])
        .graveyard(0, [card(PEEK, CardType::Sorcery)])
        .battlefield(0, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Wish").unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    driver.pass().unwrap();
    settle(&mut driver);
    assert_eq!(zone_of(&driver, "Wish"), mtg_engine::zones::Zone::Exile);
    assert_eq!(
        zone_of(&driver, "Peek"),
        mtg_engine::zones::Zone::Graveyard(PlayerId(0))
    );
}
