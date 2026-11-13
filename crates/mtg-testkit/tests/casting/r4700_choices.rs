//! Goal: permanents that make several choices as they enter play their Oracle text at tables of two to four seats: Seal
//! of the Guildpact asks two different colors and takes {1} from each spell its controller casts for each of them the
//! spell is (an opponent's spell, a spell of neither and the colored part aside), Tablet of the Guilds gains its
//! controller 1 life for each chosen color of a spell they cast and does not trigger for a spell of neither, and Volrath's
//! Laboratory and Riptide Replicator ask a color then a creature type and make a creature token of that color and type,
//! 2/2 or as large as the charge counters; the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{ChoiceOption, NumberChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_engine::zones::Zone;
use mtg_engine::{characteristics, replay};
use mtg_testkit::cards::{cost, forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::casting::lands;

/// An artifact with this mana cost and no ability: its colors are those of the cost.
fn r4700_artifact(name: &str, generic: u32, colors: &[Color]) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(generic, colors)),
    )
}

/// A table where seat 0 holds `hand` and every seat has six lands of each basic type, in the first main phase of
/// `active`.
fn r4700_table(seats: usize, active: usize, hand: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut scenario = Scenario::new(seats).hand(0, hand);
    for seat in 0..seats {
        let mut board = lands(6, plains);
        board.extend(lands(6, island));
        board.extend(lands(6, swamp));
        board.extend(lands(6, mountain));
        board.extend(lands(6, forest));
        scenario = scenario.battlefield(seat, board);
    }
    scenario.at(1, active, mtg_engine::turn::Step::Main1)
}

/// The label of a choice: a color by its name, a creature type by itself.
fn r4700_label(option: &ChoiceOption) -> Option<String> {
    match option {
        ChoiceOption::Color(color) => Some(format!("{color:?}")),
        ChoiceOption::Label(label) => Some(label.clone()),
        _ => None,
    }
}

/// Passes until the stack is empty, answering each choice of an option with the next of `picks` and counting them.
fn r4700_resolve(driver: &mut Driver, picks: &[&str]) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                let want = picks[asked];
                let at = options
                    .options
                    .iter()
                    .position(|option| r4700_label(option).as_deref() == Some(want))
                    .unwrap_or_else(|| panic!("{want} is offered"));
                asked += 1;
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::Priority(_) if driver.state().zone(Zone::Stack).is_empty() => {
                return Ok(asked);
            }
            _ => {
                driver.pass()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack does not empty".to_owned()))
}

/// What the permanent named remembers it chose.
fn r4700_chosen(driver: &Driver, name: &str) -> Option<Chosen> {
    let id = driver.id_of(name).ok()?;
    driver.state().object(id)?.permanent()?.chosen().cloned()
}

#[rstest]
#[case::two_colors_it_is(2, 0, (3, &[Color::Blue, Color::Red][..]), "{1}{U}{R}")]
#[case::one_color_it_is(3, 0, (3, &[Color::Blue][..]), "{2}{U}")]
#[case::neither_color(4, 0, (3, &[Color::Green][..]), "{3}{G}")]
#[case::colored_part_stays(2, 0, (1, &[Color::Blue, Color::Red][..]), "{U}{R}")]
#[case::an_opponents_spell(3, 1, (3, &[Color::Blue, Color::Red][..]), "{3}{U}{R}")]
fn r4700_seal_takes_one_for_each_chosen_color_of_its_controllers_spells(
    #[case] seats: usize,
    #[case] caster: usize,
    #[case] spell: (u32, &[Color]),
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let seal = card("Seal of the Guildpact").unwrap();
    let scenario = || {
        let trinket = r4700_artifact("Gold Trinket", spell.0, spell.1);
        if caster == 0 {
            r4700_table(seats, 0, vec![seal.clone(), trinket])
        } else {
            r4700_table(seats, 0, vec![seal.clone()]).hand(caster, vec![trinket])
        }
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Seal of the Guildpact")?;
    assert_eq!(r4700_resolve(&mut driver, &["Blue", "Red"])?, 2);
    assert_eq!(
        r4700_chosen(&driver, "Seal of the Guildpact"),
        Some(Chosen::Colors(vec![Color::Blue, Color::Red]))
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    if caster != 0 {
        // The next turn is the next seat's.
        driver.idle_until(2, mtg_engine::turn::Step::Main1)?;
        assert_eq!(driver.state().turn().active(), crate::common::pid(caster));
    }
    driver.begin_cast("Gold Trinket")?;
    assert_eq!(
        driver.pay_options()?.cost.to_string(),
        ManaCost::parse(expected).unwrap().to_string()
    );
    Ok(())
}

#[test]
fn r4700_seal_offers_a_second_color_other_than_the_first() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r4700_table(
        2,
        0,
        vec![card("Seal of the Guildpact").unwrap()],
    ))?;
    driver.cast("Seal of the Guildpact")?;
    for _ in 0..20 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
            break;
        }
        driver.pass()?;
    }
    let first = driver.decision()?.clone();
    let DecisionKind::ChooseOption(options) = &first.kind else {
        panic!("a color is asked");
    };
    assert_eq!(options.options.len(), 5);
    let green = options
        .options
        .iter()
        .position(|option| *option == ChoiceOption::Color(Color::Green))
        .unwrap();
    driver.answer(Answer::ChooseOption(OptionChoice(green)))?;
    let DecisionKind::ChooseOption(second) = &driver.decision()?.kind else {
        panic!("a second color is asked");
    };
    assert_eq!(second.options.len(), 4);
    assert!(!second.options.contains(&ChoiceOption::Color(Color::Green)));
    Ok(())
}

#[rstest]
#[case::both_colors(2, (2, &[Color::White, Color::Black][..]), 2)]
#[case::one_color(3, (2, &[Color::White][..]), 1)]
#[case::neither(4, (2, &[Color::Green][..]), 0)]
#[case::colorless(2, (2, &[][..]), 0)]
fn r4700_tablet_gains_one_life_for_each_chosen_color_of_the_spell(
    #[case] seats: usize,
    #[case] spell: (u32, &[Color]),
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let scenario = || {
        r4700_table(
            seats,
            0,
            vec![
                card("Tablet of the Guilds").unwrap(),
                r4700_artifact("Guild Trinket", spell.0, spell.1),
            ],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Tablet of the Guilds")?;
    r4700_resolve(&mut driver, &["White", "Black"])?;
    let before = driver.state().player(crate::common::pid(0)).unwrap().life();
    driver.cast("Guild Trinket")?;
    r4700_resolve(&mut driver, &[])?;
    let after = driver.state().player(crate::common::pid(0)).unwrap().life();
    assert_eq!(after - before, gained);
    let triggered = driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, mtg_engine::Event::AbilityTriggered { .. }));
    assert_eq!(
        triggered,
        gained > 0,
        "the trigger checks its condition as the spell is cast"
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::laboratory_two_seats(2, "Volrath's Laboratory", None, "Green", "Elf", 2)]
#[case::laboratory_four_seats(4, "Volrath's Laboratory", None, "Black", "Zombie", 2)]
#[case::replicator_three(3, "Riptide Replicator", Some(3), "Blue", "Merfolk", 3)]
#[case::replicator_one(2, "Riptide Replicator", Some(1), "Red", "Goblin", 1)]
fn r4700_tokens_of_the_chosen_color_and_type(
    #[case] seats: usize,
    #[case] name: &str,
    #[case] x: Option<u32>,
    #[case] color: &str,
    #[case] kind: &str,
    #[case] size: i32,
) -> Result<(), ScriptError> {
    let scenario = || r4700_table(seats, 0, vec![card(name).unwrap()]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast(name)?;
    if let Some(x) = x {
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    }
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    assert_eq!(r4700_resolve(&mut driver, &[color, kind])?, 2);
    let chosen = r4700_chosen(&driver, name);
    let Some(Chosen::ColorAndType(held, subtype)) = chosen else {
        panic!("a color and a type are remembered, got {chosen:?}");
    };
    assert_eq!(format!("{held:?}"), color);
    assert_eq!(subtype, Subtype(kind.to_owned()));
    let before: Vec<_> = driver.state().zone(Zone::Battlefield).to_vec();
    driver.begin_activation(name)?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    r4700_resolve(&mut driver, &[])?;
    let made: Vec<_> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| !before.contains(id))
        .collect();
    assert_eq!(made.len(), 1, "one token");
    let chars = characteristics(driver.state(), made[0]).unwrap();
    assert!(chars.types.contains(CardType::Creature));
    assert_eq!(
        chars
            .colors
            .iter()
            .map(|c| format!("{c:?}"))
            .collect::<Vec<_>>(),
        [color]
    );
    let printed = driver
        .state()
        .object(made[0])
        .unwrap()
        .card()
        .subtypes
        .clone();
    assert_eq!(printed, vec![Subtype(kind.to_owned())]);
    assert!(chars.has_subtype(&printed, &Subtype(kind.to_owned())));
    assert_eq!((chars.power, chars.toughness), (Some(size), Some(size)));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
