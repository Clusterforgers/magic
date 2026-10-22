//! Goal: what is a commander at game level, at 2 to 6 seats: a token that is a copy of a commander, and a card that enters as a copy of one, are other objects and no commander (whichever the legend rule keeps, the copy is never put into the command zone and the commander is, when it is the one that goes); a commander that is exiled and returned is a new object that keeps the casts and the commander damage it had; and the colour identity of a pair is the two together at setup.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::error::SetupError;
use mtg_engine::format::{Commander, DeckError};
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup};
use mtg_testkit::cards::{cost, island, mountain};
use mtg_testkit::commander_cards::{legend, partner_legend};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SEAT: PlayerId = PlayerId(0);

/// "Create a token that's a copy of target creature", a free instant.
fn twin() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Twin")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rule),
    )
}

/// A Commander table where seat 0 has the commander "Cmd" ({0}, `power`/`power`) in its command zone.
fn table(seats: usize, power: i32) -> Scenario {
    Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 0, &[], power, power)])
}

fn commander(driver: &Driver) -> ObjectId {
    driver.state().commanders().next().unwrap()
}

fn zone_of(driver: &Driver, id: ObjectId) -> Zone {
    driver.state().object(id).unwrap().zone()
}

/// The battlefield objects called `name`.
fn named(driver: &Driver, name: &str) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == name)
        .collect()
}

/// Casts "Cmd" from the command zone and lets it resolve.
fn cast_commander(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    driver.cast("Cmd")?;
    for _ in 0..seats {
        driver.pass()?;
    }
    assert_eq!(zone_of(driver, commander(driver)), Zone::Battlefield);
    Ok(())
}

fn asks_yes_no(driver: &Driver) -> bool {
    matches!(
        driver.decision().map(|d| &d.kind),
        Ok(DecisionKind::YesNo(_))
    )
}

/// The legend rule is asked about the commander and its copy; returns the copy.
fn the_copy(driver: &Driver, original: ObjectId) -> Result<ObjectId, ScriptError> {
    let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() else {
        panic!("{:?}", driver.decision()?.kind);
    };
    assert_eq!(options.from.len(), 2, "{options:?}");
    assert!(options.from.contains(&original));
    Ok(*options.from.iter().find(|id| **id != original).unwrap())
}

/// The copy is no commander; the legend rule keeps `keep_the_commander` or the copy. The one that goes is a commander
/// only when it is the original: its owner is asked and puts it into the command zone, with the casts it had. A copy
/// that goes, or dies later, asks nothing.
fn the_legend_rule_keeps(
    driver: &mut Driver,
    original: ObjectId,
    keep_the_commander: bool,
) -> Result<(), ScriptError> {
    let copy = the_copy(driver, original)?;
    assert!(!driver.state().is_commander(copy));
    assert_eq!(driver.state().commanders().collect::<Vec<_>>(), [original]);
    let keep = if keep_the_commander { original } else { copy };
    driver.answer(Answer::ChooseCards(CardsChoice(vec![keep])))?;
    if keep_the_commander {
        assert!(!asks_yes_no(driver), "the copy goes and nobody is asked");
        assert_eq!(named(driver, "Cmd"), [original]);
    } else {
        assert!(
            asks_yes_no(driver),
            "the commander goes and its owner is asked"
        );
        driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        assert_eq!(zone_of(driver, original), Zone::Command(SEAT));
        assert_eq!(named(driver, "Cmd"), [copy]);
        assert!(!driver.state().is_commander(copy));
        driver.manual(ManualAction::Move {
            object: copy,
            to: ManualZone::Graveyard,
        })?;
        assert!(!asks_yes_no(driver), "a copy that dies asks nothing");
    }
    assert!(driver.state().is_commander(original));
    assert_eq!(driver.state().commander_casts(original), 1);
    Ok(())
}

#[rstest]
fn a_token_that_copies_a_commander_is_no_commander(
    #[values(2, 3, 6)] seats: usize,
    #[values(true, false)] keep_the_commander: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 2).hand(0, [twin()]))?;
    cast_commander(&mut driver, seats)?;
    let original = commander(&driver);
    let target = driver.target_named("Cmd")?;
    driver.cast_at("Twin", &[target])?;
    for _ in 0..seats {
        driver.pass()?;
    }
    the_legend_rule_keeps(&mut driver, original, keep_the_commander)
}

#[rstest]
fn a_card_that_enters_as_a_copy_of_a_commander_is_no_commander(
    #[values(2, 4)] seats: usize,
    #[values(true, false)] keep_the_commander: bool,
) -> Result<(), ScriptError> {
    let scenario = table(seats, 2)
        .hand(0, [card("Clone").unwrap()])
        .battlefield(0, (0..4).map(|_| ready(island())));
    let mut driver = Driver::from_scenario(scenario)?;
    cast_commander(&mut driver, seats)?;
    let original = commander(&driver);
    driver.cast_tapping("Clone", &["Island", "Island", "Island", "Island"])?;
    for _ in 0..50 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseCards(options) if options.from == [original] => {
                driver.answer(Answer::ChooseCards(CardsChoice(vec![original])))?;
            }
            DecisionKind::ChooseCards(_) => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    the_legend_rule_keeps(&mut driver, original, keep_the_commander)
}

/// Seat 0's commander attacks seat 1 for `power`, and is then exiled and returned to the battlefield.
fn blinked_after_combat(seats: usize, power: i32) -> Result<(Driver, ObjectId), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, power))?;
    cast_commander(&mut driver, seats)?;
    let id = commander(&driver);
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::DeclareAttackers)?;
    driver.attack(&[("Cmd", 1)])?;
    driver.idle_until(turn, Step::EndCombat).ok();
    assert_eq!(
        driver.state().commander_damage(id, PlayerId(1)),
        power.cast_unsigned()
    );
    let before = driver.state().object(id).unwrap().counter();
    driver.manual(ManualAction::Move {
        object: id,
        to: ManualZone::Exile,
    })?;
    assert!(asks_yes_no(&driver), "a commander in exile is asked about");
    driver.answer(Answer::YesNo(YesNoChoice(false)))?;
    driver.manual(ManualAction::Move {
        object: id,
        to: ManualZone::Battlefield,
    })?;
    assert!(driver.state().object(id).unwrap().counter() > before);
    Ok((driver, id))
}

#[rstest]
fn a_commander_that_is_exiled_and_returned_is_a_new_object_that_keeps_its_casts_and_its_damage(
    #[values(2, 3, 6)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, id) = blinked_after_combat(seats, 3)?;
    assert_eq!(zone_of(&driver, id), Zone::Battlefield);
    assert!(driver.state().is_commander(id));
    assert_eq!(driver.state().commanders().collect::<Vec<_>>(), [id]);
    assert_eq!(driver.state().commander_casts(id), 1);
    assert_eq!(driver.state().commander_tax(id), 2);
    assert_eq!(driver.state().commander_damage(id, PlayerId(1)), 3);
    assert!(!asks_yes_no(&driver));
    Ok(())
}

#[rstest]
fn the_damage_of_a_commander_that_came_back_adds_to_what_it_dealt_before(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, id) = blinked_after_combat(seats, 4)?;
    let turn = u32::try_from(seats).unwrap() * 2 + 1;
    driver.idle_until(turn, Step::DeclareAttackers)?;
    driver.attack(&[("Cmd", 1)])?;
    driver.idle_until(turn, Step::EndCombat).ok();
    assert_eq!(driver.state().commander_damage(id, PlayerId(1)), 8);
    Ok(())
}

// The colour identity of a pair at game setup.

fn pair() -> Vec<Arc<CardDefinition>> {
    vec![
        partner_legend("Red Partner", 1, &[Color::Red], 2, 2),
        partner_legend("Blue Partner", 1, &[Color::Blue], 2, 2),
    ]
}

/// A game of `seats` seats; seat 0 leads with the pair and holds Mountains and `extra`, the others lead with "Cmd".
fn paired(seats: usize, extra: Arc<CardDefinition>) -> GameSetup {
    let mut first = vec![mountain(); 97];
    first.push(extra);
    let mut decks = vec![first];
    let mut commanders = vec![pair()];
    for _ in 1..seats {
        decks.push(vec![mountain(); 99]);
        commanders.push(vec![legend("Cmd", 1, &[Color::Red], 2, 2)]);
    }
    GameSetup::new(Arc::new(Commander), 7, decks).with_commanders(commanders)
}

fn spell(name: &str, color: Color) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[color]))
            .with_power_toughness(1, 1),
    )
}

#[rstest]
#[case::inside_the_first(Color::Red, true)]
#[case::inside_the_second(Color::Blue, true)]
#[case::outside_both(Color::Green, false)]
#[case::outside_both_too(Color::Black, false)]
fn a_card_is_judged_against_the_identity_of_both_partners(
    #[values(2, 3, 6)] seats: usize,
    #[case] color: Color,
    #[case] fits: bool,
) {
    let setup = paired(seats, spell("Extra", color));
    match setup.validate() {
        Ok(()) => {
            assert!(fits);
            Engine::new(setup).unwrap();
        }
        Err(SetupError::InvalidDeck { player, source }) => {
            assert!(!fits, "{source:?}");
            assert_eq!(player, SEAT);
            assert!(matches!(source, DeckError::OutsideColorIdentity { .. }));
        }
        Err(other) => panic!("{other:?}"),
    }
}
