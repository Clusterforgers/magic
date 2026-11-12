//! Goal: the compiled statics of this piece play as their text says at tables of two to four seats: a group prevention stops the damage of a burn to each member of the group and to nothing else (Crystal Barricade spares itself, Bubble Matrix covers every creature); "during your turn, prevent all damage that would be dealt to you" holds only in its controller's turn (Personal Sanctuary); and a condition on what the attached permanent is gives the bonus only while it holds (Akki War Paint on a creature and an artifact, Butcher's Cleaver on a Human and a Bear).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::has_keyword;
use mtg_engine::object::PermanentState;
use mtg_testkit::cards::{bear, burn, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;
use std::sync::Arc;

/// Passes priority until the stack is empty.
fn c11h_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(());
        };
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn c11h_table(seats: usize) -> Scenario {
    (0..seats).fold(Scenario::new(seats), |scenario, seat| {
        scenario.library(seat, (0..10).map(|_| mountain()).collect::<Vec<_>>())
    })
}

fn c11h_damage(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, PermanentState::damage)
}

fn c11h_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(common::pid(seat)).unwrap().life()
}

#[rstest]
#[case::barricade_spares_another_creature("Crystal Barricade", "Ally", 0)]
#[case::barricade_not_itself("Crystal Barricade", "Crystal Barricade", 2)]
#[case::barricade_not_an_opponents_creature("Crystal Barricade", "Foe", 2)]
#[case::matrix_spares_its_controllers_creature("Bubble Matrix", "Ally", 0)]
#[case::matrix_spares_an_opponents_creature("Bubble Matrix", "Foe", 0)]
fn c11h_a_group_prevention_covers_its_group_alone(
    #[values(2, 3, 4)] seats: usize,
    #[case] holder: &str,
    #[case] target: &str,
    #[case] marked: u32,
) -> Result<(), ScriptError> {
    let scenario = c11h_table(seats)
        .battlefield(
            0,
            [
                ready(card(holder).unwrap()),
                ready(free_creature("Ally", 3, 3)),
            ],
        )
        .battlefield(1, [ready(free_creature("Foe", 3, 3)), ready(mountain())])
        .hand(1, [burn("Spark", 2)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let at = driver.target_named(target)?;
    driver.pass()?;
    driver.cast_at("Spark", &[at])?;
    c11h_settle(&mut driver)?;
    assert_eq!(c11h_damage(&driver, target), marked, "{holder} {target}");
    Ok(())
}

#[rstest]
#[case::in_its_controllers_turn(0, 1, 20)]
#[case::in_another_players_turn(1, 0, 18)]
fn c11h_personal_sanctuary_prevents_only_in_its_controllers_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] holder: usize,
    #[case] caster: usize,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let scenario = c11h_table(seats)
        .battlefield(holder, [ready(card("Personal Sanctuary").unwrap())])
        .battlefield(caster, [ready(mountain())])
        .hand(caster, [burn("Spark", 2)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let at = driver.target_player(holder);
    if caster != 0 {
        driver.pass()?;
    }
    driver.cast_at("Spark", &[at])?;
    c11h_settle(&mut driver)?;
    assert_eq!(c11h_life(&driver, holder), life);
    Ok(())
}

/// An artifact that is not a creature.
fn c11h_relic() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact))
}

/// A 2/2 Human.
fn c11h_human() -> Arc<CardDefinition> {
    Arc::new(
        (*free_creature("Villager", 2, 2))
            .clone()
            .with_subtype("Human"),
    )
}

#[rstest]
#[case::war_paint_on_a_creature("Akki War Paint", bear(), Some((4, 3)), None)]
#[case::war_paint_on_an_artifact("Akki War Paint", c11h_relic(), None, None)]
#[case::cleaver_on_a_human("Butcher's Cleaver", c11h_human(), Some((5, 2)), Some(true))]
#[case::cleaver_on_a_bear("Butcher's Cleaver", bear(), Some((5, 2)), Some(false))]
fn c11h_a_condition_on_the_attached_permanent_gives_while_it_holds(
    #[values(2, 3, 4)] seats: usize,
    #[case] attachment: &str,
    #[case] host: Arc<CardDefinition>,
    #[case] stats: Option<(i32, i32)>,
    #[case] lifelink: Option<bool>,
) -> Result<(), ScriptError> {
    let name = host.name.clone();
    let scenario = c11h_table(seats).battlefield(
        0,
        [
            ready(host),
            attached_to(ready(card(attachment).unwrap()), 0, 0),
        ],
    );
    let driver = Driver::from_scenario(scenario)?;
    let id = driver.id_of(&name)?;
    let chars = characteristics(driver.state(), id).unwrap();
    assert_eq!(
        chars.power.zip(chars.toughness),
        stats,
        "{attachment} on {name}"
    );
    if let Some(lifelink) = lifelink {
        assert_eq!(
            has_keyword(driver.state(), id, &Keyword::Lifelink),
            lifelink
        );
    }
    Ok(())
}
