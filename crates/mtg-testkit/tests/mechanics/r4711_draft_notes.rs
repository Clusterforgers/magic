//! Goal: the Conspiracy cards that read a draft note play at two to four seats as their rulings say for a game without a
//! draft: a noted number is 0 (Lurking Automaton and Pyretic Hunter enter with no counters and die, Garbage Fire deals no
//! damage: "If you somehow cast ... Garbage Fire but you didn't draft one, the highest noted number is considered to be 0"),
//! no guess was right (Spire Phantasm draws nothing) and no color was chosen (Regicide has no legal target: "you won't be
//! able to cast it because you won't be able to choose a legal target").
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Priority passes until the stack is empty.
fn r4711_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 with `lands` ready and `spell` in hand, every other seat with a 1/1, in seat 0's first main phase.
fn r4711_table(
    seats: usize,
    spell: &str,
    lands: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, lands.into_iter().map(ready).collect::<Vec<_>>())
        .hand(0, [card(spell).unwrap()]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, vec![ready(free_creature("Pip", 1, 1))]);
    }
    Driver::from_scenario(scenario.at(1, 0, Step::Main1))
}

#[rstest]
#[case::automaton_two_seats("Lurking Automaton", 2, 5)]
#[case::automaton_four_seats("Lurking Automaton", 4, 5)]
#[case::hunter_three_seats("Pyretic Hunter", 3, 5)]
fn r4711_a_noted_number_is_zero_so_the_creature_enters_with_no_counters(
    #[case] name: &str,
    #[case] seats: usize,
    #[case] lands: usize,
) -> Result<(), ScriptError> {
    let mut driver = r4711_table(seats, name, (0..lands).map(|_| mountain()).collect())?;
    driver.cast(name)?;
    r4711_settle(&mut driver)?;
    let graveyard = driver
        .state()
        .zone(Zone::Graveyard(PlayerId::from_index(0)));
    let in_graveyard = graveyard.iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    });
    assert!(in_graveyard, "{name} enters as a 0/0 and dies");
    Ok(())
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 3)]
fn r4711_garbage_fire_deals_no_damage_without_a_draft(
    #[case] seats: usize,
    #[case] victim: usize,
) -> Result<(), ScriptError> {
    let mut driver = r4711_table(seats, "Garbage Fire", (0..3).map(|_| mountain()).collect())?;
    driver.begin_cast("Garbage Fire")?;
    let pips: Vec<_> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            mtg_engine::characteristics::controller_of(driver.state(), *id)
                == Some(PlayerId::from_index(victim))
        })
        .collect();
    let pip = pips[0];
    driver.choose_targets(&[Target::Object(
        driver.state().object(pip).unwrap().reference(),
    )])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    r4711_settle(&mut driver)?;
    assert!(
        driver.state().zone(Zone::Battlefield).contains(&pip),
        "the 1/1 takes no damage"
    );
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn r4711_spire_phantasm_draws_nothing_without_a_guess(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r4711_table(seats, "Spire Phantasm", (0..4).map(|_| island()).collect())?;
    driver.cast("Spire Phantasm")?;
    r4711_settle(&mut driver)?;
    let hand = driver.state().zone(Zone::Hand(PlayerId::from_index(0)));
    assert!(hand.is_empty(), "no card drawn: {hand:?}");
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
fn r4711_regicide_has_no_legal_target_without_chosen_colors(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r4711_table(seats, "Regicide", vec![swamp()])?;
    assert!(
        driver.begin_cast("Regicide").is_err()
            || !matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)),
        "Regicide offers no target"
    );
    Ok(())
}

/// Seat 0 casts `name` and passes until the stack is empty, answering every other question passively.
fn r4711_cast_and_settle(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.cast(name)?;
    for _ in 0..60 {
        match driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(())
}

/// Whether a permanent with this name is on the battlefield.
fn r4711_on_battlefield(driver: &Driver, name: &str) -> bool {
    driver.state().zone(Zone::Battlefield).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    })
}

/// Arcane Savant ("copy a card you exiled with cards named Arcane Savant") and Aether Searcher ("search your hand and/or
/// library for a card with a name noted as you drafted ...") enter and find nothing: nothing was exiled before the game
/// and no name was noted ("Unless you also drafted a card named Aether Searcher, you won't cast a card").
#[rstest]
#[case::savant_two_seats("Arcane Savant", 2, 5)]
#[case::savant_four_seats("Arcane Savant", 4, 5)]
#[case::searcher_three_seats("Aether Searcher", 3, 7)]
fn r4711_an_enters_ability_of_a_draft_finds_nothing(
    #[case] name: &str,
    #[case] seats: usize,
    #[case] lands: usize,
) -> Result<(), ScriptError> {
    let mut driver = r4711_table(seats, name, (0..lands).map(|_| island()).collect())?;
    let library = driver
        .state()
        .zone(Zone::Library(PlayerId::from_index(0)))
        .len();
    r4711_cast_and_settle(&mut driver, name)?;
    assert!(r4711_on_battlefield(&driver, name));
    assert!(
        driver.state().stack().is_empty() && driver.state().zone(Zone::Exile).is_empty(),
        "no copy was made"
    );
    let hand = driver.state().zone(Zone::Hand(PlayerId::from_index(0)));
    assert!(hand.is_empty(), "nothing was found: {hand:?}");
    assert_eq!(
        driver
            .state()
            .zone(Zone::Library(PlayerId::from_index(0)))
            .len(),
        library
    );
    Ok(())
}

/// Volatile Chimera's "{1}{R}: Choose a card at random you exiled with cards named Volatile Chimera. This creature becomes a
/// copy of that card" picks from no card without a draft, so it stays itself.
#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
fn r4711_volatile_chimera_stays_itself_without_exiled_cards(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r4711_table(
        seats,
        "Volatile Chimera",
        (0..5).map(|_| mountain()).collect(),
    )?;
    r4711_cast_and_settle(&mut driver, "Volatile Chimera")?;
    driver.begin_activation("Volatile Chimera")?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    r4711_settle(&mut driver)?;
    assert!(r4711_on_battlefield(&driver, "Volatile Chimera"));
    Ok(())
}
