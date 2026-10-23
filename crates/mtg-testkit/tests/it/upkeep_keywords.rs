//! Goal: echo, cumulative upkeep, undying and persist play whole at tables of two, three and four seats as their rules say: echo asks its controller for the cost in the upkeep after the permanent came under their control (and again for whoever controls it after a change of control, never for a permanent held since the turn began), sacrifices it when the cost is declined or cannot be paid (asked the same way, with No the only answer) and asks once for each instance of the keyword; cumulative upkeep puts an age counter on the permanent each upkeep and asks for its cost once for each counter (mana, life, sacrificed creatures), counts the counters of other instances, and asks nothing of a permanent that left in response; undying and persist return the card that died with a counter of their kind unless it had one, both together return it once, whichever the controller orders first, and neither finds a card that left the graveyard in response or a token; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{copying, name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::{burn, forest, free_creature, mountain, one_target};
use mtg_testkit::corpus::card;
use mtg_testkit::upkeep_cards::{
    ager, both, discard_a_card, echoer, mana, pay_life, persister, sacrifice_a_creature, undier,
};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, with_counters};
use rstest::rstest;

fn yes_no(yes: bool) -> Answer {
    Answer::YesNo(YesNoChoice(yes))
}

fn age() -> CounterKind {
    CounterKind::Other("age".into())
}

/// The log replays to the live state, and an engine rebuilt from it stands at the same decision.
#[allow(clippy::unnecessary_wraps)] // the tests end with it, as they end with their last script step
fn assert_replays(driver: &Driver, setup: &GameSetup) -> Result<(), ScriptError> {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
    Ok(())
}

/// The question the table waits on: who answers it, what is asked and whether No is the only answer.
fn asked(driver: &Driver) -> (usize, String, bool) {
    let decision = driver.decision().unwrap();
    match &decision.kind {
        DecisionKind::YesNo(options) => (
            decision.player.index(),
            options.text.clone(),
            options.only_no,
        ),
        other => panic!("a yes-or-no question was expected, not {}", other.name()),
    }
}

/// Answers passively until a yes-or-no question is pending (true) or the stack is empty at a priority decision (false).
fn until_question(driver: &mut Driver) -> Result<bool, ScriptError> {
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => return Ok(true),
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(false),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("a question or an empty stack".into()))
}

/// Passes priority until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty()
        && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    Ok(())
}

/// Says yes to the pending question and makes the picks and the payment it leads to; the picks prefer cards named
/// `prefer` and otherwise the first cards offered.
fn pay(driver: &mut Driver, prefer: &str) -> Result<(), ScriptError> {
    driver.answer(yes_no(true))?;
    loop {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                let mut from = options.from.clone();
                from.sort_by_key(|id| name_of(driver.state(), *id) != prefer);
                let picks = from.into_iter().take(options.min as usize).collect();
                driver.answer(Answer::ChooseCards(CardsChoice(picks)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => return Ok(()),
        }
    }
}

fn counters_on(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(kind))
}

fn on_battlefield(driver: &Driver, name: &str) -> bool {
    driver.id_of(name).is_ok()
}

fn in_graveyard(driver: &Driver, seat: usize, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Graveyard(pid(seat)))
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

/// Seat 0 controls `permanents`, and the other seats a creature each so that the table has its seats.
fn table(seats: usize, permanents: Vec<mtg_engine::setup::PresetPermanent>) -> Scenario {
    let mut table = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, permanents);
    for seat in 1..seats {
        table = table.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    table
}

fn lands(count: usize) -> Vec<mtg_engine::setup::PresetPermanent> {
    vec![ready(forest()); count]
}

/// The first upkeep of seat 0 after the opening turn.
fn next_upkeep(seats: usize) -> u32 {
    1 + u32::try_from(seats).unwrap()
}

/// How seat 0 answers the question of the keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reply {
    Pays,
    Declines,
    Cannot,
}

fn echo_table(seats: usize, cost: Cost, lands_held: usize, cards: usize) -> Scenario {
    let hand: Vec<_> = (0..cards)
        .map(|n| free_creature(&format!("Card {n}"), 1, 1))
        .collect();
    table(
        seats,
        [vec![sick(echoer("Echoer", cost))], lands(lands_held)].concat(),
    )
    .hand(0, hand)
}

#[rstest]
fn echo_is_paid_or_the_permanent_is_sacrificed(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        (mana("{1}"), "Pay {1}?", 2, 0, Reply::Pays),
        (mana("{1}"), "Pay {1}?", 2, 0, Reply::Declines),
        (mana("{1}"), "Pay {1}?", 0, 0, Reply::Cannot),
        (discard_a_card(), "Discard a card?", 0, 2, Reply::Pays),
        (discard_a_card(), "Discard a card?", 0, 1, Reply::Declines),
        (discard_a_card(), "Discard a card?", 0, 0, Reply::Cannot),
    )]
    case: (Cost, &str, usize, usize, Reply),
) -> Result<(), ScriptError> {
    let (cost, words, lands_held, cards, reply) = case;
    let setup = echo_table(seats, cost.clone(), lands_held, cards).setup()?;
    let mut driver = Driver::from_scenario(echo_table(seats, cost, lands_held, cards))?;
    driver.idle_until(next_upkeep(seats), Step::Upkeep)?;
    assert_eq!(driver.state().stack().len(), 1, "the echo trigger waits");
    assert!(until_question(&mut driver)?);
    assert_eq!(
        asked(&driver),
        (0, words.to_owned(), reply == Reply::Cannot)
    );
    match reply {
        Reply::Cannot => {
            assert!(
                driver.answer(yes_no(true)).is_err(),
                "a yes is no answer to what they cannot pay"
            );
            driver.answer(yes_no(false))?;
        }
        Reply::Declines => {
            driver.answer(yes_no(false))?;
        }
        Reply::Pays => pay(&mut driver, "Card 1")?,
    }
    settle(&mut driver)?;
    let paid = reply == Reply::Pays;
    assert_eq!(on_battlefield(&driver, "Echoer"), paid);
    assert_eq!(in_graveyard(&driver, 0, "Echoer"), !paid);
    if paid {
        // It is not asked again: the creature has been held since the turn began.
        driver.idle_until(
            next_upkeep(seats) + u32::try_from(seats).unwrap(),
            Step::Upkeep,
        )?;
        assert_eq!(driver.state().stack(), []);
        assert!(on_battlefield(&driver, "Echoer"));
    }
    assert_replays(&driver, &setup)
}

#[rstest]
fn echo_is_not_asked_of_a_permanent_held_since_the_turn_began(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || {
        table(
            seats,
            vec![ready(echoer("Echoer", mana("{1}"))), ready(forest())],
        )
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.idle_until(next_upkeep(seats), Step::Upkeep)?;
    assert_eq!(driver.state().stack(), []);
    assert!(on_battlefield(&driver, "Echoer"));
    assert_replays(&driver, &setup)
}

#[rstest]
fn echo_is_asked_of_the_player_who_took_control_of_a_permanent(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat 1 has held the Echoer for a long time; seat 0 takes it, and owes echo at its own next upkeep.
    let build = || {
        table(seats, vec![ready(forest())]).battlefield(1, [ready(echoer("Echoer", mana("{1}")))])
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let echoer = driver.id_of("Echoer")?;
    driver.add_effect(
        driver.on(echoer),
        Modification::SetController(pid(0)),
        Duration::Forever,
    )?;
    driver.idle_until(next_upkeep(seats), Step::Upkeep)?;
    assert_eq!(driver.state().stack().len(), 1);
    assert!(until_question(&mut driver)?);
    assert_eq!(asked(&driver), (0, "Pay {1}?".to_owned(), false));
    driver.answer(yes_no(false))?;
    settle(&mut driver)?;
    assert!(!on_battlefield(&driver, "Echoer"));
    assert!(
        in_graveyard(&driver, 1, "Echoer"),
        "it goes to its owner's graveyard"
    );
    assert_replays(&driver, &setup)
}

#[rstest]
fn echo_moves_to_the_player_who_took_the_permanent_before_the_upkeep(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // The Echoer entered under seat 0 this turn, and seat 1 takes it: seat 1's next upkeep asks, seat 0's never does.
    let build = || table(seats, vec![sick(echoer("Echoer", mana("{1}")))]);
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let echoer = driver.id_of("Echoer")?;
    driver.add_effect(
        driver.on(echoer),
        Modification::SetController(pid(1)),
        Duration::Forever,
    )?;
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(driver.state().stack().len(), 1);
    assert!(until_question(&mut driver)?);
    // Seat 1 has no mana to pay with.
    assert_eq!(asked(&driver), (1, "Pay {1}?".to_owned(), true));
    driver.answer(yes_no(false))?;
    settle(&mut driver)?;
    assert!(in_graveyard(&driver, 0, "Echoer"));
    assert_replays(&driver, &setup)
}

#[rstest]
fn each_instance_of_echo_is_asked_and_a_sacrifice_ends_the_rest(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        (vec![true, true], true),
        (vec![true, false], false),
        (vec![false], false),
    )]
    case: (Vec<bool>, bool),
) -> Result<(), ScriptError> {
    let (replies, survives) = case;
    let twice = Arc::new(
        CardDefinition::new("Twice")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(Ability::Keyword(Keyword::Echo(mana("{1}"))))
            .with_rules(Ability::Keyword(Keyword::Echo(mana("{1}")))),
    );
    let build = || table(seats, [vec![sick(twice.clone())], lands(2)].concat());
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.idle_until(next_upkeep(seats), Step::Upkeep)?;
    assert_eq!(
        driver.state().stack().len(),
        2,
        "one trigger for each instance"
    );
    let paid = replies.iter().filter(|reply| **reply).count();
    for reply in replies {
        assert!(until_question(&mut driver)?);
        if reply {
            pay(&mut driver, "")?;
        } else {
            driver.answer(yes_no(false))?;
        }
    }
    settle(&mut driver)?;
    // The second instance finds nothing to ask once the first sacrificed the creature.
    assert!(!until_question(&mut driver)?);
    assert_eq!(on_battlefield(&driver, "Twice"), survives);
    let tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            name_of(driver.state(), **id) == "Forest"
                && driver
                    .state()
                    .object(**id)
                    .and_then(|object| object.permanent())
                    .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count();
    assert_eq!(tapped, paid, "each payment tapped a land");
    assert_replays(&driver, &setup)
}

fn age_table(
    seats: usize,
    ager: Arc<CardDefinition>,
    extra: Vec<mtg_engine::setup::PresetPermanent>,
) -> Scenario {
    table(seats, [vec![ready(ager)], extra].concat())
}

#[rstest]
fn cumulative_upkeep_asks_for_its_mana_once_for_each_age_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || age_table(seats, ager("Ager", mana("{1}")), lands(3));
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let round = u32::try_from(seats).unwrap();
    for (count, words, cannot) in [
        (1, "Pay {1}?", false),
        (2, "Pay {2}?", false),
        (3, "Pay {3}?", false),
        (4, "Pay {4}?", true),
    ] {
        driver.idle_until(1 + round * count, Step::Upkeep)?;
        assert!(until_question(&mut driver)?);
        assert_eq!(asked(&driver), (0, words.to_owned(), cannot));
        assert_eq!(counters_on(&driver, "Ager", &age()), count);
        if cannot {
            driver.answer(yes_no(false))?;
        } else {
            pay(&mut driver, "")?;
        }
        settle(&mut driver)?;
    }
    assert!(!on_battlefield(&driver, "Ager"));
    assert!(in_graveyard(&driver, 0, "Ager"));
    assert_replays(&driver, &setup)
}

#[rstest]
fn cumulative_upkeep_declined_sacrifices_the_permanent(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || age_table(seats, ager("Ager", mana("{1}")), lands(2));
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.idle_until(next_upkeep(seats), Step::Upkeep)?;
    assert!(until_question(&mut driver)?);
    driver.answer(yes_no(false))?;
    settle(&mut driver)?;
    assert!(in_graveyard(&driver, 0, "Ager"));
    assert_replays(&driver, &setup)
}

#[rstest]
fn cumulative_upkeep_in_life_takes_the_life_once_for_each_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || age_table(seats, ager("Ager", pay_life(1)), Vec::new());
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let round = u32::try_from(seats).unwrap();
    let life = |driver: &Driver| driver.state().player(pid(0)).unwrap().life();
    let start = life(&driver);
    for count in 1..=3u32 {
        driver.idle_until(1 + round * count, Step::Upkeep)?;
        assert!(until_question(&mut driver)?);
        pay(&mut driver, "")?;
        settle(&mut driver)?;
        let spent: i32 = (1..=count).map(|n| i32::try_from(n).unwrap()).sum();
        assert_eq!(life(&driver), start - spent);
    }
    assert!(on_battlefield(&driver, "Ager"));
    assert_replays(&driver, &setup)
}

#[rstest]
fn cumulative_upkeep_in_life_cannot_be_paid_with_less_life_than_all_of_it_takes(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Two life pays the first upkeep (1) and leaves one, which is not the two the second upkeep asks for.
    let build = || age_table(seats, ager("Ager", pay_life(1)), Vec::new()).life(0, 2);
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let round = u32::try_from(seats).unwrap();
    driver.idle_until(1 + round, Step::Upkeep)?;
    assert!(until_question(&mut driver)?);
    assert_eq!(asked(&driver), (0, "Pay 1 life?".to_owned(), false));
    pay(&mut driver, "")?;
    settle(&mut driver)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 1);
    driver.idle_until(1 + round * 2, Step::Upkeep)?;
    assert!(until_question(&mut driver)?);
    assert_eq!(asked(&driver), (0, "Pay 2 life?".to_owned(), true));
    assert!(driver.answer(yes_no(true)).is_err());
    driver.answer(yes_no(false))?;
    settle(&mut driver)?;
    assert!(in_graveyard(&driver, 0, "Ager"));
    assert_replays(&driver, &setup)
}

#[rstest]
fn cumulative_upkeep_by_sacrifice_takes_a_creature_for_each_counter(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mites = vec![ready(free_creature("Mite", 1, 1)); 3];
    let build = || age_table(seats, ager("Ager", sacrifice_a_creature()), mites.clone());
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let round = u32::try_from(seats).unwrap();
    let mites_left = |driver: &Driver| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| name_of(driver.state(), **id) == "Mite")
            .count()
    };
    driver.idle_until(1 + round, Step::Upkeep)?;
    assert!(until_question(&mut driver)?);
    assert_eq!(asked(&driver).1, "Sacrifice a creature?");
    pay(&mut driver, "Mite")?;
    settle(&mut driver)?;
    assert_eq!(mites_left(&driver), 2);
    driver.idle_until(1 + round * 2, Step::Upkeep)?;
    assert!(until_question(&mut driver)?);
    pay(&mut driver, "Mite")?;
    settle(&mut driver)?;
    assert_eq!(mites_left(&driver), 0);
    assert!(on_battlefield(&driver, "Ager"));
    // Nothing is left to sacrifice but the Ager itself, and a Bystander of another seat is not theirs to give.
    driver.idle_until(1 + round * 3, Step::Upkeep)?;
    assert!(until_question(&mut driver)?);
    let (_, _, only_no) = asked(&driver);
    assert!(only_no, "three creatures are asked of a player with one");
    driver.answer(yes_no(false))?;
    settle(&mut driver)?;
    assert!(in_graveyard(&driver, 0, "Ager"));
    assert_replays(&driver, &setup)
}

#[rstest]
fn the_age_counters_are_shared_by_the_instances_of_cumulative_upkeep(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let twice = Arc::new(
        CardDefinition::new("Twice")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(Ability::Keyword(Keyword::CumulativeUpkeep(mana("{1}"))))
            .with_rules(Ability::Keyword(Keyword::CumulativeUpkeep(mana("{1}")))),
    );
    let build = || age_table(seats, twice.clone(), lands(4));
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.idle_until(next_upkeep(seats), Step::Upkeep)?;
    assert_eq!(driver.state().stack().len(), 2);
    for words in ["Pay {1}?", "Pay {2}?"] {
        assert!(until_question(&mut driver)?);
        assert_eq!(asked(&driver).1, words);
        pay(&mut driver, "")?;
    }
    settle(&mut driver)?;
    assert_eq!(counters_on(&driver, "Twice", &age()), 2);
    assert_replays(&driver, &setup)
}

#[rstest]
fn cumulative_upkeep_asks_nothing_of_a_permanent_that_left_in_response(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || {
        age_table(
            seats,
            ager("Ager", mana("{1}")),
            vec![ready(mountain()), ready(forest())],
        )
        .hand(0, vec![burn("Bolt", 3)])
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.idle_until(next_upkeep(seats), Step::Upkeep)?;
    assert_eq!(driver.state().stack().len(), 1);
    let target = driver.target_named("Ager")?;
    driver.cast_at("Bolt", &[target])?;
    // The Bolt resolves first and the Ager dies; the trigger then finds nothing to count and nobody is asked.
    assert!(!until_question(&mut driver)?);
    settle(&mut driver)?;
    assert!(in_graveyard(&driver, 0, "Ager"));
    assert_replays(&driver, &setup)
}

/// What a creature with counters preset does when it dies: whether it comes back and with which counters.
#[rstest]
fn undying_and_persist_return_the_card_with_a_counter_unless_it_had_one(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        (true, vec![], true, 1, 0),
        (true, vec![CounterKind::PlusOnePlusOne], false, 0, 0),
        (true, vec![CounterKind::MinusOneMinusOne], true, 1, 0),
        (false, vec![], true, 0, 1),
        (false, vec![CounterKind::MinusOneMinusOne], false, 0, 0),
        (false, vec![CounterKind::PlusOnePlusOne], true, 0, 1),
    )]
    case: (bool, Vec<CounterKind>, bool, u32, u32),
) -> Result<(), ScriptError> {
    let (undying, preset, returns, plus, minus) = case;
    let card = if undying {
        undier("Dier")
    } else {
        persister("Dier")
    };
    let mut dier = ready(card);
    for kind in preset {
        dier = with_counters(dier, kind, 1);
    }
    let build =
        || table(seats, vec![dier.clone(), ready(mountain())]).hand(0, vec![burn("Bolt", 9)]);
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let target = driver.target_named("Dier")?;
    driver.cast_at("Bolt", &[target])?;
    settle(&mut driver)?;
    assert_eq!(on_battlefield(&driver, "Dier"), returns);
    assert_eq!(in_graveyard(&driver, 0, "Dier"), !returns);
    if returns {
        assert_eq!(
            counters_on(&driver, "Dier", &CounterKind::PlusOnePlusOne),
            plus
        );
        assert_eq!(
            counters_on(&driver, "Dier", &CounterKind::MinusOneMinusOne),
            minus
        );
    }
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_returned_creature_that_dies_again_stays_dead(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] undying: bool,
) -> Result<(), ScriptError> {
    let card = if undying {
        undier("Dier")
    } else {
        persister("Dier")
    };
    let build = || {
        table(
            seats,
            vec![ready(card.clone()), ready(mountain()), ready(mountain())],
        )
        .hand(0, vec![burn("Bolt", 9), burn("Bolt", 9)])
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    for _ in 0..2 {
        let target = driver.target_named("Dier");
        let Ok(target) = target else { break };
        driver.cast_at("Bolt", &[target])?;
        settle(&mut driver)?;
    }
    assert!(!on_battlefield(&driver, "Dier"));
    assert!(in_graveyard(&driver, 0, "Dier"));
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_creature_with_undying_and_persist_returns_once_whichever_the_controller_orders_first(
    #[values(2, 3, 4)] seats: usize,
    #[values(vec![0, 1], vec![1, 0])] order: Vec<usize>,
) -> Result<(), ScriptError> {
    let build = || {
        table(seats, vec![ready(both("Twice Dead")), ready(mountain())])
            .hand(0, vec![burn("Bolt", 9)])
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let target = driver.target_named("Twice Dead")?;
    driver.cast_at("Bolt", &[target])?;
    // Two triggers of different abilities: the controller is asked which goes on the stack first.
    while !matches!(driver.decision()?.kind, DecisionKind::OrderTriggers(_)) {
        driver.pass()?;
    }
    assert_eq!(driver.trigger_options()?.triggers.len(), 2);
    driver.order_triggers(&order)?;
    settle(&mut driver)?;
    let plus = counters_on(&driver, "Twice Dead", &CounterKind::PlusOnePlusOne);
    let minus = counters_on(&driver, "Twice Dead", &CounterKind::MinusOneMinusOne);
    assert_eq!(plus + minus, 1, "it came back once, with one counter");
    // The trigger put on the stack last resolves first.
    let first = *order.last().unwrap();
    assert_eq!((plus, minus), if first == 0 { (1, 0) } else { (0, 1) });
    assert_replays(&driver, &setup)
}

/// An instant for nothing that exiles a target card in a graveyard.
fn exiler() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Exiler")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Object(Selector(vec![Pred::In(
                    ZoneKind::Graveyard,
                )])))],
                effects: vec![Effect::Exile {
                    what: Ref::Target(0),
                    bind: None,
                    duration: None,
                }],
            }),
    )
}

#[rstest]
fn a_card_that_left_the_graveyard_in_response_is_not_returned(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] undying: bool,
) -> Result<(), ScriptError> {
    let card = if undying {
        undier("Dier")
    } else {
        persister("Dier")
    };
    let build = || {
        table(seats, vec![ready(card.clone()), ready(mountain())])
            .hand(0, vec![burn("Bolt", 9), exiler()])
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let target = driver.target_named("Dier")?;
    driver.cast_at("Bolt", &[target])?;
    // Pass until the Bolt has resolved and the trigger waits on the stack, then exile the card in response.
    while driver.state().stack().len() != 1
        || !in_graveyard(&driver, 0, "Dier")
        || !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    let card = driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .find(|id| name_of(driver.state(), **id) == "Dier")
        .copied()
        .unwrap();
    let reference = driver.state().object(card).unwrap().reference();
    driver.begin_cast("Exiler")?;
    driver.choose_targets(&[Target::Object(reference)])?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    settle(&mut driver)?;
    assert!(!on_battlefield(&driver, "Dier"));
    assert!(!in_graveyard(&driver, 0, "Dier"));
    assert_eq!(driver.state().zone(Zone::Exile).len(), 1);
    assert_replays(&driver, &setup)
}

/// Seat 0 casts Clone over four Islands, and every question about what to copy is answered with everything offered (the
/// creature seat 1 controls); it returns once the stack is empty.
fn clone_copying(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    driver.cast_tapping("Clone", &["Island", "Island", "Island", "Island"])?;
    until_settled(driver, seats)
}

/// Answers the questions of what to copy with everything offered and passes until the stack is empty.
fn until_settled(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..50 {
        match driver.engine().decision().map(|d| d.kind.clone()) {
            Some(DecisionKind::ChooseCards(options)) => {
                driver.answer(Answer::ChooseCards(CardsChoice(options.from)))?;
            }
            _ if driver.state().stack().is_empty() && driver.state().resolving().is_none() => {
                return Ok(());
            }
            Some(DecisionKind::Priority(_)) => copying::run(driver, seats)?,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the copy".into()))
}

fn copy_table(seats: usize, copied: Arc<CardDefinition>) -> Scenario {
    Scenario::new(seats)
        .hand(0, vec![card("Clone").unwrap(), burn("Bolt", 9)])
        .battlefield(0, copying::lands(4, 1))
        .battlefield(1, [ready(copied)])
}

#[rstest]
fn a_copy_of_a_permanent_with_echo_owes_it_at_its_controllers_next_upkeep(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || copy_table(seats, echoer("Echoer", mana("{1}")));
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    clone_copying(&mut driver, seats)?;
    let echoers = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == "Echoer")
        .count();
    assert_eq!(echoers, 2, "the copy entered beside the original");
    driver.idle_until(next_upkeep(seats), Step::Upkeep)?;
    // The copy came under seat 0's control this turn; the original has been held by seat 1 since long ago.
    assert_eq!(driver.state().stack().len(), 1);
    assert!(until_question(&mut driver)?);
    assert_eq!(asked(&driver).0, 0);
    driver.answer(yes_no(false))?;
    settle(&mut driver)?;
    assert!(
        in_graveyard(&driver, 0, "Clone"),
        "the copy was sacrificed as the card it is"
    );
    assert!(on_battlefield(&driver, "Echoer"), "the original stays");
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_copy_of_a_permanent_with_undying_returns_as_the_card_it_is_with_a_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || copy_table(seats, undier("Dier"));
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    clone_copying(&mut driver, seats)?;
    let copy = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            let object = driver.state().object(*id).unwrap();
            object.controller() == pid(0) && name_of(driver.state(), *id) == "Dier"
        })
        .expect("the copy entered");
    let target = Target::Object(driver.state().object(copy).unwrap().reference());
    driver.cast_at("Bolt", &[target])?;
    // The card comes back as a Clone and chooses again what to copy, so it is named as the Dier is.
    until_settled(&mut driver, seats)?;
    let returned = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            let object = driver.state().object(*id).unwrap();
            object.controller() == pid(0) && name_of(driver.state(), *id) == "Dier"
        })
        .expect("the Clone card is back on the battlefield, a copy of the Dier");
    let counters = driver
        .state()
        .object(returned)
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&CounterKind::PlusOnePlusOne));
    assert_eq!(counters, Some(1));
    assert!(!in_graveyard(&driver, 0, "Clone"));
    assert_replays(&driver, &setup)
}

/// A sorcery for nothing that creates a 2/2 token with undying.
fn wisp_maker() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Wisp","types":["Creature"],"power":2,"toughness":2,"abilities":[{"Keyword":"Undying"}]}}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Maker")
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(ability),
    )
}

#[rstest]
fn a_token_with_undying_ceases_to_exist_and_is_not_returned(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build =
        || table(seats, vec![ready(mountain())]).hand(0, vec![wisp_maker(), burn("Bolt", 9)]);
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.cast("Maker")?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    settle(&mut driver)?;
    assert!(on_battlefield(&driver, "Wisp"));
    let target = driver.target_named("Wisp")?;
    driver.cast_at("Bolt", &[target])?;
    settle(&mut driver)?;
    assert!(!on_battlefield(&driver, "Wisp"));
    assert!(!in_graveyard(&driver, 0, "Wisp"));
    assert_replays(&driver, &setup)
}

#[rstest]
fn the_cards_offered_to_discard_for_echo_are_the_hand_of_the_player_who_pays_and_no_one_elses(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || {
        let mut table = echo_table(seats, discard_a_card(), 0, 2);
        for seat in 1..seats {
            table = table.hand(seat, vec![free_creature(&format!("Secret {seat}"), 1, 1)]);
        }
        table
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.idle_until(next_upkeep(seats), Step::Upkeep)?;
    assert!(until_question(&mut driver)?);
    driver.answer(yes_no(true))?;
    let decision = driver.decision()?.clone();
    assert_eq!(decision.player, pid(0), "the payer picks");
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("a pick was expected, not {}", decision.kind.name());
    };
    let mut offered: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    offered.sort();
    assert_eq!(offered, ["Card 0", "Card 1"]);
    assert_replays(&driver, &setup)
}
