//! Goal: what an Aura or a Curse gives plays as the rules say at tables of two, three and four seats: a quoted ability and a keyword of an Aura belong to the host and its controller (not the Aura's), "they" of an enchanted player is that player, and a Curse that sets sizes and drops creature types changes the enchanted player's creatures only.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use mtg_core::card::Subtype;
use mtg_core::ids::ObjectId;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, characteristics, replay};
use mtg_testkit::attach_cards::legendary_bear;
use mtg_testkit::cards::{bear, burn, island, study};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, enchanting, ready};
use rstest::rstest;

const BEAR: &str = "Grizzly Bears";

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Answers whatever is asked passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn scenario(seats: usize, battlefield: Vec<Vec<PresetPermanent>>) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for (seat, list) in battlefield.into_iter().enumerate() {
        scenario = scenario
            .battlefield(seat, list)
            .library(seat, vec![bear(), bear(), bear(), bear(), bear(), bear()]);
    }
    scenario
}

/// Every seat's permanents, with `extra` added to the seat's list.
fn seats(count: usize, extra: Vec<(usize, PresetPermanent)>) -> Vec<Vec<PresetPermanent>> {
    let mut lists: Vec<Vec<PresetPermanent>> = vec![Vec::new(); count];
    for (seat, permanent) in extra {
        lists[seat].push(permanent);
    }
    lists
}

/// Seat `caster` draws two cards with a sorcery while the Curse of Fool's Wisdom of seat `controller` is on `enchanted`:
/// the caster's life and the controller's life by seat.
#[rstest]
#[case::the_enchanted_player_draws(2, 1, 0, 0, &[-4, 4])]
#[case::the_curse_on_its_controller(2, 0, 0, 0, &[0, 0])]
#[case::an_opponent_draws_and_is_not_enchanted(3, 2, 1, 0, &[0, 0, 0])]
#[case::a_third_player_controls_it(3, 2, 0, 0, &[-4, 0, 4])]
#[case::four_seats(4, 3, 1, 1, &[0, -4, 0, 4])]
#[case::four_seats_not_the_caster(4, 3, 1, 2, &[0, 0, 0, 0])]
fn the_enchanted_player_loses_life_for_each_card_drawn_and_the_controller_gains_it(
    #[case] players: usize,
    #[case] controller: usize,
    #[case] enchanted: usize,
    #[case] caster: usize,
    #[case] change: &[i32],
) -> Result<(), ScriptError> {
    let game = || {
        let curse = enchanting(ready(card("Curse of Fool's Wisdom").unwrap()), enchanted);
        let lands = (caster, ready(island()));
        scenario(players, seats(players, vec![(controller, curse), lands]))
            .hand(caster, vec![study("Study")])
            .at(1, caster, Step::Main1)
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let before: Vec<i32> = (0..players).map(|seat| life(&driver, seat)).collect();
    driver.cast("Study")?;
    settle(&mut driver)?;
    for (seat, delta) in change.iter().enumerate() {
        assert_eq!(life(&driver, seat) - before[seat], *delta, "seat {seat}");
    }
    replays(&driver, &setup);
    Ok(())
}

/// The size of the creature and whether it is still a Bear.
fn stats(driver: &Driver, id: ObjectId) -> ((i32, i32), bool) {
    let chars = characteristics(driver.state(), id).unwrap();
    let printed = &driver.state().object(id).unwrap().card().subtypes;
    (
        (chars.power.unwrap(), chars.toughness.unwrap()),
        chars.has_subtype(printed, &Subtype("Bear".to_owned())),
    )
}

/// A 2/2 Bear with its creature type.
fn typed_bear() -> std::sync::Arc<mtg_cards::CardDefinition> {
    std::sync::Arc::new((*bear()).clone().with_subtype("Bear"))
}

/// The Curse of Conformity of seat `controller` on seat `enchanted`: that seat's nonlegendary creatures are 3/3 with no
/// creature types, its legendary creature and every other seat's creatures keep what they have.
#[rstest]
#[case::two_seats(2, 1, 0)]
#[case::the_controller_is_enchanted(2, 0, 0)]
#[case::three_seats(3, 2, 1)]
#[case::four_seats(4, 3, 2)]
fn the_enchanted_players_creatures_become_plain_three_by_threes(
    #[case] players: usize,
    #[case] controller: usize,
    #[case] enchanted: usize,
) -> Result<(), ScriptError> {
    let game = || {
        let curse = enchanting(ready(card("Curse of Conformity").unwrap()), enchanted);
        let mut lists = seats(players, vec![(controller, curse)]);
        for list in &mut lists {
            list.push(ready(typed_bear()));
            list.push(ready(legendary_bear()));
        }
        scenario(players, lists)
    };
    let setup = game().setup().unwrap();
    let driver = Driver::from_scenario(game())?;
    let mut checked = 0;
    for id in driver.state().zone(Zone::Battlefield).iter().copied() {
        let object = driver.state().object(id).unwrap();
        let name = object.card().name.clone();
        if name != BEAR && name != "Legendary Bear" {
            continue;
        }
        let (size, is_bear) = stats(&driver, id);
        let seat = object.controller().index();
        if seat == enchanted && name == BEAR {
            assert_eq!((size, is_bear), ((3, 3), false), "{name} of seat {seat}");
        } else {
            assert_eq!(size, (2, 2), "{name} of seat {seat}");
            assert_eq!(
                is_bear,
                name == BEAR,
                "{name} of seat {seat} keeps its types"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, players * 2);
    replays(&driver, &setup);
    Ok(())
}

/// The Aura of seat `aura_seat` on the Bear of seat 0, which attacks seat `defender`: the Bear is 3/3 with lifelink
/// and its controller (not the Aura's) draws for the damage it deals.
#[rstest]
#[case::the_host_controls_the_aura(2, 0, 1)]
#[case::an_opponent_controls_the_aura(2, 1, 1)]
#[case::three_seats(3, 2, 1)]
#[case::four_seats_the_last_defends(4, 3, 3)]
fn a_keyword_and_a_trigger_the_aura_gives_belong_to_the_host(
    #[case] players: usize,
    #[case] aura_seat: usize,
    #[case] defender: usize,
) -> Result<(), ScriptError> {
    let game = || {
        let aura = attached_to(ready(card("Staggering Insight").unwrap()), 0, 0);
        let mut lists = seats(players, vec![(aura_seat, aura)]);
        // The Bear is the first permanent of seat 0, whoever holds the Aura.
        lists[0].insert(0, ready(bear()));
        scenario(players, lists)
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let bear_id = driver.id_of(BEAR)?;
    assert_eq!(stats(&driver, bear_id).0, (3, 3));
    let before: Vec<(i32, usize)> = (0..players)
        .map(|seat| (life(&driver, seat), hand(&driver, seat)))
        .collect();
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(BEAR, defender)])?;
    driver.pass_until(Step::EndCombat)?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, defender), before[defender].0 - 3);
    assert_eq!(life(&driver, 0), before[0].0 + 3, "lifelink");
    assert_eq!(
        hand(&driver, 0),
        before[0].1 + 1,
        "the host's controller draws"
    );
    for (seat, (_, cards)) in before.iter().enumerate().skip(1) {
        if seat != aura_seat {
            assert_eq!(hand(&driver, seat), *cards, "seat {seat}");
        }
    }
    replays(&driver, &setup);
    Ok(())
}

/// Sticky Fingers of seat `aura_seat` on the Bear of seat 0 and a spell of seat `burner` that kills the Bear: the
/// Aura's controller draws when the host dies, and nobody else does.
#[rstest]
#[case::the_host_controls_the_aura(2, 0, 1)]
#[case::an_opponent_controls_the_aura(2, 1, 1)]
#[case::a_third_seat_controls_the_aura(3, 2, 1)]
#[case::four_seats(4, 3, 2)]
fn the_controller_of_an_aura_draws_when_its_host_dies(
    #[case] players: usize,
    #[case] aura_seat: usize,
    #[case] burner: usize,
) -> Result<(), ScriptError> {
    let game = || {
        let aura = attached_to(ready(card("Sticky Fingers").unwrap()), 0, 0);
        let mut lists = seats(players, vec![(aura_seat, aura)]);
        lists[0].insert(0, ready(bear()));
        lists[burner].push(ready(mtg_testkit::cards::mountain()));
        scenario(players, lists)
            .hand(burner, vec![burn("Spark", 2)])
            .at(1, burner, Step::Main1)
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let at = driver.target_named(BEAR)?;
    let before: Vec<usize> = (0..players).map(|seat| hand(&driver, seat)).collect();
    driver.cast_at("Spark", &[at])?;
    settle(&mut driver)?;
    assert!(driver.id_of(BEAR).is_err(), "the Bear died");
    for (seat, cards) in before.iter().enumerate() {
        let drawn = hand(&driver, seat) + usize::from(seat == burner) - cards;
        assert_eq!(drawn, usize::from(seat == aura_seat), "seat {seat}");
    }
    replays(&driver, &setup);
    Ok(())
}
