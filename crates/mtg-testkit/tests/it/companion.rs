//! Goal: a companion lies outside the game and its owner brings it into their hand with a special action that costs {3} (whatever the cost modifiers say), that is offered with priority at sorcery timing only, uses no stack, can be backed out of and is used up for the game; the card is revealed before the shuffles in turn order from the first player, a seat that left the game is never offered it, a deck that breaks the companion's condition or a card without the ability is refused when the game is set up, and a replay of the log rebuilds the game before and after the action, all at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_engine::companions::CompanionStage;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction, PriorityOptions};
use mtg_engine::error::SetupError;
use mtg_engine::format::{Commander, DeckError, Normal};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameSetup};
use mtg_testkit::cards::{cost, creature, forest, island, land};
use mtg_testkit::companion_cards::{
    companion_with, distinct_nonlands, lands_and_big_spells, small_companion, small_deck,
};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// The options of the holder of priority.
fn options(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("priority is asked, not {}", other.name()),
    }
}

/// A game of `seats` seats at `step` of `turn` with seat `active` to move; seat 0 has `lands` untapped Forests and a
/// companion, every other seat has a companion too.
fn game(
    seats: usize,
    turn: u32,
    active: usize,
    step: Step,
    lands: usize,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(turn, active, step)
        .battlefield(0, (0..lands).map(|_| ready(forest())).collect::<Vec<_>>());
    for seat in 0..seats {
        scenario = scenario.companion(seat, small_companion());
    }
    Driver::from_scenario(scenario)
}

/// Passes until seat 0 holds priority, at most a turn's worth of times.
fn until_seat_zero(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.decision()?.player == pid(0) {
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("seat 0 never held priority");
}

/// Brings seat 0's companion into the hand, paying with its Forests.
fn bring(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    driver.answer(Answer::Priority(PriorityAction::BringCompanion))?;
    driver.pay_tapping(&["Forest", "Forest", "Forest"], 0)
}

#[rstest]
#[case::first_main_with_three_lands(Step::Main1, 0, 3, true)]
#[case::second_main_with_three_lands(Step::Main2, 0, 3, true)]
#[case::first_main_with_more_lands(Step::Main1, 0, 5, true)]
#[case::first_main_with_two_lands(Step::Main1, 0, 2, false)]
#[case::first_main_without_lands(Step::Main1, 0, 0, false)]
#[case::upkeep(Step::Upkeep, 0, 3, false)]
#[case::draw_step(Step::Draw, 0, 3, false)]
#[case::beginning_of_combat(Step::BeginCombat, 0, 3, false)]
#[case::end_step(Step::End, 0, 3, false)]
#[case::an_opponents_first_main(Step::Main1, 1, 3, false)]
#[case::an_opponents_second_main(Step::Main2, 1, 3, false)]
#[case::an_opponents_end_step(Step::End, 1, 3, false)]
fn the_action_is_offered_at_sorcery_timing_with_three_mana(
    #[case] step: Step,
    #[case] active: usize,
    #[case] lands: usize,
    #[case] offered: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 1, active, step, lands)?;
    until_seat_zero(&mut driver)?;
    assert_eq!(options(&driver).companion, offered);
    assert_eq!(
        options(&driver).can_act,
        offered,
        "with nothing else to do, the action is the only thing there is to do"
    );
    Ok(())
}

#[rstest]
fn the_action_is_not_offered_with_a_spell_on_the_stack(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, [mtg_testkit::cards::burn("Spark", 1)])
        .battlefield(
            0,
            (0..4)
                .map(|_| ready(mtg_testkit::cards::mountain()))
                .collect::<Vec<_>>(),
        );
    for seat in 0..seats {
        scenario = scenario.companion(seat, small_companion());
    }
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(options(&driver).companion, "offered before the spell");
    let target = driver.target_player(1);
    driver.cast_at("Spark", &[target])?;
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
    assert!(!options(&driver).companion, "a sorcery could not be cast");
    Ok(())
}

#[rstest]
fn the_action_puts_the_card_in_the_hand_for_three_without_the_stack(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 1, 0, Step::Main1, 4)?;
    let before = driver.state().zone(Zone::Hand(pid(0))).len();
    assert_eq!(
        driver.state().companion(pid(0)).unwrap().stage,
        CompanionStage::Revealed
    );
    let events = bring(&mut driver)?;
    let [brought] = events
        .iter()
        .filter_map(|event| match event {
            Event::CompanionBrought { player, object } => Some((*player, *object)),
            _ => None,
        })
        .collect::<Vec<_>>()[..]
    else {
        panic!("exactly one CompanionBrought in {events:?}");
    };
    assert_eq!(brought.0, pid(0));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::SpellCast { .. })),
        "a special action does not use the stack"
    );
    let state = driver.state();
    assert_eq!(state.zone(Zone::Stack), []);
    assert_eq!(state.zone(Zone::Hand(pid(0))).len(), before + 1);
    let card = state.object(brought.1).unwrap();
    assert_eq!(card.zone(), Zone::Hand(pid(0)));
    assert_eq!(card.owner(), pid(0));
    assert_eq!(card.card().name, "Small Companion");
    let info = state.companion(pid(0)).unwrap();
    assert_eq!(
        (info.stage, info.card),
        (CompanionStage::Brought, brought.1)
    );
    let tapped = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.permanent().is_some_and(|p| p.tapped()))
        .count();
    assert_eq!(tapped, 3, "{{3}} was paid");
    assert_eq!(
        driver.decision()?.player,
        pid(0),
        "the owner keeps priority"
    );
    Ok(())
}

#[rstest]
fn the_action_is_used_up_for_the_game(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] cast_it: bool,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 1, 0, Step::Main1, 8)?;
    bring(&mut driver)?;
    assert!(
        !options(&driver).companion,
        "offered again in the same phase"
    );
    if cast_it {
        // The card in the hand is a creature for {1}{G}: casting it leaves the action used up as well.
        driver.cast_tapping("Small Companion", &["Forest", "Forest"])?;
        driver.pass()?;
        driver.pass()?;
        while driver.decision()?.player != pid(0) || driver.state().zone(Zone::Stack).len() > 0 {
            driver.pass()?;
        }
        assert!(!options(&driver).companion, "offered with the card in play");
    }
    let err = driver.answer(Answer::Priority(PriorityAction::BringCompanion));
    assert!(err.is_err(), "a second use is refused: {err:?}");
    driver.idle_until(3, Step::Main1)?;
    until_seat_zero(&mut driver)?;
    assert!(!options(&driver).companion, "still used up two turns later");
    Ok(())
}

#[rstest]
fn the_action_is_refused_when_it_is_not_offered(
    #[values(2, 3, 4)] seats: usize,
    #[values(Step::Upkeep, Step::End)] step: Step,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 1, 0, step, 5)?;
    let err = driver.answer(Answer::Priority(PriorityAction::BringCompanion));
    assert!(err.is_err(), "refused outside sorcery timing: {err:?}");
    assert_eq!(
        driver.state().companion(pid(0)).unwrap().stage,
        CompanionStage::Revealed
    );
    Ok(())
}

#[rstest]
fn backing_out_of_the_payment_leaves_everything_as_it_was(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 1, 0, Step::Main1, 3)?;
    let hash = driver.engine().state_hash();
    let events = driver.engine().events().len();
    driver.answer(Answer::Priority(PriorityAction::BringCompanion))?;
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(driver.engine().state_hash(), hash);
    assert_eq!(driver.engine().events().len(), events, "nothing is logged");
    assert!(options(&driver).companion, "offered again");
    Ok(())
}

#[rstest]
fn a_seat_that_left_the_game_is_never_offered_its_companion(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 1, 0, Step::Main1, 3)?;
    driver.concede(pid(1))?;
    assert!(
        driver.state().companion(pid(1)).is_some(),
        "the card is still named"
    );
    // Seat 0 can still bring its own; the seat that left gets no priority at all.
    assert!(options(&driver).companion);
    for _ in 0..30 {
        let decision = driver.decision()?;
        assert_ne!(decision.player, pid(1));
        if driver.state().turn().active() == pid(2) {
            break;
        }
        driver.pass()?;
    }
    assert!(driver.state().zone(Zone::Hand(pid(1))).is_empty());
    assert_eq!(
        driver.state().companion(pid(1)).unwrap().stage,
        CompanionStage::Revealed,
        "it never came into the game"
    );
    Ok(())
}

#[rstest]
#[case::before_the_action(false)]
#[case::after_the_action(true)]
fn a_replay_of_the_log_rebuilds_the_game(
    #[case] brought: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 1, 0, Step::Main1, 3)?;
    if brought {
        bring(&mut driver)?;
    }
    let rebuilt = Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), driver.engine().state_hash());
    let stage = rebuilt.state().companion(pid(0)).unwrap().stage;
    assert_eq!(
        stage,
        if brought {
            CompanionStage::Brought
        } else {
            CompanionStage::Revealed
        }
    );
    Ok(())
}

/// A Normal game of `seats` seats from a seed, with a small deck at each seat and these companions.
fn opening(seats: usize, seed: u64, companions: &[usize]) -> Engine {
    let decks = (0..seats)
        .map(|seat| {
            small_deck(&mut mtg_engine::rng::GameRng::substream(
                seed,
                9_000 + seat as u64,
            ))
        })
        .collect();
    let setup = GameSetup::new(Arc::new(Normal), seed, decks).with_companions(
        (0..seats)
            .map(|seat| companions.contains(&seat).then(small_companion))
            .collect(),
    );
    Engine::new(setup).unwrap()
}

#[rstest]
fn companions_are_revealed_before_the_shuffles_from_the_first_player_in_turn_order(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3, 4, 5, 6, 7)] seed: u64,
    #[values(0, 1)] which: usize,
) {
    let companions: Vec<usize> = if which == 0 {
        (0..seats).collect()
    } else {
        vec![seats - 1]
    };
    let engine = opening(seats, seed, &companions);
    let events = engine.events();
    let Some(Event::FirstPlayerChosen { player: first }) = events.first().cloned() else {
        panic!("the first event picks the first player: {events:?}");
    };
    let revealed: Vec<_> = events
        .iter()
        .take_while(|event| !matches!(event, Event::LibraryShuffled { .. }))
        .filter_map(|event| match event {
            Event::CompanionRevealed { player } => Some(*player),
            _ => None,
        })
        .collect();
    let expected: Vec<_> = (0..seats)
        .map(|offset| pid((first.index() + offset) % seats))
        .filter(|player| companions.contains(&player.index()))
        .collect();
    assert_eq!(revealed, expected);
    for seat in 0..seats {
        let info = engine.state().companion(pid(seat));
        assert_eq!(
            info.map(|info| info.stage),
            companions
                .contains(&seat)
                .then_some(CompanionStage::Revealed)
        );
    }
    let in_game_objects = engine.state().object_count();
    let library_cards: usize = (0..seats)
        .map(|seat| engine.state().zone(Zone::Library(pid(seat))).len())
        .sum::<usize>()
        + (0..seats)
            .map(|seat| engine.state().zone(Zone::Hand(pid(seat))).len())
            .sum::<usize>();
    assert_eq!(
        in_game_objects, library_cards,
        "the companions are no objects"
    );
}

/// A Normal setup for seat 0 with a small deck and `companion`, validated.
fn validated(
    companion: Arc<CardDefinition>,
    mutate: impl FnOnce(&mut Vec<Arc<CardDefinition>>),
) -> Result<(), SetupError> {
    let mut deck = small_deck(&mut mtg_engine::rng::GameRng::substream(1, 1));
    mutate(&mut deck);
    let other = small_deck(&mut mtg_engine::rng::GameRng::substream(1, 2));
    GameSetup::new(Arc::new(Normal), 1, vec![deck, other])
        .with_companions(vec![Some(companion), None])
        .validate()
}

fn no_ability() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new("Plain Card").with_type(CardType::Creature))
}

type Deck = Vec<Arc<CardDefinition>>;

fn unchanged(_: &mut Deck) {}

fn with_a_three_drop(deck: &mut Deck) {
    deck[0] = creature("Big", 3, &[], 3, 3);
}

fn with_another_land(deck: &mut Deck) {
    deck.push(land(Color::Red));
}

#[rstest]
#[case::a_deck_that_fits(small_companion(), unchanged, None)]
#[case::a_three_drop_among_the_permanents(small_companion(), with_a_three_drop, Some("Big"))]
#[case::a_land_never_breaks_a_value_condition(small_companion(), with_another_land, None)]
#[case::a_card_without_the_ability(no_ability(), unchanged, Some("Plain Card"))]
#[case::lands_and_big_spells_with_cheap_spells(
    companion_with("Keruga", lands_and_big_spells()),
    unchanged,
    Some("")
)]
#[case::distinct_names_with_repeated_bears(
    companion_with("Lutri", distinct_nonlands()),
    unchanged,
    Some("Grizzly Bears")
)]
fn a_companion_must_have_the_ability_and_the_deck_must_meet_its_condition(
    #[case] companion: Arc<CardDefinition>,
    #[case] mutate: fn(&mut Deck),
    #[case] refused_for: Option<&str>,
) {
    let name = companion.name.clone();
    let result = validated(companion, mutate);
    match (result, refused_for) {
        (Ok(()), None) => {}
        (Err(SetupError::InvalidCompanion { player, source }), Some(card)) => {
            assert_eq!(player, pid(0));
            match source {
                DeckError::NotACompanion { name: found } => assert_eq!(found, name),
                DeckError::CompanionCondition {
                    name: found,
                    reason,
                } => {
                    assert_eq!(found, name);
                    assert!(reason.contains(card), "{reason}");
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        (other, expected) => panic!("expected {expected:?}, got {other:?}"),
    }
}

/// A Commander deck for seat 0: a legendary `leader` costing `{R}` and 99 Mountains, with this companion.
fn commander_game(leader_color: Color, companion: Arc<CardDefinition>) -> Result<(), SetupError> {
    let leader = Arc::new(
        CardDefinition::new("Leader")
            .with_supertype(Supertype::Legendary)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[leader_color]))
            .with_power_toughness(1, 1),
    );
    let deck = vec![land(leader_color); 99];
    GameSetup::new(Arc::new(Commander), 1, vec![deck.clone(), deck])
        .with_commanders(vec![vec![leader.clone()], vec![leader]])
        .with_companions(vec![Some(companion), None])
        .validate()
}

#[rstest]
#[case::inside_the_identity(Color::Green, small_companion(), None)]
#[case::outside_the_identity(Color::Red, small_companion(), Some("Small Companion"))]
#[case::the_commander_counts_toward_the_condition(
    Color::Green,
    companion_with("Cheap Only", mtg_cards::companion::DeckCondition::Each {
        of: Default::default(),
        has: mtg_cards::refs::Selector(vec![mtg_cards::refs::Pred::Colorless]),
    }),
    Some("Leader")
)]
#[case::the_extra_cards_of_yorion_cannot_be_met_in_a_hundred_card_deck(
    Color::Green,
    companion_with("Yorion", mtg_cards::companion::DeckCondition::ExtraCards(20)),
    Some("100")
)]
fn in_commander_the_companion_stays_inside_the_identity_and_the_commander_is_part_of_the_deck(
    #[case] leader: Color,
    #[case] companion: Arc<CardDefinition>,
    #[case] refused_for: Option<&str>,
) {
    match (commander_game(leader, companion), refused_for) {
        (Ok(()), None) => {}
        (Err(SetupError::InvalidCompanion { source, .. }), Some(text)) => {
            assert!(source.to_string().contains(text), "{source}");
        }
        (other, expected) => panic!("expected {expected:?}, got {other:?}"),
    }
}

#[rstest]
fn a_scenario_does_not_check_the_decks_against_the_companion(#[values(2, 3, 4)] seats: usize) {
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, vec![ready(island())])
            .companion(0, no_ability()),
    );
    assert!(driver.is_ok());
}
