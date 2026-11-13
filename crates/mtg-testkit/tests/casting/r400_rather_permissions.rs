//! Goal: permissions that put another cost in place of the mana cost of other spells play as printed at tables of two,
//! three and four seats: under Kentaro, the Smiling Cat a Samurai spell is cast for generic mana equal to its mana value
//! (never another creature, never for less), and under Demon of Fate's Design one enchantment spell each of its
//! controller's turns is cast for life equal to its mana value (a second one that turn is not, nor a creature); the log
//! replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::pay;
use crate::common::pid;

/// A 2/2 creature card costing {2}{W}{W} of the creature type `subtype`.
fn r400_creature(subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(format!("Plain {subtype}"))
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_mana_cost(ManaCost::parse("{2}{W}{W}").unwrap())
            .with_power_toughness(2, 2),
    )
}

/// An enchantment card named `name` costing {3}{U}.
fn r400_enchantment(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_mana_cost(ManaCost::parse("{3}{U}").unwrap()),
    )
}

fn r400_offered(driver: &Driver, name: &str) -> Result<bool, ScriptError> {
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

/// Casts `name` for the way that is not its printed mana cost `printed`, then lets it resolve.
fn r400_cast_other_way(driver: &mut Driver, name: &str, printed: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    for _ in 0..20 {
        let resolved = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == name);
        if resolved {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| {
                        matches!(option, ChoiceOption::Label(label) if label != &format!("Cast for {printed}"))
                    })
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::PayMana(_) => {
                pay(driver)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck(format!("{name} did not resolve")))
}

fn r400_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0 holds Kentaro and `mountains` Mountains, and a {2}{W}{W} creature of `subtype` in hand.
#[rstest]
#[case::a_samurai_for_four("Samurai", 4, true)]
#[case::a_samurai_for_three("Samurai", 3, false)]
#[case::a_soldier("Soldier", 4, false)]
fn r400_kentaro_casts_samurai_for_their_mana_value(
    #[values(2, 3, 4)] seats: usize,
    #[case] subtype: &str,
    #[case] mountains: usize,
    #[case] open: bool,
) -> Result<(), ScriptError> {
    let mut lands = vec![ready(card("Kentaro, the Smiling Cat").unwrap())];
    lands.extend((0..mountains).map(|_| ready(mountain())));
    let name = format!("Plain {subtype}");
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, lands)
        .hand(0, [r400_creature(subtype)]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(r400_offered(&driver, &name)?, open);
    if open {
        r400_cast_other_way(&mut driver, &name, "{2}{W}{W}")?;
        let untapped = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| {
                let object = driver.state().object(**id).unwrap();
                object.card().name == "Mountain" && !object.permanent().unwrap().tapped()
            })
            .count();
        assert_eq!(untapped, 0, "four generic mana paid");
        r400_replays(&driver);
    }
    Ok(())
}

/// Seat 0 holds Demon of Fate's Design and two {3}{U} enchantments and a creature card in hand, with no lands: on its
/// own turn the first enchantment is cast for 4 life, then neither the second nor the creature is castable.
#[rstest]
fn r400_demon_casts_one_enchantment_a_turn_for_life(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(card("Demon of Fate's Design").unwrap())])
        .hand(
            0,
            [
                r400_enchantment("First Omen"),
                r400_enchantment("Second Omen"),
                r400_creature("Samurai"),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(r400_offered(&driver, "First Omen")?);
    assert!(
        !r400_offered(&driver, "Plain Samurai")?,
        "a creature spell is no enchantment"
    );
    r400_cast_other_way(&mut driver, "First Omen", "{3}{U}")?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 16);
    for _ in 0..5 {
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
        {
            break;
        }
        driver.answer_passively()?;
    }
    assert!(!r400_offered(&driver, "Second Omen")?, "once each turn");
    r400_replays(&driver);
    Ok(())
}
