//! Goal: costs paid with something other than mana where a player chooses: an Aura's "its controller may exile three
//! cards from their graveyard (or sacrifice a permanent) for that player to ignore this effect until end of turn"
//! (`GameRule::PayToIgnoreBy`) is offered to the enchanted creature's controller alone, takes the cards or permanent
//! picked, and frees that creature to attack this turn; "you may have an opponent gain 3 life" in place of the mana
//! cost or as a splice cost (`CostItem::OpponentGainsLife`) gives the life to the opponent the caster picks, and "each
//! blocked creature" (`Pred::Blocked`) is an attacker a creature blocked, at every table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, ChoiceOption, CostsChoice, OptionChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, forest, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::settle;
use crate::common::{names, pid};

/// The sources the deciding player may pay to ignore now.
fn ignorable(driver: &Driver) -> Vec<ObjectId> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.ignorable.iter().map(|o| o.object).collect(),
        _ => Vec::new(),
    }
}

/// Whether the deciding player may attack with the Bear now.
/// Passes to the declaration of attackers; a turn with no creature able to attack has none, and the Bear may not.
fn bear_may_attack(driver: &mut Driver) -> bool {
    let bear = driver.id_of("Bear").unwrap();
    for _ in 0..20 {
        match &driver.decision().unwrap().kind {
            DecisionKind::DeclareAttackers(options) => {
                return options.attackers.iter().any(|option| option.object == bear);
            }
            DecisionKind::Priority(_) if driver.state().turn().step() == Step::Main2 => {
                return false;
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the turn never reached its attacks")
}

/// Seat 1 is active with a Bear enchanted by `aura` (seat 0's), two Forests and `cards` in its graveyard.
fn cursed(seats: usize, aura: &str, cards: usize) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(
            1,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(forest()),
                ready(forest()),
            ],
        )
        .battlefield(0, [attached_to(ready(card(aura).unwrap()), 1, 0)])
        .graveyard(1, (0..cards).map(|_| forest()).collect::<Vec<_>>());
    Driver::from_scenario(scenario)
}

#[rstest]
fn the_creatures_controller_exiles_three_cards_to_attack(
    #[values(2, 3, 4)] seats: usize,
    #[values(2, 3, 4)] cards: usize,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    let mut driver = cursed(seats, "Lost in Thought", cards)?;
    let aura = driver.id_of("Lost in Thought")?;
    if cards < 3 {
        assert!(ignorable(&driver).is_empty(), "too few cards to exile");
    } else {
        assert_eq!(ignorable(&driver), [aura]);
    }
    let paid = pays && cards >= 3;
    if paid {
        driver.answer(Answer::Priority(PriorityAction::IgnoreEffect(aura)))?;
        if let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
            assert_eq!((options.min, options.max), (3, 3));
            let picked = options.from[..3].to_vec();
            driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
        }
        assert_eq!(
            names(driver.state(), Zone::Graveyard(pid(1))).len(),
            cards - 3
        );
        assert_eq!(names(driver.state(), Zone::Exile).len(), 3);
        assert!(
            ignorable(&driver).is_empty(),
            "paid once is enough this turn"
        );
    }
    assert_eq!(bear_may_attack(&mut driver), paid);
    Ok(())
}

#[rstest]
fn only_the_enchanted_creatures_controller_may_pay(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = cursed(seats, "Lost in Thought", 3)?;
    let aura = driver.id_of("Lost in Thought")?;
    // Seat 1 casts nothing and passes; the next player with priority is another seat, who is not offered the payment.
    driver.pass()?;
    assert_ne!(driver.decision()?.player, pid(1));
    assert!(!ignorable(&driver).contains(&aura));
    Ok(())
}

#[rstest]
fn a_sacrificed_permanent_frees_the_creature(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = cursed(seats, "Volrath's Curse", 0)?;
    let aura = driver.id_of("Volrath's Curse")?;
    assert_eq!(ignorable(&driver), [aura]);
    driver.answer(Answer::Priority(PriorityAction::IgnoreEffect(aura)))?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("the permanent to sacrifice is picked");
    };
    let bear = driver.id_of("Bear")?;
    assert!(options.from.contains(&bear), "any permanent of theirs");
    let land = *options.from.iter().find(|id| **id != bear).unwrap();
    driver.answer(Answer::ChooseCards(CardsChoice(vec![land])))?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(1))), ["Forest"]);
    assert!(bear_may_attack(&mut driver));
    Ok(())
}

#[rstest]
fn the_chosen_opponent_gains_the_life(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let chosen = seats - 1;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(forest()), ready(free_creature("Bear", 2, 2))])
        .hand(0, [card("Invigorate").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let bear = driver.id_of("Bear")?;
    driver.begin_cast("Invigorate")?;
    for _ in 0..10 {
        let answer = match &driver.decision()?.kind {
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| match option {
                        ChoiceOption::Player(player) => *player == pid(chosen),
                        ChoiceOption::Label(label) => label.contains("gain 3 life"),
                        _ => false,
                    })
                    .expect("the way or the opponent");
                Answer::ChooseOption(OptionChoice(at))
            }
            DecisionKind::ChooseTargets(options) => {
                let target = *options
                    .legal
                    .iter()
                    .find(|target| format!("{target:?}").contains(&format!("{bear:?}")))
                    .expect("the Bear");
                Answer::ChooseTargets(TargetChoice::Targets(vec![target]))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => break,
        };
        driver.answer(answer)?;
    }
    settle(&mut driver)?;
    for seat in 0..seats {
        let life = driver.state().player(pid(seat)).unwrap().life();
        assert_eq!(life, if seat == chosen { 23 } else { 20 }, "seat {seat}");
    }
    assert_eq!(
        characteristics(driver.state(), bear).unwrap().power,
        Some(6),
        "the spell resolved"
    );
    Ok(())
}

/// An Arcane instant for {R} that deals 1 damage to any target.
fn arcane_bolt() -> std::sync::Arc<mtg_cards::CardDefinition> {
    std::sync::Arc::new((*burn("Arcane Bolt", 1)).clone().with_subtype("Arcane"))
}

#[rstest]
fn a_splice_cost_gives_the_chosen_opponent_life(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let chosen = seats - 1;
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(mountain())])
        .hand(0, [arcane_bolt(), card("Roar of Jukai").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Arcane Bolt")?;
    for _ in 0..10 {
        let answer = match &driver.decision()?.kind {
            DecisionKind::ChooseCosts(_) => Answer::ChooseCosts(CostsChoice::Costs(vec![0])),
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![driver.target_player(0)]))
            }
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| *option == ChoiceOption::Player(pid(chosen)))
                    .expect("the opponent");
                Answer::ChooseOption(OptionChoice(at))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => break,
        };
        driver.answer(answer)?;
    }
    settle(&mut driver)?;
    for seat in 0..seats {
        let life = driver.state().player(pid(seat)).unwrap().life();
        let expected = match seat {
            0 => 19,
            _ if seat == chosen => 25,
            _ => 20,
        };
        assert_eq!(life, expected, "seat {seat}");
    }
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Roar of Jukai"]);
    Ok(())
}

#[rstest]
fn only_blocked_creatures_get_the_bonus(
    #[values(2, 3)] seats: usize,
    #[values(true, false)] forest_there: bool,
) -> Result<(), ScriptError> {
    let land = if forest_there { forest() } else { mountain() };
    let lands = if forest_there { forest() } else { mountain() };
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Runner", 2, 2)),
                ready(land),
                ready(lands),
                ready(forest_or_mountain(forest_there)),
            ],
        )
        .battlefield(1, [ready(free_creature("Wall", 0, 5))])
        .hand(0, [green_roar(forest_there)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Bear", 1), ("Runner", 1)])?;
    for _ in 0..20 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.pass()?;
    }
    driver.block(&[("Wall", "Bear")])?;
    for _ in 0..20 {
        if driver.decision()?.player == pid(0)
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            break;
        }
        driver.answer_passively()?;
    }
    driver.cast("Roar of Jukai")?;
    for _ in 0..5 {
        if !matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            break;
        }
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    settle(&mut driver)?;
    let power = |name: &str| {
        characteristics(driver.state(), driver.id_of(name).unwrap())
            .unwrap()
            .power
    };
    assert_eq!(power("Bear"), Some(if forest_there { 4 } else { 2 }));
    assert_eq!(
        power("Runner"),
        Some(2),
        "an unblocked attacker gets nothing"
    );
    assert_eq!(power("Wall"), Some(0), "a blocker is no blocked creature");
    Ok(())
}

/// A Forest, or a Mountain for the table without one.
fn forest_or_mountain(forest_there: bool) -> std::sync::Arc<mtg_cards::CardDefinition> {
    if forest_there { forest() } else { mountain() }
}

/// Roar of Jukai, or for the table without a Forest the card at {2}{R} so the Mountains pay for it.
fn green_roar(forest_there: bool) -> std::sync::Arc<mtg_cards::CardDefinition> {
    let roar = card("Roar of Jukai").unwrap();
    if forest_there {
        return roar;
    }
    std::sync::Arc::new(
        (*roar)
            .clone()
            .with_mana_cost(mtg_testkit::cards::cost(2, &[mtg_core::color::Color::Red])),
    )
}
