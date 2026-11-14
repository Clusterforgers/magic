//! Goal: "for each card in your hand and each foretold card you own in exile" (Alrund) and "for each foretold card you own in exile" (Niko Defies Destiny) count the foretold cards with the hand, and "can't be blocked by creatures that don't have a name" (Pompous Gadabout) keeps a face-down creature from blocking while a named one blocks, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};
use crate::common::{name_of, pid};

const ALRUND: &str = "Alrund, God of the Cosmos // Hakka, Whispering Raven";

/// The id of the card called `name` in a zone.
fn find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

/// Seat 0 foretells the card called `name` from its hand, paying with two Forests.
fn foretell(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    let id = find(driver, Zone::Hand(pid(0)), name);
    driver.answer(Answer::Priority(PriorityAction::Foretell(id)))?;
    driver.pay_tapping(&["Forest", "Forest"], 0)?;
    Ok(())
}

/// Lets the stack resolve: priority passes until it is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 holds `hand` and lands (four Forests, Plains, Island) in its first main phase; every seat has a library.
fn table(seats: usize, hand: &[&str], field: &[&str]) -> Scenario {
    let mut lands = vec![
        ready(forest()),
        ready(forest()),
        ready(forest()),
        ready(forest()),
        ready(plains()),
        ready(island()),
    ];
    lands.extend(field.iter().map(|name| ready(card(name).unwrap())));
    (0..seats).fold(
        Scenario::new(seats)
            .hand_named(0, hand.iter().copied())
            .battlefield(0, lands),
        |scenario, seat| scenario.library(seat, vec![forest(); 12]),
    )
}

/// Alrund's power and toughness against the cards in seat 0's hand and its foretold cards in exile.
fn alrund_counts(driver: &Driver, foretold: i32) {
    let state = driver.state();
    let alrund = find(driver, Zone::Battlefield, "Alrund, God of the Cosmos");
    let chars = characteristics(state, alrund).unwrap();
    let hand = i32::try_from(state.zone(Zone::Hand(pid(0))).len()).unwrap();
    assert_eq!(chars.power, Some(1 + hand + foretold));
    assert_eq!(chars.toughness, Some(1 + hand + foretold));
}

#[rstest]
fn alrund_counts_the_hand_and_each_foretold_card(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] foretold: i32,
) -> Result<(), ScriptError> {
    let hand = ["Behold the Multiverse", "Saw It Coming", "Forest"];
    let mut driver = Driver::from_scenario(table(seats, &hand, &[ALRUND]))?;
    alrund_counts(&driver, 0);
    for name in hand.iter().take(usize::try_from(foretold).unwrap()) {
        if *name == "Saw It Coming" {
            // The first foretelling tapped two Forests: two more pay for the second.
            let id = find(&driver, Zone::Hand(pid(0)), name);
            driver.answer(Answer::Priority(PriorityAction::Foretell(id)))?;
            pay(&mut driver)?;
        } else {
            foretell(&mut driver, name)?;
        }
    }
    alrund_counts(&driver, foretold);
    Ok(())
}

#[rstest]
fn niko_gains_two_life_for_each_foretold_card(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] foretold: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        &["Behold the Multiverse", "Niko Defies Destiny"],
        &[],
    ))?;
    if foretold == 1 {
        foretell(&mut driver, "Behold the Multiverse")?;
    }
    let before = driver.state().player(pid(0)).unwrap().life();
    driver.begin_cast("Niko Defies Destiny")?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    let after = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(after - before, 2 * foretold);
    Ok(())
}

/// A disguise creature: a 3/3 for {1}{G} that is cast face down for {3}.
fn masked_ape() -> Arc<CardDefinition> {
    let mut def = CardDefinition::new("Masked Ape")
        .with_type(CardType::Creature)
        .with_power_toughness(3, 3);
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{1}{G}").unwrap());
    def.rules = vec![Arc::new(Ability::Keyword(Keyword::Disguise(Cost::mana(
        "{G}".parse().unwrap(),
    ))))];
    Arc::new(def)
}

#[rstest]
fn a_face_down_creature_cannot_block_pompous_gadabout(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat 1 casts a face-down creature on its turn; seat 0 attacks it on the next turn of seat 0.
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .at(1, 1, Step::Main1)
            .hand(1, vec![masked_ape()])
            .battlefield(
                1,
                vec![
                    ready(forest()),
                    ready(forest()),
                    ready(forest()),
                    ready(card("Grizzly Bears").unwrap()),
                ],
            )
            .battlefield_named(0, ["Pompous Gadabout"]),
        |scenario, seat| scenario.library(seat, vec![forest(); 12]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Masked Ape")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(&mut driver, "Cast face down")?;
    }
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    settle(&mut driver)?;
    let masked = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .expect("a face-down creature");
    let turn = u32::try_from(seats).unwrap();
    driver.idle_until(turn, Step::DeclareAttackers)?;
    driver.attack(&[("Pompous Gadabout", 1)])?;
    driver.pass_until(Step::DeclareBlockers)?;
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        panic!("seat 1 declares blockers");
    };
    let can_block = |id: ObjectId| {
        options
            .blockers
            .iter()
            .find(|b| b.object == id)
            .is_some_and(|b| !b.can_block.is_empty())
    };
    let bears = find(&driver, Zone::Battlefield, "Grizzly Bears");
    assert!(can_block(bears), "a creature with a name blocks");
    assert!(!can_block(masked), "a face-down creature has no name");
    driver.block(&[("Grizzly Bears", "Pompous Gadabout")])?;
    Ok(())
}
