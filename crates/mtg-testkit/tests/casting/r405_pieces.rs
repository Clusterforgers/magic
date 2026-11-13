//! Goal: the r40-5 pieces play as printed at tables of two, three and four seats: mana spent only on spells that are one
//! or more colors without {X} (Titans' Nest), on Equipment spells and equip abilities (Freya Crescent), on a disturb cost or
//! an instant or sorcery spell (Unblinking Observer), on monocolored spells of the chosen color with an ability paid only
//! with that color (Throne of Eldraine), counters for non-Human creatures (Animal Attendant); targets judged only as a
//! spell is cast (Enthralling Hold), creatures with a {T} ability (Magewright's Stone), lands that are snow or could
//! produce {C} (Break the Ice); and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, island, mountain, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

const SPELL: &str = "Spell";

/// A spell of these types and subtypes with this cost, `x` of its symbols being {X}.
fn r405_spell(types: &[CardType], subtypes: &[&str], cost: &str, x: u8) -> Arc<CardDefinition> {
    let mut definition = CardDefinition::new(SPELL)
        .with_mana_cost(ManaCost::parse(cost).unwrap())
        .with_x_in_cost(x);
    for card_type in types {
        definition = definition.with_type(*card_type);
    }
    for subtype in subtypes {
        definition = definition.with_subtype(*subtype);
    }
    if types.contains(&CardType::Creature) {
        definition = definition.with_power_toughness(1, 1);
    }
    Arc::new(definition)
}

/// Seat 0's first main phase at a table of `seats`: it holds the spell, has `mine` in play (cards of the corpus by name,
/// or a basic land of the testkit) and a Grizzly Bears card in its graveyard.
fn r405_table(
    seats: usize,
    spell: Arc<CardDefinition>,
    mine: &[&str],
) -> Result<Driver, ScriptError> {
    let permanents: Vec<_> = mine
        .iter()
        .map(|name| match *name {
            "Forest" => ready(forest()),
            "Mountain" => ready(mountain()),
            "Island" => ready(island()),
            other => ready(card(other).unwrap()),
        })
        .collect();
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [spell])
        .battlefield(0, permanents)
        .graveyard(0, [bear()]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| bear()));
    }
    Driver::from_scenario(scenario)
}

fn r405_castable(driver: &Driver) -> Result<bool, ScriptError> {
    match &driver.decision()?.kind {
        DecisionKind::Priority(options) => Ok(!options.castable.is_empty()),
        _ => Err(ScriptError::Expectation("priority was expected".into())),
    }
}

/// A Forest that is snow, named apart from the plain one.
fn r405_snowy_forest() -> Arc<CardDefinition> {
    let mut land = CardDefinition::clone(&forest()).with_supertype(mtg_core::card::Supertype::Snow);
    land.name = "Snowy Forest".to_owned();
    Arc::new(land)
}

/// Answers passively until the stack and the pending triggers are empty.
fn r405_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn r405_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

use CardType::{Artifact, Creature, Instant, Sorcery};

#[rstest]
#[case::colored_without_x(2, &[Creature], "{1}{G}", 0, true)]
#[case::colored_without_x_three_seats(3, &[Creature], "{1}{G}", 0, true)]
#[case::colorless(2, &[Artifact], "{2}", 0, false)]
#[case::colored_with_x(4, &[Creature], "{1}{G}", 1, false)]
#[case::four_seats(4, &[Instant], "{1}{G}", 0, true)]
fn the_nest_pays_for_colored_spells_without_x(
    #[case] seats: usize,
    #[case] types: &[CardType],
    #[case] cost: &str,
    #[case] x: u8,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = r405_table(
        seats,
        r405_spell(types, &[], cost, x),
        &["Titans' Nest", "Forest"],
    )?;
    // The Nest's mana floats first: it exiles the Grizzly Bears card from the graveyard.
    let ability = *driver
        .offered_abilities("Titans' Nest")?
        .first()
        .expect("the Nest's mana ability is offered");
    driver.begin_ability("Titans' Nest", ability)?;
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        let decision = driver.decision()?.clone();
        driver.answer(Driver::passive_answer(&decision))?;
    }
    assert_eq!(r405_castable(&driver)?, offered);
    Ok(())
}

#[rstest]
#[case::an_equipment(2, &[Artifact], &["Equipment"], true)]
#[case::an_artifact(3, &[Artifact], &[], false)]
#[case::a_creature(4, &[Creature], &[], false)]
fn freya_pays_for_equipment_spells(
    #[case] seats: usize,
    #[case] types: &[CardType],
    #[case] subtypes: &[&str],
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = r405_table(
        seats,
        r405_spell(types, subtypes, "{R}", 0),
        &["Freya Crescent"],
    )?;
    assert_eq!(r405_castable(&driver)?, offered);
    Ok(())
}

/// Freya's mana pays Bonesplitter's equip ability; a land for any ability beside it is not needed.
#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn freya_pays_for_an_equip_ability(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut driver = r405_table(
        seats,
        r405_spell(&[Sorcery], &[], "{9}", 0),
        &["Freya Crescent", "Bonesplitter"],
    )?;
    assert_eq!(driver.offered_abilities("Bonesplitter")?.len(), 1);
    driver.begin_ability("Bonesplitter", 0)?;
    let target = driver.target_named("Freya Crescent")?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r405_settle(&mut driver)?;
    let splitter = driver.id_of("Bonesplitter")?;
    let freya = driver.id_of("Freya Crescent")?;
    assert_eq!(
        driver
            .state()
            .object(splitter)
            .and_then(|o| o.permanent())
            .and_then(|p| p.attached_to())
            .map(|host| host.id),
        Some(freya)
    );
    r405_replays(&driver);
    Ok(())
}

#[rstest]
#[case::an_instant(2, &[Instant], true)]
#[case::a_sorcery(3, &[Sorcery], true)]
#[case::a_creature(4, &[Creature], false)]
fn the_observer_pays_for_instants_and_sorceries(
    #[case] seats: usize,
    #[case] types: &[CardType],
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = r405_table(
        seats,
        r405_spell(types, &[], "{U}", 0),
        &["Unblinking Observer"],
    )?;
    assert_eq!(r405_castable(&driver)?, offered);
    Ok(())
}

/// A creature cast with the Attendant's mana enters with a counter unless it is a Human.
#[rstest]
#[case::an_elf(2, &["Elf"], 1)]
#[case::a_human(3, &["Human"], 0)]
#[case::a_human_elf(4, &["Human", "Elf"], 0)]
fn the_attendant_gives_a_counter_to_non_humans(
    #[case] seats: usize,
    #[case] subtypes: &[&str],
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let mut driver = r405_table(
        seats,
        r405_spell(&[Creature], subtypes, "{G}", 0),
        &["Animal Attendant"],
    )?;
    driver.cast(SPELL)?;
    r405_settle(&mut driver)?;
    let spell = driver.id_of(SPELL)?;
    let object = driver.state().object(spell).unwrap();
    assert_eq!(object.zone(), Zone::Battlefield);
    assert_eq!(
        object
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne),
        counters
    );
    r405_replays(&driver);
    Ok(())
}

/// Seat 0 casts Throne of Eldraine with five Wastes, beside `lands`, choosing red, and holds `spell`.
fn r405_throne(
    seats: usize,
    lands: &[&str],
    spell: Arc<CardDefinition>,
) -> Result<Driver, ScriptError> {
    let mut mine: Vec<_> = (0..5).map(|_| ready(wastes())).collect();
    mine.extend(lands.iter().map(|name| match *name {
        "Forest" => ready(forest()),
        _ => ready(mountain()),
    }));
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [card("Throne of Eldraine").unwrap(), spell])
        .battlefield(0, mine);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| bear()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Throne of Eldraine", &["Wastes"; 5])?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                break;
            }
            DecisionKind::ChooseOption(options) => {
                let red = options
                    .options
                    .iter()
                    .position(|o| matches!(o, ChoiceOption::Color(Color::Red)))
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(red)))?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    let throne = driver.id_of("Throne of Eldraine")?;
    assert_eq!(
        driver.state().object(throne).unwrap().zone(),
        Zone::Battlefield
    );
    Ok(driver)
}

/// Throne of Eldraine's mana pays for monocolored spells of the color chosen as it entered, and nothing else.
#[rstest]
#[case::monocolored_red(2, &[Creature], "{R}", true)]
#[case::more_red(3, &[Sorcery], "{2}{R}{R}", true)]
#[case::monocolored_green(3, &[Creature], "{G}", false)]
#[case::red_and_white(4, &[Creature], "{R}{W}", false)]
#[case::colorless(4, &[Artifact], "{2}", false)]
fn the_throne_pays_for_monocolored_spells_of_its_color(
    #[case] seats: usize,
    #[case] types: &[CardType],
    #[case] cost: &str,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = r405_throne(seats, &[], r405_spell(types, &[], cost, 0))?;
    assert_eq!(r405_castable(&driver)?, offered);
    r405_replays(&driver);
    Ok(())
}

/// The throne's draw ability is paid with mana of the chosen color only: three Mountains pay it, a Mountain and two
/// Forests do not.
#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
fn the_thrones_draw_is_paid_with_the_chosen_color(#[case] seats: usize) -> Result<(), ScriptError> {
    let spell = || r405_spell(&[Instant], &[], "{9}", 0);
    let red = r405_throne(seats, &["Mountain", "Mountain", "Mountain"], spell())?;
    let mixed = r405_throne(seats, &["Mountain", "Forest", "Forest"], spell())?;
    let offered = red.offered_abilities("Throne of Eldraine")?;
    let short = mixed.offered_abilities("Throne of Eldraine")?;
    assert_eq!(offered.len(), short.len() + 1, "{offered:?} {short:?}");
    Ok(())
}

/// Enthralling Hold can't be cast at an untapped creature, and a tapped one that untaps before it resolves is still
/// taken.
#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn the_hold_targets_only_tapped_creatures_as_it_is_cast(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [card("Enthralling Hold").unwrap()])
        .battlefield(
            0,
            [
                ready(island()),
                ready(island()),
                ready(island()),
                ready(island()),
                ready(island()),
            ],
        );
    let foe = seats - 1;
    scenario = scenario.battlefield(
        foe,
        [
            tapped(card("Grizzly Bears").unwrap()),
            ready(card("Llanowar Elves").unwrap()),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| bear()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let bears = driver.target_named("Grizzly Bears")?;
    let elves = driver.target_named("Llanowar Elves")?;
    driver.begin_cast("Enthralling Hold")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the target is asked");
    };
    assert!(options.legal.contains(&bears), "a tapped creature");
    assert!(!options.legal.contains(&elves), "an untapped creature");
    driver.choose_targets(&[bears])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r405_settle(&mut driver)?;
    let id = driver.id_of("Grizzly Bears")?;
    assert_eq!(
        mtg_engine::characteristics::controller_of(driver.state(), id),
        Some(pid(0))
    );
    r405_replays(&driver);
    Ok(())
}

/// Magewright's Stone untaps a creature with a {T} ability and can't target one without.
#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn the_stone_targets_creatures_with_a_tap_ability(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Magewright's Stone").unwrap()),
            ready(forest()),
            tapped(card("Llanowar Elves").unwrap()),
            tapped(card("Grizzly Bears").unwrap()),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| bear()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let elves = driver.target_named("Llanowar Elves")?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.begin_ability("Magewright's Stone", 0)?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the target is asked");
    };
    assert!(options.legal.contains(&elves));
    assert!(!options.legal.contains(&bears));
    driver.choose_targets(&[elves])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r405_settle(&mut driver)?;
    let id = driver.id_of("Llanowar Elves")?;
    assert!(
        !driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    r405_replays(&driver);
    Ok(())
}

/// Break the Ice targets a land that is snow or could produce {C}, and not a Forest.
#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
fn break_the_ice_targets_snow_or_colorless_lands(#[case] seats: usize) -> Result<(), ScriptError> {
    let foe = seats - 1;
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [card("Break the Ice").unwrap()])
        .battlefield(0, (0..2).map(|_| ready(mtg_testkit::cards::swamp())))
        .battlefield(
            foe,
            [ready(wastes()), ready(forest()), ready(r405_snowy_forest())],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| bear()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let snow = driver.target_named("Snowy Forest")?;
    let forest = driver.target_named("Forest")?;
    let wastes = driver.target_named("Wastes")?;
    driver.begin_cast("Break the Ice")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the target is asked");
    };
    assert!(options.legal.contains(&snow));
    assert!(options.legal.contains(&wastes));
    assert!(!options.legal.contains(&forest));
    Ok(())
}
