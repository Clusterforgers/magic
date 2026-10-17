//! Goal: characteristics are derived from base values plus continuous effects, applied by layer then timestamp, honouring each effect's duration.

mod common;

use common::names;
use mtg_core::card::CardType;
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::ObjectId;
use mtg_engine::effects::{Duration, Layer, Modification, Selector};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics};
use mtg_testkit::cards::{bear, forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, damaged, ready};
use rstest::rstest;

fn pump(power: i32, toughness: i32) -> Modification {
    Modification::ModifyPowerToughness { power, toughness }
}

fn set(power: i32, toughness: i32) -> Modification {
    Modification::SetPowerToughness { power, toughness }
}

fn stats(driver: &Driver, id: ObjectId) -> Option<(i32, i32)> {
    let chars = characteristics(driver.state(), id)?;
    Some((chars.power?, chars.toughness?))
}

fn with_subject(base: (i32, i32)) -> Result<(Driver, ObjectId), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(free_creature("Subject", base.0, base.1)),
            ready(forest()),
        ],
    ))?;
    let id = driver.id_of("Subject")?;
    Ok((driver, id))
}

#[rstest]
#[case::no_effects(vec![], (6, 6))]
#[case::one_pump(vec![pump(1, 1)], (7, 7))]
#[case::pumps_add_up(vec![pump(1, 1), pump(2, 0), pump(0, 3)], (9, 10))]
#[case::debuff(vec![pump(-2, -3)], (4, 3))]
#[case::set_alone(vec![set(1, 1)], (1, 1))]
#[case::set_then_pump(vec![set(1, 1), pump(2, 2)], (3, 3))]
#[case::pump_then_set_still_pumps_after(vec![pump(2, 2), set(1, 1)], (3, 3))]
#[case::later_set_wins(vec![set(4, 4), set(1, 1)], (1, 1))]
#[case::earlier_set_loses(vec![set(1, 1), set(4, 4)], (4, 4))]
#[case::pump_set_pump(vec![pump(1, 0), set(0, 1), pump(3, 0)], (4, 1))]
#[case::set_is_not_a_pump(vec![set(2, 9), pump(-1, -1), set(5, 5)], (4, 4))]
fn power_and_toughness_follow_layer_then_timestamp(
    #[case] effects: Vec<Modification>,
    #[case] expected: (i32, i32),
) -> Result<(), ScriptError> {
    let (mut driver, id) = with_subject((6, 6))?;
    for modification in effects {
        driver.add_effect(Selector::Object(id), modification, Duration::UntilEndOfTurn)?;
    }
    assert_eq!(stats(&driver, id), Some(expected));
    Ok(())
}

#[test]
fn effects_are_applied_in_cr_613_layer_order() {
    let order = [
        Layer::Copy,
        Layer::Control,
        Layer::Text,
        Layer::Type,
        Layer::Color,
        Layer::Ability,
        Layer::PtCharacteristicDefining,
        Layer::PtSet,
        Layer::PtModify,
        Layer::PtSwitch,
    ];
    assert!(order.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(
        Modification::AddType(CardType::Creature).layer(),
        Layer::Type
    );
    assert_eq!(
        Modification::SetColors(ColorSet::EMPTY).layer(),
        Layer::Color
    );
    assert_eq!(set(1, 1).layer(), Layer::PtSet);
    assert_eq!(pump(1, 1).layer(), Layer::PtModify);
}

#[test]
fn a_type_change_in_an_earlier_layer_makes_later_power_toughness_effects_apply()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(forest())]))?;
    let land = driver.id_of("Forest")?;
    // Registered in the "wrong" order on purpose: pump first, then the set, then the type change.
    driver.add_effect(Selector::Object(land), pump(1, 1), Duration::UntilEndOfTurn)?;
    assert_eq!(
        stats(&driver, land),
        None,
        "a plain land has no power or toughness to modify"
    );
    driver.add_effect(Selector::Object(land), set(3, 3), Duration::UntilEndOfTurn)?;
    driver.add_effect(
        Selector::Object(land),
        Modification::AddType(CardType::Creature),
        Duration::UntilEndOfTurn,
    )?;
    assert_eq!(stats(&driver, land), Some((4, 4)));
    assert!(characteristics(driver.state(), land).is_some_and(|c| c.is_creature()));
    assert_eq!(
        driver.state().object(land).map(|o| o.card().power),
        Some(None),
        "the card itself never changed"
    );
    Ok(())
}

#[test]
fn creature_selectors_see_types_added_by_earlier_layers() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(forest()), ready(bear())])
            .battlefield(1, [ready(free_creature("Theirs", 1, 1))]),
    )?;
    let land = driver.id_of("Forest")?;
    let theirs = driver.id_of("Theirs")?;
    let ours = driver.id_of("Grizzly Bears")?;
    driver.add_effect(Selector::AllCreatures, pump(1, 1), Duration::UntilEndOfTurn)?;
    assert_eq!(
        (
            stats(&driver, ours),
            stats(&driver, theirs),
            stats(&driver, land)
        ),
        (Some((3, 3)), Some((2, 2)), None)
    );
    // Power and toughness are set first: a creature without a toughness would die to state-based actions.
    driver.add_effect(Selector::Object(land), set(0, 1), Duration::UntilEndOfTurn)?;
    assert_eq!(
        stats(&driver, land),
        Some((0, 1)),
        "not a creature yet, so the all-creatures pump skips it"
    );
    driver.add_effect(
        Selector::Object(land),
        Modification::AddType(CardType::Creature),
        Duration::UntilEndOfTurn,
    )?;
    assert_eq!(
        stats(&driver, land),
        Some((1, 2)),
        "the earlier all-creatures pump now reaches the land"
    );
    driver.add_effect(
        Selector::CreaturesControlledBy(mtg_core::ids::PlayerId(1)),
        pump(5, 5),
        Duration::UntilEndOfTurn,
    )?;
    assert_eq!(
        (stats(&driver, theirs), stats(&driver, ours)),
        (Some((7, 7)), Some((3, 3)))
    );
    Ok(())
}

#[rstest]
#[case::white(ColorSet::EMPTY.with(Color::White))]
#[case::two_colors(ColorSet::EMPTY.with(Color::Red).with(Color::Blue))]
#[case::colorless(ColorSet::EMPTY)]
fn color_changing_effects_replace_colors(#[case] colors: ColorSet) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(bear())]))?;
    let id = driver.id_of("Grizzly Bears")?;
    assert_eq!(
        characteristics(driver.state(), id).map(|c| c.colors),
        Some(ColorSet::EMPTY.with(Color::Green))
    );
    driver.add_effect(
        Selector::Object(id),
        Modification::SetColors(colors),
        Duration::UntilEndOfTurn,
    )?;
    assert_eq!(
        characteristics(driver.state(), id).map(|c| c.colors),
        Some(colors)
    );
    Ok(())
}

#[rstest]
#[case(Step::Main1, Step::End)]
#[case(Step::Upkeep, Step::End)]
fn until_end_of_turn_effects_last_exactly_until_cleanup(
    #[case] created_in: Step,
    #[case] still_active_in: Step,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, created_in)
            .battlefield(0, [ready(free_creature("Subject", 2, 2))]),
    )?;
    let id = driver.id_of("Subject")?;
    driver.add_effect(Selector::Object(id), pump(1, 1), Duration::UntilEndOfTurn)?;
    assert_eq!(stats(&driver, id), Some((3, 3)));
    driver.idle_until(2, still_active_in)?;
    assert_eq!(
        stats(&driver, id),
        Some((3, 3)),
        "still buffed in {still_active_in:?}"
    );
    driver.idle_until(3, Step::Upkeep)?;
    assert_eq!(stats(&driver, id), Some((2, 2)), "gone in the next turn");
    assert_eq!(driver.state().effects().len(), 0);
    driver.expect_event("EffectExpired", |e| {
        matches!(e, Event::EffectExpired { .. })
    })
}

#[test]
fn effects_created_in_different_turns_expire_with_their_own_turn() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .battlefield(0, [ready(free_creature("Subject", 2, 2))]),
    )?;
    let id = driver.id_of("Subject")?;
    driver.add_effect(Selector::Object(id), pump(1, 1), Duration::UntilEndOfTurn)?;
    driver.idle_until(3, Step::Main1)?;
    driver.add_effect(Selector::Object(id), pump(5, 5), Duration::UntilEndOfTurn)?;
    assert_eq!(
        stats(&driver, id),
        Some((7, 7)),
        "the turn-2 pump is gone, only the new one applies"
    );
    assert_eq!(driver.state().effects().len(), 1);
    Ok(())
}

#[rstest]
#[case::source_stays(false, (4, 4))]
#[case::source_leaves(true, (2, 2))]
fn while_source_on_battlefield_ends_when_the_source_leaves(
    #[case] source_dies: bool,
    #[case] expected: (i32, i32),
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(free_creature("Subject", 2, 2)),
            ready(free_creature("Lord", 1, 1)),
        ],
    ))?;
    let (subject, lord) = (driver.id_of("Subject")?, driver.id_of("Lord")?);
    driver.add_effect(
        Selector::Object(subject),
        pump(2, 2),
        Duration::WhileSourceOnBattlefield(lord),
    )?;
    assert_eq!(stats(&driver, subject), Some((4, 4)));
    if source_dies {
        driver.add_effect(
            Selector::Object(lord),
            pump(-1, -1),
            Duration::UntilEndOfTurn,
        )?;
        assert_eq!(names(driver.state(), Zone::Battlefield), ["Subject"]);
        assert_eq!(
            driver.state().effects().len(),
            1,
            "the lapsed buff was pruned, the debuff remains"
        );
    }
    // The buff outlives turns for as long as its source does.
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(stats(&driver, subject), Some(expected));
    Ok(())
}

#[test]
fn a_buff_can_keep_a_damaged_creature_alive_only_while_it_lasts() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [damaged(free_creature("Subject", 2, 3), 2)]),
    )?;
    let id = driver.id_of("Subject")?;
    driver.add_effect(Selector::Object(id), pump(0, 1), Duration::UntilEndOfTurn)?;
    assert_eq!(stats(&driver, id), Some((2, 4)));
    driver.add_effect(Selector::Object(id), pump(0, -1), Duration::UntilEndOfTurn)?;
    assert_eq!(
        names(driver.state(), Zone::Battlefield),
        ["Subject"],
        "2 damage on toughness 3 survives"
    );
    driver.add_effect(Selector::Object(id), pump(0, -1), Duration::UntilEndOfTurn)?;
    assert!(
        names(driver.state(), Zone::Battlefield).is_empty(),
        "damage 2 on toughness 2 is lethal"
    );
    Ok(())
}

#[test]
fn effect_ids_and_timestamps_strictly_increase() -> Result<(), ScriptError> {
    let (mut driver, id) = with_subject((1, 1))?;
    for _ in 0..4 {
        driver.add_effect(Selector::Object(id), pump(1, 1), Duration::UntilEndOfTurn)?;
    }
    let effects = driver.state().effects();
    assert!(
        effects
            .windows(2)
            .all(|w| w[0].id < w[1].id && w[0].timestamp < w[1].timestamp)
    );
    assert!(effects.iter().all(|e| e.layer == Layer::PtModify));
    Ok(())
}

#[test]
fn effects_only_touch_the_battlefield() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [free_creature("Unplayed", 2, 2)])
            .battlefield(0, [ready(forest())]),
    )?;
    let in_hand = driver.state().zone(Zone::Hand(mtg_core::ids::PlayerId(0)))[0];
    driver.add_effect(Selector::AllCreatures, pump(5, 5), Duration::UntilEndOfTurn)?;
    assert_eq!(stats(&driver, in_hand), Some((2, 2)));
    Ok(())
}
