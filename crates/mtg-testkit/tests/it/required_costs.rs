//! Goal: a cost a spell must pay in addition ("As an additional cost to cast this spell, discard a card", sacrifice a creature, a land or an artifact, pay 3 life, exile a creature card from your graveyard) is part of every cast at 2, 3 and 4 seats: a spell whose caster cannot pay it is not offered, the pick is asked of the caster alone when more than one card or permanent qualifies and not when there is only one, the cost is paid before the spell is cast and stays paid when the spell is countered, a copy of the spell pays nothing, the cost adds to an alternative cost, a cost increase and the commander tax, and the mana of a permanent the cost sacrifices is not there to pay with.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{lands, pay};
use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::{Cost, CostItem};
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{CardsChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::ZoneCause;
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, free_creature, island, mountain, swamp};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ScriptError, Takes, ready};
use rstest::rstest;

fn id_in(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    *driver
        .state()
        .zone(zone)
        .iter()
        .find(|id| crate::common::name_of(driver.state(), **id) == name)
        .unwrap_or_else(|| panic!("no {name} in {zone:?}"))
}

fn in_zone(driver: &Driver, zone: Zone, name: &str) -> usize {
    names(driver.state(), zone)
        .iter()
        .filter(|found| *found == name)
        .count()
}

/// The cards the pending pick offers, by name and in order.
fn offered(driver: &Driver) -> Vec<String> {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!(
            "a pick was expected, not {}",
            driver.decision().unwrap().kind.name()
        );
    };
    options
        .from
        .iter()
        .map(|id| crate::common::name_of(driver.state(), *id))
        .collect()
}

fn pick(driver: &mut Driver, zone: Zone, name: &str) -> Result<Vec<Event>, ScriptError> {
    let id = id_in(driver, zone, name);
    driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))
}

fn replays(driver: &Driver, scenario: Scenario) {
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// The card called `name`: a basic land, or a corpus card.
fn card(name: &str) -> Arc<CardDefinition> {
    match name {
        "Forest" => forest(),
        "Island" => island(),
        "Swamp" => swamp(),
        "Mountain" => mountain(),
        other => mtg_testkit::corpus::card(other).unwrap(),
    }
}

/// Seat 0 holds `hand` and controls `lands` Mountains; every other seat has a library to draw from.
fn table(seats: usize, hand: &[&str], mountains: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, hand.iter().map(|name| card(name)))
        .battlefield(0, lands(mountains, mountain))
        .library(0, (0..8).map(|_| forest()));
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..8).map(|_| forest()));
    }
    scenario
}

fn discards(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, Event::CardDiscarded { .. }))
        .count()
}

#[rstest]
#[case::alone_in_the_hand(&["Thrill of Possibility"], false)]
#[case::with_a_card_to_give(&["Thrill of Possibility", "Forest"], true)]
#[case::with_two(&["Thrill of Possibility", "Forest", "Island"], true)]
fn a_spell_is_offered_only_when_the_hand_holds_another_card_to_discard(
    #[case] hand: &[&str],
    #[case] offered: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats, hand, 2)).unwrap();
    assert_eq!(driver.begin_cast("Thrill of Possibility").is_ok(), offered);
}

#[rstest]
fn the_pick_is_asked_of_the_caster_alone_pays_before_the_cast_and_draws_on_resolution(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, &["Thrill of Possibility", "Forest", "Island"], 2);
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Thrill of Possibility")?;
    driver.expect_decision(0, "choose-cards")?;
    // The spell is not offered for its own cost.
    assert_eq!(offered(&driver), ["Forest", "Island"]);
    let logged = driver.engine().events().len();
    let refused = driver.answer(Answer::ChooseCards(CardsChoice(vec![id_in(
        &driver,
        Zone::Battlefield,
        "Mountain",
    )])));
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(driver.engine().events().len(), logged);
    pick(&mut driver, Zone::Hand(pid(0)), "Island")?;
    assert_eq!(
        driver.engine().events().len(),
        logged,
        "nothing is logged until the spell is paid for"
    );
    let events = pay(&mut driver)?;
    let discarded = events
        .iter()
        .position(|event| matches!(event, Event::CardDiscarded { .. }))
        .expect("the card is discarded");
    let cast = events
        .iter()
        .position(|event| matches!(event, Event::SpellCast { .. }))
        .unwrap();
    assert!(
        discarded < cast,
        "the cost is paid before the spell is cast"
    );
    assert_eq!(in_zone(&driver, Zone::Graveyard(pid(0)), "Island"), 1);
    resolve_all(&mut driver, seats)?;
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), 3);
    assert_eq!(in_zone(&driver, Zone::Hand(pid(0)), "Forest"), 3);
    replays(&driver, scenario());
    Ok(())
}

/// Every seat passes, so the top of the stack resolves.
fn resolve_all(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
fn the_only_card_to_give_is_not_asked_for(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Thrill of Possibility", "Island"], 2))?;
    driver.begin_cast("Thrill of Possibility")?;
    driver.expect_decision(0, "pay-mana")?;
    let events = pay(&mut driver)?;
    assert_eq!(discards(&events), 1);
    Ok(())
}

#[rstest]
#[case::two_cards("Cathartic Reunion", 2, "{1}{R}")]
fn a_cost_of_several_cards_takes_them_all(
    #[case] spell: &str,
    #[case] count: usize,
    #[case] mana: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand = [spell, "Forest", "Island", "Swamp"];
    let mut driver = Driver::from_scenario(table(seats, &hand, 2))?;
    driver.begin_cast(spell)?;
    driver.expect_decision(0, "choose-cards")?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        unreachable!()
    };
    assert_eq!((options.min as usize, options.max as usize), (count, count));
    let cards = options.from[..count].to_vec();
    driver.answer(Answer::ChooseCards(CardsChoice(cards)))?;
    assert_eq!(driver.pay_options()?.cost, ManaCost::parse(mana).unwrap());
    let events = pay(&mut driver)?;
    assert_eq!(discards(&events), count);
    Ok(())
}

/// The opposition: seat 1 holds Counterspell with two Islands.
fn counter_table(seats: usize) -> Scenario {
    table(seats, &["Thrill of Possibility", "Forest"], 2)
        .hand_named(1, ["Counterspell"])
        .battlefield(1, lands(2, island))
}

#[rstest]
fn a_spell_that_is_countered_has_paid_its_cost(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(counter_table(seats))?;
    driver.begin_cast("Thrill of Possibility")?;
    pay(&mut driver)?;
    driver.pass()?;
    let spell = driver.state().stack()[0];
    let target = Target::Object(driver.state().object(spell).unwrap().reference());
    driver.cast_at("Counterspell", &[target])?;
    for _ in 0..(2 * seats) {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    assert_eq!(
        in_zone(&driver, Zone::Graveyard(pid(0)), "Thrill of Possibility"),
        1
    );
    assert_eq!(
        in_zone(&driver, Zone::Graveyard(pid(0)), "Forest"),
        1,
        "the card stays discarded"
    );
    assert!(
        names(driver.state(), Zone::Hand(pid(0))).is_empty(),
        "nothing was drawn"
    );
    replays(&driver, counter_table(seats));
    Ok(())
}

#[rstest]
fn a_copy_of_the_spell_pays_nothing(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = || {
        table(
            seats,
            &["Thrill of Possibility", "Twincast", "Forest", "Island"],
            0,
        )
        .battlefield(0, [lands(2, mountain), lands(4, island)].concat())
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Thrill of Possibility")?;
    pick(&mut driver, Zone::Hand(pid(0)), "Forest")?;
    pay(&mut driver)?;
    let spell = driver.state().stack()[0];
    let target = Target::Object(driver.state().object(spell).unwrap().reference());
    driver.begin_cast("Twincast")?;
    driver.choose_targets(&[target])?;
    // Twincast has no cost of its own beyond mana, and the copy it makes is not cast.
    let events = pay(&mut driver)?;
    assert_eq!(discards(&events), 0);
    let logged = driver.engine().events().len();
    resolve_all(&mut driver, seats)?;
    assert_eq!(
        discards(&driver.engine().events()[logged..]),
        0,
        "the copy pays no cost"
    );
    resolve_all(&mut driver, seats)?;
    resolve_all(&mut driver, seats)?;
    // One discard for the cast, four cards drawn for the copy and the spell.
    assert_eq!(in_zone(&driver, Zone::Graveyard(pid(0)), "Forest"), 1);
    assert_eq!(in_zone(&driver, Zone::Hand(pid(0)), "Island"), 1);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), 5);
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
fn a_cost_increase_adds_to_the_cost_and_leaves_it_in_place(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        table(seats, &["Thrill of Possibility", "Forest"], 3).battlefield(
            1,
            [ready(
                mtg_testkit::corpus::card("Thalia, Guardian of Thraben").unwrap(),
            )],
        )
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Thrill of Possibility")?;
    assert_eq!(driver.pay_options()?.cost.mana_value(), 3);
    let events = pay(&mut driver)?;
    assert_eq!(discards(&events), 1);
    // With two Mountains the total of three mana cannot be paid, so the spell is not offered.
    let mut short = Driver::from_scenario(
        table(seats, &["Thrill of Possibility", "Forest"], 2).battlefield(
            1,
            [ready(
                mtg_testkit::corpus::card("Thalia, Guardian of Thraben").unwrap(),
            )],
        ),
    )?;
    assert!(short.begin_cast("Thrill of Possibility").is_err());
    Ok(())
}

#[rstest]
fn flashback_pays_the_additional_cost_beside_its_own(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario =
        |hand: &[&str]| table(seats, hand, 4).graveyard_named(0, ["Electric Revelation"]);
    // With an empty hand the flashback is not offered: there is no card to discard.
    let mut bare = Driver::from_scenario(scenario(&[]))?;
    assert!(bare.begin_cast("Electric Revelation").is_err());
    let mut driver = Driver::from_scenario(scenario(&["Island"]))?;
    driver.begin_cast("Electric Revelation")?;
    assert_eq!(driver.pay_options()?.cost.mana_value(), 4);
    let events = pay(&mut driver)?;
    assert_eq!(discards(&events), 1);
    resolve_all(&mut driver, seats)?;
    assert_eq!(in_zone(&driver, Zone::Exile, "Electric Revelation"), 1);
    assert_eq!(in_zone(&driver, Zone::Graveyard(pid(0)), "Island"), 1);
    replays(&driver, scenario(&["Island"]));
    Ok(())
}

#[rstest]
#[case::altars_reap("Altar's Reap")]
#[case::village_rites("Village Rites")]
fn a_creature_is_sacrificed_before_the_cast_and_only_yours_are_offered(
    #[case] spell: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let creature = "Bear";
    let scenario = |bears: usize| {
        let mut battlefield = lands(2, swamp);
        battlefield.extend((0..bears).map(|_| ready(free_creature(creature, 1, 1))));
        Scenario::new(seats)
            .hand_named(0, [spell])
            .battlefield(0, battlefield)
            .battlefield(1, [ready(free_creature("Foe", 2, 2))])
            .library(0, (0..4).map(|_| forest()))
    };
    let mut none = Driver::from_scenario(scenario(0))?;
    assert!(none.begin_cast(spell).is_err(), "no creature to sacrifice");
    let mut driver = Driver::from_scenario(scenario(2))?;
    driver.begin_cast(spell)?;
    driver.expect_decision(0, "choose-cards")?;
    assert_eq!(offered(&driver), [creature, creature]);
    let first = driver.state().zone(Zone::Battlefield)[2];
    driver.answer(Answer::ChooseCards(CardsChoice(vec![first])))?;
    let events = pay(&mut driver)?;
    let sacrificed = events
        .iter()
        .position(|event| {
            matches!(
                event,
                Event::ZoneChanged { cause: ZoneCause::Sacrificed, object, .. } if *object == first
            )
        })
        .expect("the creature is sacrificed");
    let cast = events
        .iter()
        .position(|event| matches!(event, Event::SpellCast { .. }))
        .unwrap();
    assert!(sacrificed < cast);
    resolve_all(&mut driver, seats)?;
    assert_eq!(in_zone(&driver, Zone::Battlefield, creature), 1);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), 2);
    replays(&driver, scenario(2));
    Ok(())
}

#[rstest]
#[case::enough_life(20, true)]
#[case::exactly_the_life(3, true)]
#[case::one_short(2, false)]
fn paying_life_needs_the_life(
    #[case] life: i32,
    #[case] offered: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(boon_table(seats, life))?;
    // The creature spell is on the stack, so the counterspell has its target.
    driver.cast("Bear")?;
    assert_eq!(driver.begin_cast("Withering Boon").is_ok(), offered);
    Ok(())
}

/// Seat 0 holds a creature and Withering Boon ("As an additional cost to cast this spell, pay 3 life") with four Swamps.
fn boon_table(seats: usize, life: i32) -> Scenario {
    Scenario::new(seats)
        .hand(
            0,
            [
                mtg_testkit::cards::creature("Bear", 1, &[Color::Black], 2, 2),
                mtg_testkit::corpus::card("Withering Boon").unwrap(),
            ],
        )
        .battlefield(0, lands(4, swamp))
        .life(0, life)
}

#[rstest]
fn the_life_is_paid_with_the_cast_and_the_creature_spell_is_countered(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(boon_table(seats, 20))?;
    driver.cast("Bear")?;
    let bear = driver.state().stack()[0];
    let target = Target::Object(driver.state().object(bear).unwrap().reference());
    driver.begin_cast("Withering Boon")?;
    driver.choose_targets(&[target])?;
    let events = pay(&mut driver)?;
    let paid = events
        .iter()
        .position(|event| matches!(event, Event::LifePaid { amount: 3, .. }))
        .expect("3 life is paid");
    let cast = events
        .iter()
        .position(|event| matches!(event, Event::SpellCast { .. }))
        .unwrap();
    assert!(paid < cast, "the life is paid before the spell is cast");
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 17);
    resolve_all(&mut driver, seats)?;
    assert_eq!(in_zone(&driver, Zone::Graveyard(pid(0)), "Bear"), 1);
    assert_eq!(in_zone(&driver, Zone::Battlefield, "Bear"), 0);
    Ok(())
}
#[rstest]
#[case::one_of_one("Weigh Down", 1, 1, false)]
#[case::one_of_two("Weigh Down", 1, 2, true)]
#[case::two_of_two("Skaab Goliath", 2, 2, false)]
#[case::two_of_three("Skaab Goliath", 2, 3, true)]
fn creature_cards_are_exiled_from_the_graveyard_with_the_cast(
    #[case] spell: &str,
    #[case] takes: usize,
    #[case] creature_cards: usize,
    #[case] asked: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = |creature_cards: usize| {
        let mut graveyard = vec!["Serra Angel"; creature_cards];
        graveyard.push("Lightning Bolt");
        Scenario::new(seats)
            .hand_named(0, [spell])
            .graveyard_named(0, graveyard)
            .battlefield(0, [lands(8, swamp), lands(8, island)].concat())
            .battlefield(1, [ready(free_creature("Foe", 2, 2))])
    };
    let mut short = Driver::from_scenario(scenario(takes - 1))?;
    assert!(
        short.begin_cast(spell).is_err(),
        "too few creature cards in the graveyard"
    );
    let mut driver = Driver::from_scenario(scenario(creature_cards))?;
    driver.begin_cast(spell)?;
    // Only the creature cards qualify; the pick is asked only when there are more of them than the cost takes, and
    // before the targets.
    if asked {
        driver.expect_decision(0, "choose-cards")?;
        assert_eq!(offered(&driver), vec!["Serra Angel"; creature_cards]);
        let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
            unreachable!()
        };
        let cards = options.from[..takes].to_vec();
        driver.answer(Answer::ChooseCards(CardsChoice(cards)))?;
    }
    if spell == "Weigh Down" {
        let foe = driver.target_named("Foe")?;
        driver.choose_targets(&[foe])?;
    }
    let events = pay(&mut driver)?;
    let exiled = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ZoneChanged {
                    from: Zone::Graveyard(_),
                    to: Zone::Exile,
                    ..
                }
            )
        })
        .count();
    assert_eq!(exiled, takes);
    assert_eq!(
        in_zone(&driver, Zone::Graveyard(pid(0)), "Lightning Bolt"),
        1
    );
    assert_eq!(
        in_zone(&driver, Zone::Graveyard(pid(0)), "Serra Angel"),
        creature_cards - takes
    );
    Ok(())
}

/// A Commander table: seat 0 has the commander "Grim Cmd" ({1}{R}, "As an additional cost to cast this spell, discard a
/// card"), two Forests in hand and Mountains.
fn commander_table(seats: usize, mountains: usize) -> Scenario {
    let discard = Ability::AdditionalCost {
        cost: Cost(vec![CostItem::Discard {
            count: mtg_cards::value::Amount::one(),
            filter: None,
        }]),
        required: true,
    };
    let commander: Arc<CardDefinition> = Arc::new(
        (*legend("Grim Cmd", 1, &[Color::Red], 2, 2))
            .clone()
            .with_rules(discard),
    );
    Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [commander])
        .hand(0, [forest(), forest()])
        .battlefield(0, lands(mountains, mountain))
}

#[rstest]
fn the_commander_tax_adds_to_the_cost_that_stays(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(commander_table(seats, 9))?;
    driver.begin_cast("Grim Cmd")?;
    driver.expect_decision(0, "choose-cards")?;
    pick(&mut driver, Zone::Hand(pid(0)), "Forest")?;
    assert_eq!(
        driver.pay_options()?.cost,
        ManaCost::parse("{1}{R}").unwrap()
    );
    let events = pay(&mut driver)?;
    assert_eq!(discards(&events), 1);
    resolve_all(&mut driver, seats)?;
    let id = driver.state().commanders().next().unwrap();
    driver.manual(ManualAction::Move {
        object: id,
        to: ManualZone::Graveyard,
    })?;
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    // The second cast costs the tax and discards the last card of the hand, which needs no pick.
    driver.begin_cast("Grim Cmd")?;
    assert_eq!(
        driver.pay_options()?.cost,
        ManaCost::parse("{3}{R}").unwrap()
    );
    let events = pay(&mut driver)?;
    assert_eq!(discards(&events), 1);
    assert!(
        names(driver.state(), Zone::Hand(pid(0))).is_empty(),
        "both Forests were discarded"
    );
    Ok(())
}

#[rstest]
fn the_commander_is_offered_only_with_a_card_to_discard_and_the_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(commander_table(seats, 1))?;
    assert!(
        driver.begin_cast("Grim Cmd").is_err(),
        "one Mountain cannot pay {{1}}{{R}}"
    );
    let mut empty = Driver::from_scenario(commander_table(seats, 9).hand(0, Vec::new()))?;
    assert!(empty.begin_cast("Grim Cmd").is_err(), "no card to discard");
    Ok(())
}

#[test]
fn the_mana_of_the_artifact_a_cost_sacrifices_is_not_there_to_pay_with() -> Result<(), ScriptError>
{
    // Reshape costs {X}{U}{U} and sacrifices an artifact: Sol Ring cannot pay for the X it is sacrificed to.
    let table = |extra: usize| {
        let mut battlefield = lands(2 + extra, island);
        battlefield.push(ready(mtg_testkit::corpus::card("Sol Ring").unwrap()));
        Scenario::new(2)
            .hand_named(0, ["Reshape"])
            .battlefield(0, battlefield)
    };
    // No spare Island: X is 0 and not asked. One spare Island: X can be 1 and no more, since Sol Ring is sacrificed.
    let mut none = Driver::from_scenario(table(0))?;
    none.begin_cast("Reshape")?;
    assert_eq!(none.pay_options()?.cost, ManaCost::parse("{U}{U}").unwrap());
    let mut one = Driver::from_scenario(table(1))?;
    one.begin_cast("Reshape")?;
    let DecisionKind::ChooseNumber(options) = &one.decision()?.kind else {
        panic!("X is asked, not {}", one.decision()?.kind.name());
    };
    assert_eq!(options.max, Some(1));
    Ok(())
}

/// How many casts of each kind of cost the games made and checked.
#[derive(Default)]
struct Seen {
    discard: u64,
    sacrifice: u64,
    exile: u64,
    life: u64,
}

/// Whether the events right before the cast at `at` pay what the spell takes: the cards moved out of the zone the cost
/// names (the picks, as announced) or the life lost.
fn paid_before(events: &[Event], at: usize, takes: Takes, picks: &[ObjectId]) -> bool {
    let window = &events[at.saturating_sub(8)..at];
    match takes {
        Takes::Life(amount) => window
            .iter()
            .any(|event| matches!(event, Event::LifePaid { amount: paid, .. } if *paid == amount)),
        Takes::Discard(count) | Takes::Sacrifice(count) | Takes::Exile(count) => {
            picks.len() == count
                && picks.iter().all(|pick| {
                    window.iter().any(|event| match event {
                        Event::CardDiscarded { object, .. } => *object == *pick,
                        Event::ZoneChanged { object, cause, .. } if *object == *pick => matches!(
                            (takes, cause),
                            (Takes::Sacrifice(_), ZoneCause::Sacrificed)
                                | (Takes::Exile(_), ZoneCause::Exiled)
                        ),
                        _ => false,
                    })
                })
        }
    }
}

#[test]
fn bot_games_with_the_cards_that_cost_more_to_cast_pay_the_cost_on_every_cast_and_keep_every_invariant()
 {
    use crate::common::seeds::{for_each_seed, fuzz_games};
    use mtg_testkit::{PlayOptions, play_game_with, required_cost_fuzz_setup, required_cost_of};
    let seen = std::sync::Mutex::new(Seen::default());
    for_each_seed(0..fuzz_games(16), |seed| {
        for players in [2, 3, 4] {
            let setup = required_cost_fuzz_setup(players, seed);
            let outcome = play_game_with(setup, seed, PlayOptions::default())
                .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed =
                replay(&required_cost_fuzz_setup(players, seed), &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            for (at, event) in outcome.events.iter().enumerate() {
                let Event::SpellCast {
                    object, choices, ..
                } = event
                else {
                    continue;
                };
                let Some(takes) = replayed
                    .object(*object)
                    .and_then(|spell| required_cost_of(&spell.card().name))
                else {
                    continue;
                };
                let picks: Vec<ObjectId> = choices.cost_picks().iter().flatten().copied().collect();
                assert!(
                    paid_before(&outcome.events, at, takes, &picks),
                    "seed {seed}, {players} seats: the cast at event {at} paid {takes:?} with {picks:?}"
                );
                match takes {
                    Takes::Discard(_) => seen.discard += 1,
                    Takes::Sacrifice(_) => seen.sacrifice += 1,
                    Takes::Exile(_) => seen.exile += 1,
                    Takes::Life(_) => seen.life += 1,
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    assert!(seen.discard > 0, "some spell discarded a card to be cast");
    assert!(
        seen.sacrifice > 0,
        "some spell sacrificed a permanent to be cast"
    );
    assert!(
        seen.exile > 0,
        "some spell exiled cards from a graveyard to be cast"
    );
    assert!(seen.life > 0, "some spell paid 3 life to be cast");
}
