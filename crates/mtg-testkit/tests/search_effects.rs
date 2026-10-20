//! Goal: Rampant Growth and Cultivate play whole at tables of two, three and four seats, as their cards say: the player searches the library for basic land cards and alone is asked which of them (none is allowed), a land found goes onto the battlefield tapped under that player's control, Cultivate reveals what it finds and puts the other card into the hand, the library is shuffled from the game's random stream (the same for the same seed) and no other seat's library is touched; a library with no match or no card at all is searched without a question; a card put onto the battlefield tapped fires what triggers on entering; answers are checked by name; an engine rebuilt from the log asks the same question at each decision and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::library::{CardsFound, Revealed};
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine, EngineError, Event, GameSetup, replay};
use mtg_testkit::cards::{cost, forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0's library from the top down: four basic lands among other cards.
const LIBRARY: [&str; 7] = [
    "Grizzly Bears",
    "Island",
    "Forest",
    "Plains",
    "Gaea's Cradle",
    "Forest",
    "Serra Angel",
];
const BASIC_LANDS: [&str; 4] = ["Island", "Forest", "Plains", "Forest"];
const RAMPANT: &str = "Search your library for a basic land card.";
const CULTIVATE: &str = "Search your library for up to 2 basic land cards.";

fn pick(name: &str) -> Arc<CardDefinition> {
    match name {
        "Island" => island(),
        "Plains" => plains(),
        "Swamp" => swamp(),
        "Mountain" => mountain(),
        other => card(other).unwrap(),
    }
}

/// Seat 0 holds `spell` and controls the Forests that pay for it; every seat has `library` (top first).
fn table_seeded(
    seats: usize,
    seed: u64,
    spell: Arc<CardDefinition>,
    forests: usize,
    library: &[&str],
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .seed(seed)
        .hand(0, [spell])
        .battlefield(0, (0..forests).map(|_| ready(forest())));
    for seat in 0..seats {
        scenario = scenario.library(seat, library.iter().map(|name| pick(name)));
    }
    scenario
}

fn table(seats: usize, spell: &str, library: &[&str]) -> Scenario {
    let forests = if spell == "Cultivate" { 3 } else { 2 };
    table_seeded(seats, 1, card(spell).unwrap(), forests, library)
}

fn passes_until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

/// Casts the spell and passes until it asks something or is done.
fn resolve(driver: &mut Driver, spell: &str) -> Result<(), ScriptError> {
    driver.cast(spell)?;
    passes_until_asked(driver)
}

/// Answers the pending question and passes on until the next one; returns the events from the answer on.
fn answer(driver: &mut Driver, answer: Answer) -> Result<Vec<Event>, ScriptError> {
    let from = driver.engine().events().len();
    driver.answer(answer)?;
    passes_until_asked(driver)?;
    Ok(driver.engine().events()[from..].to_vec())
}

fn cards(ids: &[ObjectId]) -> Answer {
    Answer::ChooseCards(CardsChoice(ids.to_vec()))
}

/// The pending choice of cards: the seat, the cards offered, the fewest and most to take and the words.
fn offered(driver: &Driver) -> (usize, Vec<ObjectId>, u32, u32, String) {
    let decision = driver.decision().unwrap();
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("a choice of cards is asked, not {}", decision.kind.name());
    };
    (
        decision.player.index(),
        options.from.clone(),
        options.min,
        options.max,
        options.text.clone(),
    )
}

/// The library of a seat from the top down, as names.
fn library(driver: &Driver, seat: usize) -> Vec<String> {
    let mut library = names(driver.state(), Zone::Library(pid(seat)));
    library.reverse();
    library
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

fn owned(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

/// `list` without one card called `name`.
fn without(list: &[&str], name: &str) -> Vec<String> {
    let mut rest = owned(list);
    rest.remove(rest.iter().position(|n| n == name).unwrap());
    rest
}

/// What happened in a resolution, as short words.
fn story(events: &[Event]) -> Vec<&'static str> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ChoiceMade { .. } => Some("chose"),
            Event::CardsFound(_) => Some("found"),
            Event::Revealed(_) => Some("revealed"),
            Event::ZoneChanged {
                to: Zone::Battlefield,
                ..
            } => Some("entered"),
            Event::Entered { tapped: true, .. } => Some("tapped"),
            Event::ZoneChanged {
                to: Zone::Hand(_), ..
            } => Some("to hand"),
            Event::LibraryShuffled { .. } => Some("shuffled"),
            Event::ResolutionEnded {
                outcome: ResolutionOutcome::Resolved,
                ..
            } => Some("resolved"),
            _ => None,
        })
        .collect()
}

fn shuffles(events: &[Event]) -> Vec<usize> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::LibraryShuffled { player, .. } => Some(player.index()),
            _ => None,
        })
        .collect()
}

fn permanent(driver: &Driver, id: ObjectId) -> (Zone, usize, Option<bool>) {
    let object = driver.state().object(id).unwrap();
    (
        object.zone(),
        object.controller().index(),
        object
            .permanent()
            .map(mtg_engine::object::PermanentState::tapped),
    )
}

#[rstest]
fn rampant_growth_puts_the_basic_land_chosen_onto_the_battlefield_tapped_and_shuffles(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] which: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Rampant Growth", &LIBRARY))?;
    let others: Vec<Vec<String>> = (1..seats).map(|seat| library(&driver, seat)).collect();
    resolve(&mut driver, "Rampant Growth")?;
    let (seat, from, min, max, text) = offered(&driver);
    assert_eq!((seat, min, max, text.as_str()), (0, 0, 1, RAMPANT));
    let lands: Vec<String> = from.iter().map(|id| name_of(driver.state(), *id)).collect();
    assert_eq!(
        sorted(lands),
        sorted(owned(&BASIC_LANDS)),
        "only the basic lands are offered"
    );
    let land = from[which];
    let wanted = name_of(driver.state(), land);
    let events = answer(&mut driver, cards(&[land]))?;
    assert_eq!(
        story(&events),
        [
            "chose", "found", "entered", "tapped", "shuffled", "resolved"
        ]
    );
    assert_eq!(
        shuffles(&events),
        [0],
        "only the searcher's library is shuffled"
    );
    let found = CardsFound {
        player: pid(0),
        zone: Zone::Library(pid(0)),
        cards: vec![land],
    };
    assert!(events.contains(&Event::CardsFound(found)));
    assert!(
        !events.iter().any(|e| matches!(e, Event::Revealed(_))),
        "nothing is revealed"
    );
    assert_eq!(permanent(&driver, land), (Zone::Battlefield, 0, Some(true)));
    assert_eq!(
        sorted(library(&driver, 0)),
        sorted(without(&LIBRARY, &wanted))
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Rampant Growth"]
    );
    for (index, before) in others.iter().enumerate() {
        assert_eq!(
            &library(&driver, index + 1),
            before,
            "seat {} is untouched",
            index + 1
        );
    }
    Ok(())
}

#[rstest]
fn a_player_may_find_nothing_and_the_library_is_shuffled_all_the_same(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Rampant Growth", &LIBRARY))?;
    resolve(&mut driver, "Rampant Growth")?;
    let events = answer(&mut driver, cards(&[]))?;
    assert_eq!(story(&events), ["chose", "found", "shuffled", "resolved"]);
    assert_eq!(sorted(library(&driver, 0)), sorted(owned(&LIBRARY)));
    assert_eq!(battlefield_lands(&driver), 2, "no land came in");
    Ok(())
}

fn battlefield_lands(driver: &Driver) -> usize {
    driver.state().zone(Zone::Battlefield).len()
}

#[rstest]
fn a_library_without_a_match_or_without_cards_is_searched_without_a_question(
    #[values(2, 3, 4)] seats: usize,
    #[values(&["Grizzly Bears", "Gaea's Cradle", "Serra Angel"][..], &[][..])] library: &[&str],
    #[values("Rampant Growth", "Cultivate")] spell: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, spell, library))?;
    let from = driver.engine().events().len();
    resolve(&mut driver, spell)?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    let events = driver.engine().events()[from..].to_vec();
    assert_eq!(story(&events), ["found", "shuffled", "resolved"]);
    let found = events.iter().find_map(|event| match event {
        Event::CardsFound(found) => Some(found.clone()),
        _ => None,
    });
    assert_eq!(
        found.map(|f| f.cards),
        Some(Vec::new()),
        "the search happened and found nothing"
    );
    assert_eq!(sorted(library_names(&driver)), sorted(owned(library)));
    Ok(())
}

fn library_names(driver: &Driver) -> Vec<String> {
    library(driver, 0)
}

#[rstest]
fn cultivate_reveals_the_lands_it_finds_and_puts_one_onto_the_battlefield_tapped_and_one_into_the_hand(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] to_the_field: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Cultivate", &LIBRARY))?;
    resolve(&mut driver, "Cultivate")?;
    let (seat, from, min, max, text) = offered(&driver);
    assert_eq!((seat, min, max, text.as_str()), (0, 0, 2, CULTIVATE));
    let found = [from[1], from[3]];
    let first = answer(&mut driver, cards(&found))?;
    assert_eq!(story(&first), ["chose", "found", "revealed"]);
    assert!(first.contains(&Event::Revealed(Revealed {
        player: pid(0),
        cards: found.to_vec()
    })));
    // Then the player says which one goes onto the battlefield.
    let (seat, among, min, max, _) = offered(&driver);
    assert_eq!((seat, among.as_slice(), min, max), (0, &found[..], 1, 1));
    let (field, hand) = (found[to_the_field], found[1 - to_the_field]);
    let second = answer(&mut driver, cards(&[field]))?;
    assert_eq!(
        story(&second),
        [
            "chose", "entered", "tapped", "to hand", "shuffled", "resolved"
        ]
    );
    assert_eq!(
        permanent(&driver, field),
        (Zone::Battlefield, 0, Some(true))
    );
    assert_eq!(permanent(&driver, hand).0, Zone::Hand(pid(0)));
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))), [hand]);
    assert_eq!(library(&driver, 0).len(), LIBRARY.len() - 2);
    Ok(())
}

#[rstest]
fn cultivate_with_one_land_found_asks_no_more_and_with_none_does_nothing_but_shuffle(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] found: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Cultivate", &LIBRARY))?;
    resolve(&mut driver, "Cultivate")?;
    let land = offered(&driver).1[0];
    let chosen: Vec<ObjectId> = [land].into_iter().take(found).collect();
    let events = answer(&mut driver, cards(&chosen))?;
    if found == 1 {
        assert_eq!(
            story(&events),
            [
                "chose", "found", "revealed", "entered", "tapped", "shuffled", "resolved"
            ]
        );
        assert_eq!(permanent(&driver, land), (Zone::Battlefield, 0, Some(true)));
    } else {
        assert_eq!(story(&events), ["chose", "found", "shuffled", "resolved"]);
    }
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))), []);
    Ok(())
}

#[test]
fn the_shuffle_after_a_search_comes_from_the_seed_and_only_from_it() -> Result<(), ScriptError> {
    let play = |seed: u64| -> Result<(Vec<String>, Vec<Event>), ScriptError> {
        let scenario = table_seeded(3, seed, card("Rampant Growth").unwrap(), 2, &LIBRARY);
        let mut driver = Driver::from_scenario(scenario)?;
        resolve(&mut driver, "Rampant Growth")?;
        let land = offered(&driver).1[0];
        answer(&mut driver, cards(&[land]))?;
        Ok((library(&driver, 0), driver.engine().events().to_vec()))
    };
    let mut orders = Vec::new();
    for seed in 1..=6 {
        let (order, log) = play(seed)?;
        assert_eq!(play(seed)?, (order.clone(), log), "seed {seed} repeats");
        orders.push(order);
    }
    orders.sort();
    orders.dedup();
    assert!(orders.len() > 1, "other seeds shuffle another way");
    Ok(())
}

/// A sorcery for `{1}` that searches for a creature card, puts it onto the battlefield tapped and shuffles.
fn tutor() -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(
        r#"{"Spell":{"effects":[
            {"Search":{"filter":[{"Type":"Creature"}],"as":"found"}},
            {"Move":{"what":{"Bound":"found"},"to":{"Battlefield":{"tapped":true}}}},
            {"Shuffle":{}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Tutor")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(1, &[]))
            .with_rules(spell),
    )
}

/// A 1/1 that gains its controller 3 life when it enters the battlefield.
fn healer() -> Arc<CardDefinition> {
    let trigger: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},
            "effects":[{"GainLife":{"amount":3}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Healer")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(trigger),
    )
}

#[rstest]
fn a_creature_put_onto_the_battlefield_tapped_triggers_what_watches_it_enter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, [tutor()])
        .battlefield(0, [ready(forest())]);
    scenario = scenario.library(0, [healer(), pick("Island")]);
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    resolve(&mut driver, "Tutor")?;
    let healer = offered(&driver).1[0];
    answer(&mut driver, cards(&[healer]))?;
    assert_eq!(
        permanent(&driver, healer),
        (Zone::Battlefield, 0, Some(true))
    );
    let triggered = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::AbilityTriggered(_)))
        .count();
    assert_eq!(triggered, 1, "what watches a creature enter triggers once");
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life + 3);
    Ok(())
}

fn refused(driver: &mut Driver, answer: Answer, expected: IllegalAnswer) {
    let (hash, pending) = (
        driver.engine().state_hash(),
        driver.decision().unwrap().clone(),
    );
    let log = driver.engine().events().len();
    let error = driver.answer(answer).unwrap_err();
    assert_eq!(error, ScriptError::Engine(EngineError::Illegal(expected)));
    assert_eq!(
        driver.engine().state_hash(),
        hash,
        "a refusal changes nothing"
    );
    assert_eq!(driver.engine().events().len(), log);
    assert_eq!(
        driver.decision().unwrap(),
        &pending,
        "the same question stands"
    );
}

#[test]
fn answers_that_are_not_among_the_lands_offered_are_refused_by_name() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, "Rampant Growth", &LIBRARY))?;
    resolve(&mut driver, "Rampant Growth")?;
    let from = offered(&driver).1;
    let bears = driver
        .state()
        .zone(Zone::Library(pid(0)))
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == "Grizzly Bears")
        .unwrap();
    refused(
        &mut driver,
        cards(&[bears]),
        IllegalAnswer::NotOffered("card"),
    );
    refused(
        &mut driver,
        cards(&[from[0], from[1]]),
        IllegalAnswer::WrongCount {
            expected: 1,
            got: 2,
        },
    );
    refused(
        &mut driver,
        cards(&[from[0], from[0]]),
        IllegalAnswer::Duplicate("card"),
    );
    refused(
        &mut driver,
        Answer::YesNo(mtg_engine::choose::YesNoChoice(true)),
        IllegalAnswer::WrongKind {
            expected: "choose-cards",
            got: "yes-no",
        },
    );
    Ok(())
}

/// What a player of these spells answers: cast the spell, pay, and find as many cards as may be found.
fn answer_for(decision: &Decision) -> Answer {
    match &decision.kind {
        DecisionKind::Priority(options) => Answer::Priority(match options.castable.first() {
            Some(card) if decision.player == pid(0) => PriorityAction::CastSpell(*card),
            _ => PriorityAction::Pass,
        }),
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        DecisionKind::ChooseCards(options) => {
            cards(&options.from[..(options.max as usize).min(options.from.len())])
        }
        other => panic!("the script does not expect {}", other.name()),
    }
}

fn plays() -> Vec<(&'static str, Scenario)> {
    vec![
        ("Rampant Growth", table(3, "Rampant Growth", &LIBRARY)),
        ("Cultivate", table(3, "Cultivate", &LIBRARY)),
        (
            "Tutor",
            Scenario::new(3)
                .hand(0, [tutor()])
                .battlefield(0, [ready(forest())])
                .library(0, [healer(), pick("Island")]),
        ),
    ]
}

fn finished(driver: &Driver, spell: &str) -> bool {
    driver.state().stack().is_empty()
        && driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .any(|id| name_of(driver.state(), *id) == spell)
}

#[test]
fn an_engine_rebuilt_from_the_log_asks_the_same_question_at_every_decision()
-> Result<(), ScriptError> {
    for (spell, scenario) in plays() {
        let setup: GameSetup = scenario.clone().setup()?;
        let mut driver = Driver::from_scenario(scenario)?;
        let mut searches = 0;
        for _ in 0..200 {
            let decision = driver.decision()?.clone();
            let mut resumed = Engine::resume(setup.clone(), driver.engine().events().to_vec())?;
            assert_eq!(
                resumed.state_hash(),
                driver.engine().state_hash(),
                "{spell}"
            );
            let mid_cast = matches!(&decision.kind, DecisionKind::PayMana(_));
            if !mid_cast {
                assert_eq!(resumed.decision(), Some(&decision), "{spell}");
            }
            searches += usize::from(matches!(decision.kind, DecisionKind::ChooseCards(_)));
            let answer = answer_for(&decision);
            let live = driver.answer(answer.clone())?;
            if !mid_cast {
                assert_eq!(resumed.apply(answer)?, live, "{spell}");
                assert_eq!(
                    resumed.state_hash(),
                    driver.engine().state_hash(),
                    "{spell}"
                );
            }
            if finished(&driver, spell) {
                break;
            }
        }
        assert!(finished(&driver, spell), "{spell} was played whole");
        assert!(searches > 0, "{spell} asked for cards");
    }
    Ok(())
}

#[test]
fn the_log_replays_to_the_state_it_was_played_to_at_every_decision() -> Result<(), ScriptError> {
    for (spell, scenario) in plays() {
        let setup = scenario.clone().setup()?;
        let mut driver = Driver::from_scenario(scenario)?;
        let mut points = Vec::new();
        for _ in 0..200 {
            points.push((driver.engine().events().len(), driver.engine().state_hash()));
            let answer = answer_for(driver.decision()?);
            driver.answer(answer)?;
            if finished(&driver, spell) {
                break;
            }
        }
        points.push((driver.engine().events().len(), driver.engine().state_hash()));
        let log = driver.engine().events().to_vec();
        assert!(points.len() > 4, "{spell}");
        for (length, hash) in points {
            assert_eq!(
                replay(&setup, &log[..length])?.state_hash(),
                hash,
                "{spell} after {length} events"
            );
        }
    }
    Ok(())
}
