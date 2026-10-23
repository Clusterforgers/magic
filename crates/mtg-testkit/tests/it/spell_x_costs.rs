//! Goal: a spell whose required additional cost counts X ("pay X life", "sacrifice X creatures", "discard X cards", "exile X cards from your graveyard") asks X as it is cast at 2, 3 and 4 seats, from 0 to the most the caster can give (all their life, the creatures, cards or graveyard cards they have, and for an `{X}` in the mana cost also what the mana allows), asks the picks for that many, pays that much with the cast, refuses an X above the most without a change, can be backed out of at the question with no trace, is offered when X = 0 can be paid, resolves reading the X it was cast with, and replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{lands, pay};
use crate::common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, replay};
use mtg_testkit::cards::{forest, free_creature, island, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// What the required additional cost of the Pitcher takes.
#[derive(Debug, Clone, Copy)]
enum Pays {
    Life,
    Sacrifice,
    Discard,
    Exile,
}

impl Pays {
    /// The cost as a card file holds it.
    const fn cost(self) -> &'static str {
        match self {
            Pays::Life => r#"[{"PayLife":"X"}]"#,
            Pays::Sacrifice => r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}],"count":"X"}}]"#,
            Pays::Discard => r#"[{"Discard":{"count":"X"}}]"#,
            Pays::Exile => r#"[{"ExileFromGraveyard":{"filter":[],"count":"X"}}]"#,
        }
    }

    /// A table where seat 0 can give `amount` of it and cast the Pitcher for one blue mana.
    fn scenario(self, seats: usize, amount: u32) -> Scenario {
        let cards = |count: u32| -> Vec<Arc<CardDefinition>> {
            (0..count)
                .map(|n| free_creature(&format!("Card {n}"), 1, 1))
                .collect()
        };
        let wolves = match self {
            Pays::Sacrifice => amount,
            _ => 0,
        };
        let mut table = Scenario::new(seats)
            .library(0, (0..8).map(|_| forest()))
            .battlefield(
                0,
                lands(1, island)
                    .into_iter()
                    .chain((0..wolves).map(|n| ready(free_creature(&format!("Wolf {n}"), 2, 2)))),
            );
        for seat in 1..seats {
            table = table.library(seat, (0..8).map(|_| forest()));
        }
        match self {
            Pays::Life => table
                .hand(0, [pitcher(self.cost())])
                .life(0, i32::try_from(amount).unwrap()),
            Pays::Sacrifice => table.hand(0, [pitcher(self.cost())]),
            Pays::Discard => table.hand(
                0,
                std::iter::once(pitcher(self.cost())).chain(cards(amount)),
            ),
            Pays::Exile => table
                .hand(0, [pitcher(self.cost())])
                .graveyard(0, cards(amount)),
        }
    }

    /// What has been paid at the table that `scenario(_, 3)` made.
    fn paid(self, driver: &Driver) -> usize {
        let state = driver.state();
        match self {
            Pays::Life => 3 - usize::try_from(state.player(pid(0)).unwrap().life()).unwrap(),
            Pays::Sacrifice | Pays::Discard => state.zone(Zone::Graveyard(pid(0))).len(),
            Pays::Exile => state.zone(Zone::Exile).len(),
        }
    }
}

/// A blue instant: "As an additional cost to cast this spell, <cost>. Draw X cards."
fn pitcher(cost: &str) -> Arc<CardDefinition> {
    let additional = format!(r#"{{"AdditionalCost":{{"cost":{cost},"required":true}}}}"#);
    let spell = r#"{"Spell":{"effects":[{"Draw":{"count":"X"}}]}}"#;
    Arc::new(
        CardDefinition::new("Pitcher")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{U}").unwrap())
            .with_rules(serde_json::from_str(&additional).unwrap())
            .with_rules(serde_json::from_str(spell).unwrap()),
    )
}

/// The range the pending number question offers.
fn range(driver: &Driver) -> (u32, Option<u32>, String, bool) {
    let DecisionKind::ChooseNumber(options) = &driver.decision().unwrap().kind else {
        panic!("a number was expected, not {:?}", driver.decision());
    };
    (
        options.min,
        options.max,
        options.text.clone(),
        options.can_cancel,
    )
}

fn say(driver: &mut Driver, x: u32) -> Result<(), ScriptError> {
    driver
        .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
        .map(|_| ())
}

/// Picks the first of the cards the pending question offers, as many as it asks.
fn pick_first(driver: &mut Driver) -> Result<(), ScriptError> {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("cards were expected, not {:?}", driver.decision());
    };
    let picks = options
        .from
        .iter()
        .take(usize::try_from(options.min).unwrap())
        .copied()
        .collect();
    driver
        .answer(Answer::ChooseCards(CardsChoice(picks)))
        .map(|_| ())
}

fn picks(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(
        driver.decision().map(|decision| &decision.kind),
        Ok(DecisionKind::ChooseCards(_))
    ) {
        pick_first(driver)?;
    }
    Ok(())
}

fn hash(driver: &Driver) -> (usize, u64) {
    (driver.engine().events().len(), driver.engine().state_hash())
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..100 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn replays(setup: &GameSetup, driver: &Driver) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
fn x_is_announced_from_nothing_to_the_most_the_caster_can_give_and_that_much_is_paid_and_drawn(
    #[values(Pays::Life, Pays::Sacrifice, Pays::Discard, Pays::Exile)] pays: Pays,
    #[values(0, 1, 2, 3)] x: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = pays.scenario(seats, 3);
    let setup: GameSetup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    let kept = driver.state().zone(Zone::Hand(pid(0))).len() - 1;
    driver.begin_cast("Pitcher")?;
    assert_eq!(
        range(&driver),
        (0, Some(3), "Choose a value for X.".to_owned(), true)
    );
    say(&mut driver, x)?;
    picks(&mut driver)?;
    assert_eq!(
        driver.pay_options()?.cost,
        ManaCost::parse("{U}").unwrap(),
        "the X of a part adds no mana"
    );
    pay(&mut driver)?;
    assert_eq!(pays.paid(&driver), usize::try_from(x).unwrap());
    // A player may pay all their life (CR 119.4) and loses the game for it.
    if matches!(pays, Pays::Life) && x == 3 {
        assert!(!driver.state().player(pid(0)).unwrap().is_in_game());
        return Ok(());
    }
    settle(&mut driver)?;
    let discarded = if matches!(pays, Pays::Discard) { x } else { 0 };
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        kept - usize::try_from(discarded).unwrap() + usize::try_from(x).unwrap(),
        "the spell drew the X it was cast with"
    );
    replays(&setup, &driver);
    Ok(())
}

#[rstest]
fn an_x_above_the_most_is_refused_without_a_change_and_the_question_can_be_backed_out_of(
    #[values(Pays::Life, Pays::Sacrifice, Pays::Discard, Pays::Exile)] pays: Pays,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(pays.scenario(seats, 3))?;
    let before = hash(&driver);
    driver.begin_cast("Pitcher")?;
    assert!(say(&mut driver, 4).is_err(), "four is more than there is");
    assert_eq!(range(&driver).1, Some(3), "the question is still asked");
    driver.answer(Answer::ChooseNumber(NumberChoice::Cancel))?;
    assert_eq!(hash(&driver), before, "nothing was logged");
    Ok(())
}

#[rstest]
fn the_picks_are_backed_out_of_with_no_trace_and_name_what_is_asked(
    #[values(Pays::Sacrifice, Pays::Discard, Pays::Exile)] pays: Pays,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(pays.scenario(2, 3))?;
    let before = hash(&driver);
    driver.begin_cast("Pitcher")?;
    say(&mut driver, 2)?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("the picks were expected");
    };
    assert_eq!((options.min, options.max), (2, 2));
    assert!(options.can_cancel);
    assert!(
        !options
            .from
            .iter()
            .any(|id| name_of(driver.state(), *id) == "Pitcher"),
        "the spell is not offered for its own cost"
    );
    driver.answer(Answer::ChooseCards(CardsChoice(Vec::new())))?;
    assert_eq!(hash(&driver), before, "nothing was logged");
    Ok(())
}

#[rstest]
#[case::two_life(Pays::Life, 2)]
#[case::two_creatures(Pays::Sacrifice, 2)]
#[case::two_cards(Pays::Discard, 2)]
#[case::two_graveyard_cards(Pays::Exile, 2)]
fn the_most_is_what_the_caster_has(
    #[case] pays: Pays,
    #[case] amount: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(pays.scenario(seats, amount))?;
    driver.begin_cast("Pitcher")?;
    assert_eq!(range(&driver).1, Some(amount));
    Ok(())
}

#[rstest]
fn with_nothing_to_give_the_spell_is_offered_for_x_0_and_nothing_is_asked(
    #[values(Pays::Sacrifice, Pays::Discard, Pays::Exile)] pays: Pays,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(pays.scenario(seats, 0))?;
    driver.begin_cast("Pitcher")?;
    assert!(
        !matches!(
            driver.decision().unwrap().kind,
            DecisionKind::ChooseNumber(_) | DecisionKind::ChooseCards(_)
        ),
        "{:?}",
        driver.decision()
    );
    pay(&mut driver)?;
    let before = driver.state().zone(Zone::Hand(pid(0))).len();
    settle(&mut driver)?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), before);
    Ok(())
}

/// Seat 0 holds Skeletal Scrying and `swamps` Swamps, with `graveyard` cards in the graveyard.
fn scrying(seats: usize, swamps: usize, graveyard: u32) -> Scenario {
    let mut table = Scenario::new(seats)
        .hand_named(0, ["Skeletal Scrying"])
        .battlefield(0, lands(swamps, swamp))
        .graveyard(
            0,
            (0..graveyard).map(|n| free_creature(&format!("Bone {n}"), 1, 1)),
        )
        .library(0, (0..8).map(|_| forest()));
    for seat in 1..seats {
        table = table.library(seat, (0..8).map(|_| forest()));
    }
    table
}

#[rstest]
#[case::the_graveyard_is_the_limit(4, 2, 2)]
#[case::the_mana_is_the_limit(2, 5, 1)]
#[case::both(3, 2, 2)]
fn skeletal_scrying_asks_x_up_to_the_smaller_of_the_graveyard_and_the_mana_left_after_the_black(
    #[case] swamps: usize,
    #[case] graveyard: u32,
    #[case] most: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scrying(seats, swamps, graveyard))?;
    driver.begin_cast("Skeletal Scrying")?;
    assert_eq!(range(&driver).1, Some(most));
    Ok(())
}

#[rstest]
fn skeletal_scrying_exiles_x_cards_pays_x_and_black_draws_x_and_loses_x_life(
    #[values(0, 1, 2, 3)] x: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = scrying(seats, 4, 3);
    let setup: GameSetup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.begin_cast("Skeletal Scrying")?;
    say(&mut driver, x)?;
    picks(&mut driver)?;
    assert_eq!(
        driver.pay_options()?.cost,
        ManaCost::parse(&format!("{{{x}}}{{B}}")).unwrap(),
        "X generic mana and the black"
    );
    pay(&mut driver)?;
    assert_eq!(
        driver.state().zone(Zone::Exile).len(),
        usize::try_from(x).unwrap()
    );
    settle(&mut driver)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        usize::try_from(x).unwrap()
    );
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        life - i32::try_from(x).unwrap()
    );
    replays(&setup, &driver);
    Ok(())
}

/// A table for Toxic Deluge: the caster has a 4/4, every opponent a 2/2.
fn deluge(seats: usize) -> Scenario {
    let mut table = Scenario::new(seats)
        .hand_named(0, ["Toxic Deluge"])
        .battlefield(
            0,
            lands(3, swamp)
                .into_iter()
                .chain([ready(free_creature("Giant", 4, 4))]),
        );
    for seat in 1..seats {
        table = table.battlefield(seat, [ready(free_creature(&format!("Cub {seat}"), 2, 2))]);
    }
    table
}

#[rstest]
#[case::nothing(0, 20)]
#[case::one(1, 19)]
#[case::the_small_ones_die(2, 18)]
#[case::three(3, 17)]
#[case::all_die(4, 16)]
fn toxic_deluge_pays_x_life_and_every_creature_gets_minus_x_until_the_end_of_the_turn(
    #[case] x: u32,
    #[case] life: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = deluge(seats);
    let setup: GameSetup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Toxic Deluge")?;
    // The most is all the life the caster has (CR 119.4).
    assert_eq!(range(&driver).1, Some(20));
    say(&mut driver, x)?;
    pay(&mut driver)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life);
    settle(&mut driver)?;
    let survivors = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| name.starts_with("Cub") || *name == "Giant")
        .count();
    // The cubs die from 2; the giant, with 4 toughness, only from 4.
    let dying = (seats - 1) * usize::from(x >= 2) + usize::from(x >= 4);
    assert_eq!(survivors, seats - dying);
    replays(&setup, &driver);
    Ok(())
}

#[test]
fn toxic_deluge_is_cast_for_x_life_in_the_log_and_its_x_is_the_one_the_spell_resolves_with()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(deluge(3))?;
    driver.begin_cast("Toxic Deluge")?;
    say(&mut driver, 2)?;
    let events = pay(&mut driver)?;
    let paid = events
        .iter()
        .position(|event| matches!(event, Event::LifePaid { amount: 2, .. }))
        .expect("the life is paid");
    let cast = events
        .iter()
        .position(|event| matches!(event, Event::SpellCast { .. }))
        .expect("the spell is cast");
    assert!(paid < cast, "the cost is paid before the spell is cast");
    let Event::SpellCast { choices, .. } = &events[cast] else {
        unreachable!()
    };
    assert_eq!(choices.x, Some(2));
    Ok(())
}

#[test]
fn bot_games_with_the_x_cost_cards_pay_the_x_they_announce_on_every_cast_and_keep_every_invariant()
{
    use crate::common::seeds::{for_each_seed, fuzz_games};
    use mtg_engine::events::ZoneCause;
    use mtg_testkit::{PlayOptions, play_game_with, x_cost_fuzz_setup};
    // Casts of Toxic Deluge and Skeletal Scrying, and how many of them had an X above 0.
    let seen = std::sync::Mutex::new([[0u64; 2]; 2]);
    for_each_seed(0..fuzz_games(16), |seed| {
        for players in [2, 3, 4] {
            let setup = x_cost_fuzz_setup(players, seed);
            let outcome = play_game_with(setup, seed, PlayOptions::default())
                .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = replay(&x_cost_fuzz_setup(players, seed), &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            for (at, event) in outcome.events.iter().enumerate() {
                let Event::SpellCast {
                    object, choices, ..
                } = event
                else {
                    continue;
                };
                let name = replayed
                    .object(*object)
                    .map(|spell| spell.card().name.clone());
                let x = choices.x.unwrap_or(0);
                let width = usize::try_from(x).unwrap() + 8;
                let window = &outcome.events[at.saturating_sub(width)..at];
                match name.as_deref() {
                    Some("Toxic Deluge") => {
                        assert!(
                            x == 0
                                || window.iter().any(|event| matches!(
                                    event,
                                    Event::LifePaid { amount, .. } if *amount == x
                                )),
                            "seed {seed}, {players} seats: Toxic Deluge cast for X = {x} paid no {x} life"
                        );
                        seen[0][0] += 1;
                        seen[0][1] += u64::from(x > 0);
                    }
                    Some("Skeletal Scrying") => {
                        let exiled = window
                            .iter()
                            .filter(|event| {
                                matches!(
                                    event,
                                    Event::ZoneChanged {
                                        cause: ZoneCause::Exiled,
                                        ..
                                    }
                                )
                            })
                            .count();
                        assert_eq!(
                            exiled,
                            usize::try_from(x).unwrap(),
                            "seed {seed}, {players} seats: Skeletal Scrying cast for X = {x}"
                        );
                        seen[1][0] += 1;
                        seen[1][1] += u64::from(x > 0);
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    assert!(seen[0][0] > 0 && seen[1][0] > 0, "both cards were cast");
    assert!(seen[0][1] > 0, "Toxic Deluge was cast for more than 0");
    assert!(seen[1][1] > 0, "Skeletal Scrying was cast for more than 0");
}
