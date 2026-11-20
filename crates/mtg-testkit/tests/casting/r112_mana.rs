//! Goal: the r112-01 mana pieces at 2 to 4 seats: blue mana pays any colored symbol of the activation costs of
//! Quicksilver Elemental's abilities, gained ones included, and of no other permanent's abilities (CR 609.4b); mana that
//! can't pay generic costs (Jegantha, CR 106.6) pays only colored symbols of its types, from a source or floating.
//! Security Rhox is cast for {R}{G} with mana from Treasures only, and for its mana cost with any mana. North Star lets
//! any mana pay the next spell's mana cost once. Jeweled Amulet notes the type of the mana its cost spent and adds one
//! mana of that type later.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_core::mana::ManaSymbol;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature_costing, forest, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers the pending payment automatically.
fn pay_auto(driver: &mut Driver) -> Result<Vec<mtg_engine::Event>, ScriptError> {
    use mtg_engine::decision::{Answer, PayChoice};
    driver.answer(Answer::PayMana(PayChoice::Auto))
}

/// Every seat passes once: the top of the stack resolves.
fn resolve(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 controls Quicksilver Elemental, a Chartooth Cougar ("{R}: +1/+0") and `islands` Islands.
fn quicksilver(seats: usize, islands: usize) -> Result<Driver, ScriptError> {
    let mut own = vec![
        ready(card("Quicksilver Elemental").unwrap()),
        ready(card("Chartooth Cougar").unwrap()),
    ];
    own.extend((0..islands).map(|_| ready(island())));
    let mut scenario = Scenario::new(seats).battlefield(0, own);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    Driver::from_scenario(scenario)
}

#[rstest]
fn r112_blue_pays_the_red_cost_of_a_gained_ability(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = quicksilver(seats, 2)?;
    // The Cougar's own {R} ability is not paid by an Island.
    assert!(driver.offered_abilities("Chartooth Cougar")?.is_empty());
    driver.begin_activation("Quicksilver Elemental")?;
    let cougar = driver.target_named("Chartooth Cougar")?;
    driver.choose_targets(&[cougar])?;
    pay_auto(&mut driver)?;
    resolve(&mut driver, seats)?;
    assert!(driver.state().zone(Zone::Stack).is_empty());
    // Its own {U} ability and the gained {R} one, which the last Island pays.
    let elemental = driver.id_of("Quicksilver Elemental")?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority after the resolution");
    };
    let own: Vec<_> = options
        .abilities
        .iter()
        .filter(|ability| ability.object == elemental)
        .copied()
        .collect();
    assert_eq!(own.len(), 2);
    driver.answer(Answer::Priority(PriorityAction::Activate(own[1])))?;
    pay_auto(&mut driver)?;
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
    // With no Island left nothing pays either ability again.
    assert!(
        driver
            .offered_abilities("Quicksilver Elemental")?
            .is_empty()
    );
    Ok(())
}

const W: ManaSymbol = ManaSymbol::Colored(Color::White);
const U: ManaSymbol = ManaSymbol::Colored(Color::Blue);
const B: ManaSymbol = ManaSymbol::Colored(Color::Black);
const R: ManaSymbol = ManaSymbol::Colored(Color::Red);
const G: ManaSymbol = ManaSymbol::Colored(Color::Green);

/// Seat 0 controls Jegantha and `forests` Forests and holds a 2/2 costing `symbols`.
fn jegantha(seats: usize, forests: usize, symbols: &[ManaSymbol]) -> Result<Driver, ScriptError> {
    let mut own = vec![ready(card("Jegantha, the Wellspring").unwrap())];
    own.extend((0..forests).map(|_| ready(forest())));
    let mut scenario = Scenario::new(seats)
        .hand(0, vec![creature_costing("Paid One", symbols, 2, 2)])
        .battlefield(0, own);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    Driver::from_scenario(scenario)
}

#[rstest]
#[case::all_five(0, &[W, U, B, R, G], true)]
#[case::two_colored(0, &[R, G], true)]
#[case::generic_alone(0, &[ManaSymbol::Generic(2)], false)]
#[case::generic_beside(0, &[ManaSymbol::Generic(1), R, G], false)]
#[case::generic_by_a_forest(1, &[ManaSymbol::Generic(1), R, G], true)]
#[case::hybrid(0, &[ManaSymbol::Hybrid(Color::Red, Color::White), ManaSymbol::MonoHybrid(Color::Blue)], true)]
#[case::two_red(0, &[R, R], false)]
#[case::two_red_with_a_forest(1, &[R, R], false)]
#[case::green_twice_with_a_forest(1, &[G, G, ManaSymbol::Generic(1)], false)]
#[case::green_twice_with_two_forests(2, &[G, G, ManaSymbol::Generic(1)], true)]
fn r112_jegantha_mana_pays_no_generic_cost(
    #[values(2, 3, 4)] seats: usize,
    #[case] forests: usize,
    #[case] symbols: &[ManaSymbol],
    #[case] paid: bool,
) -> Result<(), ScriptError> {
    let mut driver = jegantha(seats, forests, symbols)?;
    let cast = driver.cast("Paid One").is_ok();
    assert_eq!(
        cast && driver.state().zone(Zone::Stack).len() == 1,
        paid,
        "{symbols:?}"
    );
    Ok(())
}

#[rstest]
fn r112_floating_jegantha_mana_pays_colored_symbols_only(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = jegantha(seats, 1, &[ManaSymbol::Generic(1), R, G])?;
    driver.tap_for_mana("Jegantha, the Wellspring")?;
    // The floating mana pays {R}{G} and the Forest the {1}; the three other mana stay in the pool.
    driver.cast("Paid One")?;
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
    let pool = driver
        .state()
        .player(mtg_core::ids::PlayerId(0))
        .unwrap()
        .mana();
    assert_eq!(pool.restricted().len(), 3);
    Ok(())
}

/// An artifact Treasure: "{T}, Sacrifice this artifact: Add one mana of any color."
fn treasure() -> std::sync::Arc<mtg_cards::CardDefinition> {
    std::sync::Arc::new(
        mtg_cards::CardDefinition::new("Treasure")
            .with_type(mtg_core::card::CardType::Artifact)
            .with_subtype("Treasure")
            .with_rules(
                serde_json::from_str(
                    r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

#[rstest]
#[case::two_treasures(0, 2, true)]
#[case::lands_pay_no_rather_cost(2, 0, false)]
#[case::one_treasure_and_a_land(1, 1, false)]
fn r112_security_rhox_is_cast_this_way_with_treasure_mana_only(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: usize,
    #[case] treasures: usize,
    #[case] cast: bool,
) -> Result<(), ScriptError> {
    let mut own: Vec<_> = (0..lands)
        .map(|at| ready(if at % 2 == 0 { mountain() } else { forest() }))
        .collect();
    own.extend((0..treasures).map(|_| ready(treasure())));
    let mut scenario = Scenario::new(seats)
        .hand(0, vec![card("Security Rhox").unwrap()])
        .battlefield(0, own);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let offered = driver.begin_cast("Security Rhox").is_ok();
    assert_eq!(offered, cast);
    if cast {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
            crate::common::casting::choose_way(&mut driver, "Cast for {R}{G}")?;
        }
        pay_auto(&mut driver)?;
        assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
    }
    Ok(())
}

#[rstest]
fn r112_security_rhox_mana_cost_takes_any_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let own = vec![
        ready(mountain()),
        ready(forest()),
        ready(forest()),
        ready(forest()),
    ];
    let mut scenario = Scenario::new(seats)
        .hand(0, vec![card("Security Rhox").unwrap()])
        .battlefield(0, own);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Security Rhox")?;
    // Only the mana cost is payable, so no way is asked.
    pay_auto(&mut driver)?;
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
    Ok(())
}

#[rstest]
fn r112_north_star_lets_any_mana_pay_the_next_mana_cost(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut own = vec![ready(card("North Star").unwrap())];
    own.extend((0..10).map(|_| ready(forest())));
    let mut scenario = Scenario::new(seats)
        .hand(
            0,
            vec![
                creature_costing("Blue One", &[ManaSymbol::Generic(1), U, U], 2, 2),
                creature_costing("Blue Two", &[U], 1, 1),
            ],
        )
        .battlefield(0, own);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(driver.begin_cast("Blue One").is_err());
    driver.begin_activation("North Star")?;
    pay_auto(&mut driver)?;
    resolve(&mut driver, seats)?;
    assert!(driver.state().zone(Zone::Stack).is_empty());
    driver.cast("Blue One")?;
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
    resolve(&mut driver, seats)?;
    // The cast ended the rule: Forests pay no {U} again.
    assert!(driver.begin_cast("Blue Two").is_err());
    Ok(())
}

/// How much floating mana of `kind` seat 0 has.
fn floating(driver: &Driver, kind: mtg_core::mana::ManaType) -> u32 {
    driver
        .state()
        .player(mtg_core::ids::PlayerId(0))
        .unwrap()
        .mana()
        .get(kind)
}

#[rstest]
#[case::blue(island, mtg_core::mana::ManaType::Blue)]
#[case::red(mountain, mtg_core::mana::ManaType::Red)]
fn r112_jeweled_amulet_adds_the_type_its_cost_spent(
    #[values(2, 3, 4)] seats: usize,
    #[case] land: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
    #[case] kind: mtg_core::mana::ManaType,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        vec![ready(card("Jeweled Amulet").unwrap()), ready(land())],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Jeweled Amulet", 0)?;
    pay_auto(&mut driver)?;
    resolve(&mut driver, seats)?;
    assert!(driver.state().zone(Zone::Stack).is_empty());
    // With a charge counter on it the first ability is not offered; the tapped Amulet offers neither.
    assert!(driver.offered_abilities("Jeweled Amulet")?.is_empty());
    // Seat 0's next turn: the Amulet untapped, its note kept.
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, mtg_engine::turn::Step::Main1)?;
    assert_eq!(driver.offered_abilities("Jeweled Amulet")?, vec![1]);
    driver.begin_ability("Jeweled Amulet", 1)?;
    assert!(driver.state().zone(Zone::Stack).is_empty());
    assert_eq!(floating(&driver, kind), 1);
    Ok(())
}

/// Seat 0 casts Emblazoned Golem kicked for `x` with `lands`; the counters it entered with, or `None` when the payment
/// did not go through.
fn r112_golem(
    seats: usize,
    lands: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
    x: u32,
) -> Result<Option<u32>, ScriptError> {
    use mtg_engine::choose::{CostsChoice, NumberChoice};
    use mtg_engine::decision::{Answer, PayChoice};
    let mut scenario = Scenario::new(seats)
        .hand(0, vec![card("Emblazoned Golem").unwrap()])
        .battlefield(0, lands.into_iter().map(ready).collect::<Vec<_>>());
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Emblazoned Golem")?;
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(vec![0])))?;
    if driver
        .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
        .is_err()
    {
        return Ok(None);
    }
    if driver.answer(Answer::PayMana(PayChoice::Auto)).is_err()
        || driver.state().zone(Zone::Stack).is_empty()
    {
        return Ok(None);
    }
    resolve(&mut driver, seats)?;
    let golem = driver.id_of("Emblazoned Golem")?;
    Ok(Some(
        driver
            .state()
            .object(golem)
            .and_then(mtg_engine::object::GameObject::permanent)
            .unwrap()
            .counters()
            .map(|(_, count)| count)
            .sum(),
    ))
}

#[rstest]
#[case::two_colors(vec![island(), mountain(), forest(), forest()], 2, Some(2))]
#[case::one_color_twice(vec![forest(), forest(), forest(), forest()], 2, None)]
#[case::three_colors(vec![island(), mountain(), forest(), mtg_testkit::cards::plains(), mtg_testkit::cards::wastes()], 3, Some(3))]
#[case::colorless_for_x(vec![mtg_testkit::cards::wastes(), forest(), forest(), forest()], 2, None)]
fn r112_emblazoned_golem_pays_x_one_mana_of_each_color(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
    #[case] x: u32,
    #[case] counters: Option<u32>,
) -> Result<(), ScriptError> {
    assert_eq!(r112_golem(seats, lands, x)?, counters);
    Ok(())
}

/// A creature with Brigid, Doun's Mind's "{T}: Add X {G} or X {W}, where X is the number of other creatures you control."
fn r112_brigid() -> std::sync::Arc<mtg_cards::CardDefinition> {
    std::sync::Arc::new(
        mtg_cards::CardDefinition::new("Mind Dork")
            .with_type(mtg_core::card::CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(
                serde_json::from_str(
                    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"R112OneOfCount":{"among":["Green","White"],"count":{"Count":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]}}}}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

#[rstest]
#[case::two_others_white(2, 1, mtg_core::mana::ManaType::White)]
#[case::three_others_green(3, 0, mtg_core::mana::ManaType::Green)]
fn r112_x_mana_of_one_chosen_type(
    #[values(2, 3, 4)] seats: usize,
    #[case] others: usize,
    #[case] pick: usize,
    #[case] kind: mtg_core::mana::ManaType,
) -> Result<(), ScriptError> {
    use mtg_engine::choose::OptionChoice;
    use mtg_engine::decision::Answer;
    let mut own = vec![ready(r112_brigid())];
    own.extend((0..others).map(|_| ready(mtg_testkit::cards::bear())));
    let mut scenario = Scenario::new(seats).battlefield(0, own);
    // Another player's creature does not count.
    scenario = scenario.battlefield(1, vec![ready(mtg_testkit::cards::bear())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Mind Dork")?;
    driver.answer(Answer::ChooseOption(OptionChoice(pick)))?;
    assert_eq!(floating(&driver, kind), u32::try_from(others).unwrap());
    Ok(())
}

/// Seat 0 controls Night Soil and a Forest; seat `seat` of each pair holds a creature card named after it in their
/// graveyard.
fn r112_night_soil(seats: usize, graveyards: &[(usize, &str)]) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(card("Night Soil").unwrap()), ready(forest())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    for seat in 0..seats {
        let cards: Vec<_> = graveyards
            .iter()
            .filter(|(owner, _)| *owner == seat)
            .map(|(_, name)| mtg_testkit::cards::creature(name, 1, &[], 1, 1))
            .collect();
        scenario = scenario.graveyard(seat, cards);
    }
    Driver::from_scenario(scenario)
}

/// Answers the pick of the cost with the cards named.
fn r112_pick(driver: &mut Driver, names: &[&str]) -> Result<Vec<mtg_engine::Event>, ScriptError> {
    use mtg_engine::choose::CardsChoice;
    use mtg_engine::decision::Answer;
    let ids: Vec<_> = names
        .iter()
        .filter_map(|name| {
            driver
                .state()
                .objects()
                .find(|object| object.card().name == *name)
                .map(mtg_engine::object::GameObject::id)
        })
        .collect();
    driver.answer(Answer::ChooseCards(CardsChoice(ids)))
}

#[rstest]
#[case::two_of_one_owner(&[(0, "Mine A"), (0, "Mine B"), (1, "Theirs A")], true)]
#[case::two_of_an_opponent(&[(0, "Mine A"), (1, "Theirs A"), (1, "Theirs B")], true)]
#[case::one_each(&[(0, "Mine A"), (1, "Theirs A")], false)]
fn r112_night_soil_is_offered_with_two_in_one_graveyard(
    #[values(2, 3, 4)] seats: usize,
    #[case] graveyards: &[(usize, &str)],
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = r112_night_soil(seats, graveyards)?;
    assert_eq!(!driver.offered_abilities("Night Soil")?.is_empty(), offered);
    Ok(())
}

#[rstest]
fn r112_night_soil_refuses_cards_of_two_graveyards(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r112_night_soil(seats, &[(0, "Mine A"), (0, "Mine B"), (1, "Theirs A")])?;
    driver.begin_activation("Night Soil")?;
    assert!(r112_pick(&mut driver, &["Mine A", "Theirs A"]).is_err());
    r112_pick(&mut driver, &["Mine A", "Mine B"])?;
    pay_auto(&mut driver)?;
    resolve(&mut driver, seats)?;
    let exiled = crate::common::names(driver.state(), Zone::Exile);
    assert_eq!(exiled.len(), 2);
    assert!(
        crate::common::names(driver.state(), Zone::Battlefield).contains(&"Saproling".to_owned())
    );
    Ok(())
}

#[rstest]
#[case::red(mountain, &["Red Bear", "Red Ogre"])]
#[case::green(forest, &["Green Bear", "Green Ogre"])]
#[case::colorless(mtg_testkit::cards::wastes, &[])]
fn r112_protective_sphere_offers_sources_of_the_spent_colors(
    #[values(2, 3, 4)] seats: usize,
    #[case] land: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
    #[case] offered: &[&str],
) -> Result<(), ScriptError> {
    use mtg_engine::decision::DecisionKind;
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            vec![ready(card("Protective Sphere").unwrap()), ready(land())],
        )
        .battlefield(
            1,
            vec![
                ready(mtg_testkit::cards::creature(
                    "Red Bear",
                    1,
                    &[Color::Red],
                    2,
                    2,
                )),
                ready(mtg_testkit::cards::creature(
                    "Green Bear",
                    1,
                    &[Color::Green],
                    2,
                    2,
                )),
                ready(mtg_testkit::cards::creature(
                    "Red Ogre",
                    1,
                    &[Color::Red],
                    3,
                    3,
                )),
                ready(mtg_testkit::cards::creature(
                    "Green Ogre",
                    1,
                    &[Color::Green],
                    3,
                    3,
                )),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Protective Sphere")?;
    pay_auto(&mut driver)?;
    resolve(&mut driver, seats)?;
    let found: Vec<String> = match &driver.decision()?.kind {
        DecisionKind::ChooseCards(options) => options
            .from
            .iter()
            .map(|id| crate::common::name_of(driver.state(), *id))
            .collect(),
        _ => Vec::new(),
    };
    assert_eq!(found, offered);
    Ok(())
}
