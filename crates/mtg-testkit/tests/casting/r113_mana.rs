//! Goal: the r113-01 mana pieces at 2 to 4 seats: Vazi, Keen Negotiator's trigger on an opponent's spell or activated
//! ability, mana abilities included (CR 605.3b), happens only when mana from a Treasure paid for it (CR 601.2h, 602.2b):
//! a spell, an ability on the stack and a mana ability that costs mana, each paid by a Treasure or by an Island. Boreal
//! Outrider counts only snow mana of the spell's own colors (CR 106.3). Soul Burn's X takes black or red mana only, and
//! its life gain is the black mana spent on X, capped by the damage, the player's life before it or the toughness.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// An artifact Treasure: "{T}, Sacrifice this artifact: Add one mana of any color."
fn treasure() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Treasure")
            .with_type(CardType::Artifact)
            .with_subtype("Treasure")
            .with_rules(rules(
                r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
            )),
    )
}

/// An artifact with "{1}: Draw a card." (an ability on the stack).
fn thinker() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Thinker")
            .with_type(CardType::Artifact)
            .with_rules(rules(
                r#"{"Activated":{"cost":[{"Mana":"{1}"}],"effects":[{"Draw":{}}]}}"#,
            )),
    )
}

/// An artifact with "{1}, {T}: Add {U}{U}." (a mana ability that costs mana).
fn prism() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Prism")
            .with_type(CardType::Artifact)
            .with_rules(rules(
                r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Blue","Blue"]}}}]}}"#,
            )),
    )
}

/// A 2/2 creature for {1}{U}.
fn drake() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Drake")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[Color::Blue]))
            .with_power_toughness(2, 2),
    )
}

/// Seat 0 controls Vazi; seat 1, the active player, holds a Drake and controls a Treasure, two Islands, a Thinker and a
/// Prism.
fn vazi(seats: usize) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(0, [ready(card("Vazi, Keen Negotiator").unwrap())])
        .hand(1, vec![drake()])
        .battlefield(
            1,
            [
                ready(treasure()),
                ready(island()),
                ready(island()),
                ready(thinker()),
                ready(prism()),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    Driver::from_scenario(scenario)
}

/// Seat 0's hand size.
fn vazi_hand(driver: &Driver) -> usize {
    driver
        .state()
        .zone(mtg_engine::zones::Zone::Hand(PlayerId(0)))
        .len()
}

/// Answers every decision passively until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    panic!("the stack does not settle");
}

#[rstest]
#[case::spell_by_a_treasure("spell", "Treasure", 1)]
#[case::spell_by_an_island("spell", "Island", 0)]
#[case::ability_by_a_treasure("ability", "Treasure", 1)]
#[case::ability_by_an_island("ability", "Island", 0)]
#[case::mana_ability_by_a_treasure("mana", "Treasure", 1)]
#[case::mana_ability_by_an_island("mana", "Island", 0)]
fn r113_vazi_triggers_on_treasure_mana_of_a_spell_or_any_activation(
    #[values(2, 3, 4)] seats: usize,
    #[case] what: &str,
    #[case] paying: &str,
    #[case] draws: usize,
) -> Result<(), ScriptError> {
    let mut driver = vazi(seats)?;
    let before = vazi_hand(&driver);
    match what {
        "spell" => {
            driver.cast_tapping("Drake", &[paying, "Island"])?;
        }
        "ability" => {
            driver.begin_activation("Thinker")?;
            driver.pay_tapping(&[paying], 0)?;
        }
        _ => {
            driver.tap_for_mana_of(paying, ManaType::Blue)?;
            driver.tap_for_mana("Prism")?;
            let pool = driver.state().player(PlayerId(1)).unwrap().mana().total();
            assert_eq!(pool, 2, "the Prism spent the floating mana and added two");
        }
    }
    settle(&mut driver)?;
    assert_eq!(vazi_hand(&driver) - before, draws);
    let counters: u32 = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter_map(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&CounterKind::PlusOnePlusOne))
        .sum();
    assert_eq!(counters as usize, draws);
    Ok(())
}

/// A basic land of `color` named `name`, snow when `snow` is set.
fn named_land(name: &str, color: Color, snow: bool) -> Arc<CardDefinition> {
    let mut land = CardDefinition::basic_land(Some(color));
    name.clone_into(&mut land.name);
    if snow {
        land = land.with_supertype(mtg_core::card::Supertype::Snow);
    }
    Arc::new(land)
}

/// A 2/2 creature for {1}{G}.
fn bear() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Bear")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[Color::Green]))
            .with_power_toughness(2, 2),
    )
}

#[rstest]
#[case::snow_green(&["Snowy Forest", "Forest"], 1)]
#[case::snow_blue_on_generic(&["Forest", "Snowy Island"], 0)]
#[case::no_snow(&["Forest", "Island"], 0)]
fn r113_boreal_outrider_reads_snow_mana_of_the_spells_colors(
    #[values(2, 3, 4)] seats: usize,
    #[case] paying: &[&str],
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, vec![bear()])
            .battlefield(
                0,
                [
                    ready(card("Boreal Outrider").unwrap()),
                    ready(named_land("Snowy Forest", Color::Green, true)),
                    ready(named_land("Snowy Island", Color::Blue, true)),
                    ready(named_land("Forest", Color::Green, false)),
                    ready(named_land("Island", Color::Blue, false)),
                ],
            ),
    )?;
    driver.cast_tapping("Bear", paying)?;
    settle(&mut driver)?;
    let bear = driver.id_of("Bear")?;
    let on = driver
        .state()
        .object(bear)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    assert_eq!(on, counters);
    Ok(())
}

/// Seat 0 holds Soul Burn and controls `lands`; seat 1 has 3 life and controls a 2/2 Drake.
fn soul_burn(seats: usize, lands: &[(&str, Color)]) -> Result<Driver, ScriptError> {
    let own: Vec<_> = lands
        .iter()
        .map(|(name, color)| ready(named_land(name, *color, false)))
        .collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![card("Soul Burn").unwrap()])
        .battlefield(0, own)
        .battlefield(1, [ready(drake())])
        .life(1, 3);
    Driver::from_scenario(scenario)
}

fn life_of(driver: &Driver, seat: u64) -> i32 {
    driver.state().player(PlayerId(seat)).unwrap().life()
}

/// Starts Soul Burn with X announced and its target chosen.
fn begin_soul_burn(driver: &mut Driver, x: u32, at: &str) -> Result<(), ScriptError> {
    driver.begin_cast("Soul Burn")?;
    driver.answer(mtg_engine::decision::Answer::ChooseNumber(
        mtg_engine::choose::NumberChoice::Number(x),
    ))?;
    let target = if at == "player" {
        driver.target_player(1)
    } else {
        driver.target_named("Drake")?
    };
    driver.choose_targets(&[target])?;
    Ok(())
}

const LANDS: [(&str, Color); 9] = [
    ("Swamp A", Color::Black),
    ("Swamp B", Color::Black),
    ("Swamp C", Color::Black),
    ("Swamp D", Color::Black),
    ("Swamp E", Color::Black),
    ("Mountain A", Color::Red),
    ("Mountain B", Color::Red),
    ("Mountain C", Color::Red),
    ("Mountain D", Color::Red),
];

#[rstest]
// X = 2 at a player with 3 life, three black mana: two of them on X.
#[case::black_on_x(2, "player", &["Swamp A", "Swamp B", "Swamp C", "Mountain A", "Mountain B"], 2)]
// Only the {B} of the cost is black: none on X.
#[case::red_on_x(2, "player", &["Swamp A", "Mountain A", "Mountain B", "Mountain C", "Mountain D"], 0)]
// One black mana beyond the {B}.
#[case::one_black_on_x(2, "player", &["Swamp A", "Swamp B", "Mountain A", "Mountain B", "Mountain C"], 1)]
// X = 4 at a player with 3 life: no more than the life before the damage.
#[case::capped_by_life(4, "player", &["Swamp A", "Swamp B", "Swamp C", "Swamp D", "Swamp E", "Mountain A", "Mountain B"], 3)]
// X = 3 at a 2/2: no more than its toughness.
#[case::capped_by_toughness(3, "creature", &["Swamp A", "Swamp B", "Swamp C", "Swamp D", "Mountain A", "Mountain B"], 2)]
fn r113_soul_burn_gains_the_black_spent_on_x_capped(
    #[values(2, 3, 4)] seats: usize,
    #[case] x: u32,
    #[case] at: &str,
    #[case] paying: &[&str],
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let mut driver = soul_burn(seats, &LANDS)?;
    let before = life_of(&driver, 0);
    begin_soul_burn(&mut driver, x, at)?;
    driver.pay_tapping(paying, 0)?;
    settle(&mut driver)?;
    assert_eq!(life_of(&driver, 0) - before, gained);
    if at == "player" {
        assert_eq!(life_of(&driver, 1), 3 - i32::try_from(x).unwrap());
    } else {
        assert!(driver.id_of("Drake").is_err(), "the Drake is destroyed");
    }
    Ok(())
}

#[rstest]
fn r113_soul_burn_x_takes_no_other_color(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let lands = [
        ("Swamp A", Color::Black),
        ("Island A", Color::Blue),
        ("Island B", Color::Blue),
        ("Island C", Color::Blue),
    ];
    let mut driver = soul_burn(seats, &lands)?;
    driver.begin_cast("Soul Burn")?;
    // Only X = 0 can be paid, so X is not asked.
    assert!(
        !matches!(
            driver.decision()?.kind,
            mtg_engine::decision::DecisionKind::ChooseNumber(_)
        ),
        "blue mana pays no X"
    );
    Ok(())
}
