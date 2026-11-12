//! Goal: the mechanics added in round 3e play as the Comprehensive Rules say: craft (CR 702.167) exiles this permanent and the materials from the battlefield and the graveyard and returns the card transformed; assist (CR 702.132) lets another player pay generic mana of the cost, asked of that player with a default of nothing and paid from the sources that player picks, the mana counting as spent to cast the spell; prototype (CR 702.160, 718) gives the spell and the permanent the prototype mana cost, colours, power and toughness, only when cast for the prototype cost; sneak is cast in the declare blockers step by returning an unblocked attacker and the creature enters tapped and attacking what that attacker attacked, replaying to the same state; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, lands, pay};
use crate::common::{names, on_battlefield, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::alt_riders::{alt_cast, sneak_riders};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::definition::Layout;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_cards::value::Amount;
use mtg_cards::value::CastWay;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::PresetPermanent;
use mtg_engine::choose::NumberChoice;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PayOptions};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn table(
    seats: usize,
    hand: Vec<Arc<CardDefinition>>,
    permanents: Vec<PresetPermanent>,
) -> Scenario {
    (1..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, hand)
            .battlefield(0, permanents),
        |scenario, seat| scenario.hand(seat, [forest()]),
    )
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Casts `name` for the way whose label starts with `way`, answering what is asked before the payment passively.
fn cast_for(driver: &mut Driver, name: &str, way: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, way)?;
    }
    for _ in 0..10 {
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    pay(driver)?;
    settle(driver)
}

/// A colourless 6/6 for {6} with prototype {1}{R} 2/1.
fn prototype_beast() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Proto Beast")
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse("{6}").unwrap())
            .with_power_toughness(6, 6)
            .with_rules(Ability::Keyword(Keyword::Prototype {
                cost: Cost::mana("{1}{R}".parse().unwrap()),
                power: 2,
                toughness: 1,
            })),
    )
}

#[rstest]
fn prototype_gives_the_permanent_its_cost_colour_and_size(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, vec![prototype_beast()], lands(2, mountain)))?;
    cast_for(&mut driver, "Proto Beast", "Prototype")?;
    let id = on_battlefield(driver.state(), "Proto Beast").unwrap();
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.cast_way(), Some(CastWay::Prototype));
    assert_eq!(object.mana_value(), 2);
    let seen = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert_eq!((seen.power, seen.toughness), (Some(2), Some(1)));
    assert_eq!(seen.colors.iter().collect::<Vec<_>>(), [Color::Red]);
    Ok(())
}

#[rstest]
fn the_printed_cost_keeps_the_printed_card(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, vec![prototype_beast()], lands(6, mountain)))?;
    cast_for(&mut driver, "Proto Beast", "Cast for")?;
    let id = on_battlefield(driver.state(), "Proto Beast").unwrap();
    let seen = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert_eq!((seen.power, seen.toughness), (Some(6), Some(6)));
    assert!(seen.colors.is_empty());
    assert_eq!(driver.state().object(id).unwrap().mana_value(), 6);
    Ok(())
}

// Sneak: cast for its cost in the declare blockers step by returning an unblocked attacker.

/// A 2/2 for {3}{B} with sneak {B}.
fn sneaker() -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Sneaky Rogue")
        .with_type(CardType::Creature)
        .with_mana_cost(ManaCost::parse("{3}{B}").unwrap())
        .with_power_toughness(2, 2)
        .with_rules(alt_cast(CastWay::Sneak, "{B}".parse().unwrap()));
    for rider in sneak_riders() {
        card = card.with_rules(rider);
    }
    Arc::new(card)
}

/// Seat 0 holds the sneaker and has Grizzly Bears and `lands`; seat 1 has a Black Knight.
fn sneak_table(seats: usize, lands: Vec<PresetPermanent>) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .hand(0, [sneaker()])
            .battlefield(
                0,
                lands
                    .into_iter()
                    .chain([ready(mtg_testkit::corpus::card("Grizzly Bears").unwrap())]),
            )
            .battlefield_named(1, ["Black Knight"]),
        |scenario, seat| scenario.library(seat, (0..8).map(|_| forest())),
    )
}

/// Seat 0 attacks seat `defender` with Grizzly Bears and the game stops in the declare blockers step; `blocked` has the
/// defender block it.
fn to_blockers(driver: &mut Driver, defender: usize, blocked: bool) -> Result<(), ScriptError> {
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Grizzly Bears", defender)])?;
    loop {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            if blocked && driver.decision()?.player == pid(defender) {
                driver.block(&[("Black Knight", "Grizzly Bears")])?;
            } else {
                driver.answer(Answer::DeclareBlockers(Vec::new()))?;
            }
        } else if driver.state().turn().step() == Step::DeclareBlockers
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        } else {
            driver.answer_passively()?;
        }
    }
}

fn offered(driver: &Driver, name: &str) -> Result<bool, ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        return Ok(false);
    };
    Ok(options.castable.iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    }))
}

/// Casts the sneaker for its sneak cost, returning whatever is offered to return.
fn sneak(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.begin_cast("Sneaky Rogue")?;
    for _ in 0..10 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(_) => choose_way(driver, "Sneak")?,
            DecisionKind::ChooseCards(options) => {
                let pick = options.from.clone();
                driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(pick)))?;
            }
            DecisionKind::PayMana(_) => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    pay(driver)?;
    settle(driver)
}

#[rstest]
fn a_sneaked_creature_enters_tapped_and_attacking_what_the_returned_one_attacked(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let defender = seats - 1;
    let mut driver = Driver::from_scenario(sneak_table(seats, vec![ready(swamp())]))?;
    to_blockers(&mut driver, defender, false)?;
    assert!(offered(&driver, "Sneaky Rogue")?);
    sneak(&mut driver)?;
    let id = on_battlefield(driver.state(), "Sneaky Rogue").unwrap();
    let object = driver.state().object(id).unwrap();
    assert!(object.permanent().unwrap().tapped());
    assert_eq!(object.cast_way(), Some(CastWay::Sneak));
    assert_eq!(
        driver
            .state()
            .combat()
            .attackers()
            .iter()
            .map(|a| (a.object(), a.target()))
            .collect::<Vec<_>>(),
        [(id, AttackTarget::Player(pid(defender)))]
    );
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Grizzly Bears"]);
    let replayed = mtg_engine::replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    driver.idle_until(1, Step::End)?;
    settle(&mut driver)?;
    assert_eq!(driver.state().player(pid(defender)).unwrap().life(), 18);
    Ok(())
}

#[rstest]
fn sneak_is_not_offered_when_the_attacker_is_blocked(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(sneak_table(seats, vec![ready(swamp())]))?;
    to_blockers(&mut driver, 1, true)?;
    assert!(!offered(&driver, "Sneaky Rogue")?);
    Ok(())
}

#[rstest]
fn sneak_is_not_offered_outside_the_declare_blockers_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(sneak_table(seats, vec![ready(swamp())]))?;
    driver.idle_until(1, Step::Main1)?;
    assert!(!offered(&driver, "Sneaky Rogue")?);
    Ok(())
}

// Assist (CR 702.132): another player pays generic mana of the spell.

/// A 4/4 for {4}{B} with assist.
fn helped_giant() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Helped Giant")
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse("{4}{B}").unwrap())
            .with_power_toughness(4, 4)
            .with_rules(Ability::Keyword(Keyword::Assist)),
    )
}

/// Seat 0 holds the giant and controls `mine` Swamps; seat 1 controls `theirs` Forests.
fn assist_table(seats: usize, mine: usize, theirs: usize) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [helped_giant()])
            .battlefield(0, lands(mine, swamp))
            .battlefield(1, lands(theirs, forest)),
        |scenario, seat| scenario.library(seat, (0..8).map(|_| forest())),
    )
}

fn untapped_lands(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.controller() == pid(seat) && o.card().name != "Helped Giant")
        .filter(|o| o.permanent().is_some_and(|p| !p.tapped()))
        .count()
}

/// Casts the giant, answering the assist questions with `helper` (0 is no one, 1 the first helper) and `amount`.
fn cast_assisted(driver: &mut Driver, helper: usize, amount: u32) -> Result<(), ScriptError> {
    cast_assisted_with(driver, helper, amount, |_| PayChoice::Auto)
}

/// [`cast_assisted`] with the helper paying as `payment` says for the options they are offered.
fn cast_assisted_with(
    driver: &mut Driver,
    helper: usize,
    amount: u32,
    payment: impl Fn(&PayOptions) -> PayChoice,
) -> Result<(), ScriptError> {
    driver.begin_cast("Helped Giant")?;
    for _ in 0..10 {
        let decision = driver.decision()?;
        match &decision.kind {
            DecisionKind::ChooseOption(_) => {
                assert_eq!(decision.player, pid(0));
                driver.answer(Answer::ChooseOption(mtg_engine::choose::OptionChoice(
                    helper,
                )))?;
            }
            DecisionKind::ChooseNumber(options) => {
                assert_eq!(decision.player, pid(1), "the helper is asked");
                assert!(!options.can_cancel, "the helper cannot back the cast out");
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(amount)))?;
            }
            DecisionKind::PayMana(options) if decision.player != pid(0) => {
                assert!(!options.can_cancel, "the helper cannot back the cast out");
                assert_eq!(options.cost.generic(), amount);
                let choice = payment(options);
                driver.answer(Answer::PayMana(choice))?;
            }
            DecisionKind::PayMana(_) => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    pay(driver)?;
    settle(driver)
}

#[rstest]
fn the_helper_pays_generic_mana_and_the_caster_the_rest(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(assist_table(seats, 1, 4))?;
    cast_assisted(&mut driver, 1, 4)?;
    assert!(on_battlefield(driver.state(), "Helped Giant").is_some());
    assert_eq!(untapped_lands(&driver, 1), 0, "the helper paid four");
    assert_eq!(untapped_lands(&driver, 0), 0, "the caster paid the black");
    let replayed = mtg_engine::replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::two_of_four(2, 3)]
#[case::none(0, 5)]
fn the_caster_pays_what_the_helper_does_not(
    #[case] amount: u32,
    #[case] caster_taps: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(assist_table(seats, 5, 4))?;
    cast_assisted(&mut driver, 1, amount)?;
    assert!(on_battlefield(driver.state(), "Helped Giant").is_some());
    assert_eq!(5 - untapped_lands(&driver, 0), caster_taps);
    assert_eq!(4 - untapped_lands(&driver, 1), amount as usize);
    Ok(())
}

/// Seat 0 holds the giant and controls five Swamps; seat 1 controls two Forests and two Mountains.
fn mixed_table(seats: usize) -> Scenario {
    let mut helper = lands(2, forest);
    helper.extend(lands(2, mountain));
    (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [helped_giant()])
            .battlefield(0, lands(5, swamp))
            .battlefield(1, helper),
        |scenario, seat| scenario.library(seat, (0..8).map(|_| forest())),
    )
}

fn tapped_of(driver: &Driver, seat: usize, name: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.controller() == pid(seat) && o.card().name == name)
        .filter(|o| {
            o.permanent()
                .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count()
}

#[rstest]
fn the_helper_chooses_which_sources_pay_and_the_mana_counts_as_spent_to_cast_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // CR 702.132a, 601.2h: the helper pays with the Mountains they pick, and red is among the mana spent to cast it.
    let mut driver = Driver::from_scenario(mixed_table(seats))?;
    cast_assisted_with(&mut driver, 1, 2, |options| {
        let taps = options
            .sources
            .iter()
            .filter(|o| o.produces == mtg_core::mana::ManaType::Red)
            .map(|o| o.source)
            .collect();
        PayChoice::Manual(mtg_engine::mana::Payment {
            taps,
            ..mtg_engine::mana::Payment::default()
        })
    })?;
    let id = on_battlefield(driver.state(), "Helped Giant").unwrap();
    assert_eq!(tapped_of(&driver, 1, "Mountain"), 2, "the picked sources");
    assert_eq!(tapped_of(&driver, 1, "Forest"), 0);
    let spent = driver.state().object(id).unwrap().mana_spent().to_vec();
    let red = spent
        .iter()
        .filter(|m| **m == mtg_core::mana::ManaType::Red)
        .count();
    assert_eq!((red, spent.len()), (2, 5), "{spent:?}");
    Ok(())
}

#[rstest]
fn no_one_is_a_choice_and_the_cast_goes_on(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(assist_table(seats, 5, 4))?;
    cast_assisted(&mut driver, 0, 0)?;
    assert!(on_battlefield(driver.state(), "Helped Giant").is_some());
    assert_eq!(untapped_lands(&driver, 1), 4);
    Ok(())
}

#[rstest]
fn an_idle_helper_pays_nothing_by_default(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(assist_table(seats, 5, 4))?;
    driver.begin_cast("Helped Giant")?;
    driver.answer(Answer::ChooseOption(mtg_engine::choose::OptionChoice(1)))?;
    let asked = driver.decision()?;
    assert_eq!(asked.player, pid(1));
    // The default of a seat whose clock ran out: the lowest number.
    let DecisionKind::ChooseNumber(options) = &asked.kind else {
        panic!("the helper is asked a number");
    };
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(options.min)))?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    assert_eq!(untapped_lands(&driver, 1), 4);
    assert_eq!(untapped_lands(&driver, 0), 0);
    Ok(())
}

#[rstest]
#[case::with_a_helper(4, true)]
#[case::nobody_has_mana(0, false)]
fn a_spell_the_caster_cannot_pay_alone_is_offered_when_a_player_can_help(
    #[case] theirs: usize,
    #[case] offered_now: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(assist_table(seats, 1, theirs))?;
    assert_eq!(offered(&driver, "Helped Giant")?, offered_now);
    Ok(())
}

// Craft (CR 702.167): exile this permanent and materials, return the card transformed.

/// "Craft with one or more creatures {1}{B}" on an artifact whose back face is a 3/3 creature.
fn relic() -> Arc<CardDefinition> {
    let craft = Ability::Keyword(Keyword::Craft(Cost(vec![
        CostItem::Mana("{1}{B}".parse().unwrap()),
        CostItem::Craft {
            filter: Selector(vec![Pred::Type(CardType::Creature)]),
            count: Amount::X,
            least: None,
        },
    ])));
    let front = CardDefinition::new("Relic Engine")
        .with_type(CardType::Artifact)
        .with_rules(craft.clone());
    let back = CardDefinition::new("Relic Beast")
        .with_type(CardType::Creature)
        .with_power_toughness(3, 3);
    Arc::new(
        CardDefinition::new("Relic Engine")
            .with_type(CardType::Artifact)
            .with_rules(craft)
            .with_faces(Layout::Transform, vec![front, back]),
    )
}

/// Seat 0 controls the relic, two Swamps and `bears` Grizzly Bears, with `graveyard` more in the graveyard.
fn craft_table(seats: usize, bears: usize, graveyard: usize) -> Scenario {
    let bear = || mtg_testkit::corpus::card("Grizzly Bears").unwrap();
    (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                lands(2, swamp)
                    .into_iter()
                    .chain([ready(relic())])
                    .chain((0..bears).map(|_| ready(bear()))),
            )
            .graveyard(0, (0..graveyard).map(|_| bear())),
        |scenario, seat| scenario.library(seat, (0..8).map(|_| forest())),
    )
}

/// Activates the craft with `x` materials, taking the first offered cards.
fn craft(driver: &mut Driver, x: u32) -> Result<(), ScriptError> {
    driver.begin_activation("Relic Engine")?;
    for _ in 0..10 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let pick = options.from.iter().copied().take(x as usize).collect();
                driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(pick)))?;
            }
            DecisionKind::PayMana(_) => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    pay(driver)?;
    settle(driver)
}

#[rstest]
#[case::from_the_battlefield(2, 0, 2)]
#[case::from_the_graveyard(0, 2, 2)]
#[case::from_both(1, 1, 2)]
#[case::one_of_two(1, 1, 1)]
fn craft_exiles_the_materials_and_returns_the_card_transformed(
    #[case] bears: usize,
    #[case] graveyard: usize,
    #[case] x: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(craft_table(seats, bears, graveyard))?;
    craft(&mut driver, x)?;
    assert!(on_battlefield(driver.state(), "Relic Engine").is_none());
    let beast = on_battlefield(driver.state(), "Relic Beast").expect("the back face returned");
    assert_eq!(driver.state().object(beast).unwrap().face(), Some(1));
    let exiled = driver.state().zone(Zone::Exile).len();
    assert_eq!(
        exiled, x as usize,
        "the materials are exiled, the card returned"
    );
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(0))).len()
            + driver
                .state()
                .zone(Zone::Battlefield)
                .iter()
                .filter(|id| driver
                    .state()
                    .object(**id)
                    .is_some_and(|o| o.card().name == "Grizzly Bears"))
                .count(),
        bears + graveyard - x as usize
    );
    Ok(())
}

#[rstest]
fn craft_is_not_offered_without_a_material_or_at_instant_speed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(craft_table(seats, 0, 0))?;
    assert_eq!(
        driver.offered_abilities("Relic Engine")?,
        Vec::<usize>::new()
    );
    Ok(())
}
