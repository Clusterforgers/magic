//! Goal: the cards that alone held back two precon cards each play as printed at tables of two, three and four seats: the Equipment whose creature halves the life of the player it hurts (Quietus Spike, Scytheclaw).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Plays on to the second main phase of the turn, blocking as `blocks` says.
fn through_combat(driver: &mut Driver, blocks: &[(&str, &str)]) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if driver.state().turn().step() == Step::Main2
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::DeclareBlockers(_) => {
                driver.block(blocks)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("combat".into()))
}

#[rstest]
fn the_equipped_creature_halves_the_life_of_the_player_it_hurts(
    #[values("Quietus Spike", "Scytheclaw")] equipment: &str,
    #[values(2, 3, 4)] seats: usize,
    #[values(20, 9, 6)] start: i32,
    #[values(false, true)] blocked: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(free_creature("Knight", 2, 2)),
                ready(free_creature("Squire", 1, 1)),
                attached_to(ready(card(equipment).unwrap()), 0, 0),
            ],
        )
        .battlefield(1, [ready(free_creature("Wall", 0, 9))])
        .life(1, start);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Knight", 1), ("Squire", 1)])?;
    let blocks: &[(&str, &str)] = if blocked { &[("Wall", "Knight")] } else { &[] };
    through_combat(&mut driver, blocks)?;
    // The Germ of Scytheclaw is a creature of its own; the Knight carries the +1/+1 and the Spike's deathtouch.
    let power = if equipment == "Scytheclaw" { 3 } else { 2 };
    let expected = if blocked {
        // Only the Squire hurts the player, and it carries nothing.
        start - 1
    } else {
        let hurt = start - power - 1;
        hurt - (hurt + 1) / 2
    };
    assert_eq!(life(&driver, 1), expected);
    for seat in (0..seats).filter(|seat| *seat != 1) {
        assert_eq!(life(&driver, seat), 20, "seat {seat}");
    }
    assert_replays(&driver);
    Ok(())
}

/// Passes priority until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// How many cards called `name` are in `zone`.
fn in_zone(driver: &Driver, zone: Zone, name: &str) -> usize {
    crate::common::names(driver.state(), zone)
        .iter()
        .filter(|found| found.as_str() == name)
        .count()
}

#[rstest]
fn a_god_that_dies_returns_to_its_owners_hand_at_the_next_end_step(
    #[values("The Locust God", "The Scorpion God")] god: &str,
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] killer: usize,
) -> Result<(), ScriptError> {
    // Seat 0 owns the God; seat `killer` burns it during its own turn.
    let mut scenario = Scenario::new(seats)
        .at(1, killer, Step::Main1)
        .hand(killer, [burn("Bolt", 7)]);
    let mut owned = vec![ready(card(god).unwrap())];
    if killer == 0 {
        owned.extend([ready(mountain()), ready(mountain())]);
    } else {
        scenario = scenario.battlefield(killer, [ready(mountain()), ready(mountain())]);
    }
    scenario = scenario.battlefield(0, owned);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named(god)?;
    driver.cast_at("Bolt", &[target])?;
    settle(&mut driver)?;
    // It died: it waits in the graveyard until the end step.
    assert_eq!(in_zone(&driver, Zone::Battlefield, god), 0);
    assert_eq!(in_zone(&driver, Zone::Graveyard(pid(0)), god), 1);
    assert_eq!(in_zone(&driver, Zone::Hand(pid(0)), god), 0);
    driver.pass_until(Step::End)?;
    settle(&mut driver)?;
    assert_eq!(
        in_zone(&driver, Zone::Hand(pid(0)), god),
        1,
        "back in its owner's hand"
    );
    assert_eq!(in_zone(&driver, Zone::Graveyard(pid(0)), god), 0);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn the_target_player_chooses_the_card_they_exile_from_their_graveyard(
    #[values("Relic of Progenitus", "Scrabbling Claws")] artifact: &str,
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] victim: usize,
    #[values(0, 1, 2)] cards: usize,
) -> Result<(), ScriptError> {
    let victim = victim % seats;
    let names = ["Alpha", "Beta"];
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(artifact).unwrap())]);
    if cards > 0 {
        scenario = scenario.graveyard(
            victim,
            names[..cards].iter().map(|name| free_creature(name, 1, 1)),
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation(artifact)?;
    let target = driver.target_player(victim);
    driver.choose_targets(&[target])?;
    for _ in 0..40 {
        if let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() {
            // The player whose graveyard it is chooses, never the player of the artifact (unless they are the same).
            assert_eq!(driver.decision()?.player, pid(victim));
            assert_eq!(options.from.len(), cards);
            let wanted = *options
                .from
                .iter()
                .find(|id| crate::common::name_of(driver.state(), **id) == "Beta")
                .unwrap();
            driver.answer(Answer::ChooseCards(CardsChoice(vec![wanted])))?;
        } else if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        } else {
            driver.answer_passively()?;
        }
    }
    let graveyard = |name: &str| in_zone(&driver, Zone::Graveyard(pid(victim)), name);
    let exiled = |name: &str| in_zone(&driver, Zone::Exile, name);
    match cards {
        0 => assert_eq!((exiled("Alpha"), exiled("Beta")), (0, 0)),
        // A lone card is exiled without a question.
        1 => assert_eq!((graveyard("Alpha"), exiled("Alpha")), (0, 1)),
        _ => assert_eq!(
            (
                graveyard("Alpha"),
                exiled("Alpha"),
                graveyard("Beta"),
                exiled("Beta")
            ),
            (1, 0, 0, 1)
        ),
    }
    assert_replays(&driver);
    Ok(())
}

/// The number of cards in a hand.
fn hand_size(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

#[rstest]
fn each_player_draws_the_greatest_number_of_cards_a_player_discarded(
    #[values("Windfall", "Whispering Madness", "Jace's Archivist")] source: &str,
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] richest: usize,
) -> Result<(), ScriptError> {
    // Every hand holds one card but the richest seat's, which holds five.
    let richest = richest % seats;
    let hold = |seat: usize| -> Vec<_> {
        let size = if seat == richest { 5 } else { 1 };
        (0..size).map(|_| swamp()).collect()
    };
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 0..seats {
        let mut hand = hold(seat);
        if seat == 0 && source != "Jace's Archivist" {
            hand.push(card(source).unwrap());
        }
        scenario = scenario.hand(seat, hand);
    }
    let mut table = vec![
        ready(island()),
        ready(island()),
        ready(swamp()),
        ready(swamp()),
    ];
    if source == "Jace's Archivist" {
        table.push(ready(card(source).unwrap()));
    }
    scenario = scenario.battlefield(0, table);
    let mut driver = Driver::from_scenario(scenario)?;
    if source == "Jace's Archivist" {
        driver.begin_activation(source)?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    } else {
        driver.cast(source)?;
    }
    settle(&mut driver)?;
    for seat in 0..seats {
        assert_eq!(
            hand_size(&driver, seat),
            5,
            "seat {seat} draws what the richest discarded"
        );
    }
    // The cards discarded went to their owners' graveyards: the small hands' card and the richest seat's five.
    let discarded: usize = (0..seats)
        .map(|seat| {
            driver
                .state()
                .zone(Zone::Graveyard(pid(seat)))
                .iter()
                .filter_map(|id| driver.state().object(*id))
                .filter(|object| object.card().name == "Swamp")
                .count()
        })
        .sum();
    assert_eq!(discarded, 5 + (seats - 1));
    assert_replays(&driver);
    Ok(())
}
