//! Goal: the cards the compiler reads for a move between zones play whole at tables of two, three and four seats: up to a number of creature cards return from the caster's graveyard to the hand or the battlefield (tapped or not), a card of another player's graveyard enters under the caster's control, every card of a land type returns at once, a graveyard is exiled, a card that left its graveyard before the spell resolved is not moved, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::compile::compile;
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, replay};
use mtg_testkit::cards::{cost, free_creature, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// A sorcery for `{0}` with the spell ability the compiler reads from this sentence.
fn reclaim(text: &str) -> Arc<CardDefinition> {
    let printed = PrintedCard {
        oracle_id: "12345678-1234-1234-1234-123456789abd".to_owned(),
        name: "Reclaim".to_owned(),
        faces: vec![PrintedFace {
            name: "Reclaim".to_owned(),
            type_line: "Sorcery".to_owned(),
            oracle_text: text.to_owned(),
            ..PrintedFace::default()
        }],
        ..PrintedCard::default()
    };
    let card = compile(&printed).card.expect("the card compiles");
    let rules = card.faces[0].abilities[0].clone();
    Arc::new(
        CardDefinition::new("Reclaim")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

/// Seat 0 holds the spell with a Bear, an Ogre and a Plains in its graveyard; seat 1 has a Wolf and a Forest in its
/// graveyard. The game stands in seat 0's first main phase.
fn table(seats: usize, text: &str) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [reclaim(text)])
        .graveyard(
            0,
            [
                free_creature("Bear", 2, 2),
                free_creature("Ogre", 3, 3),
                plains(),
            ],
        )
        .graveyard(1, [free_creature("Wolf", 2, 2), plains()]);
    Driver::from_scenario(scenario)
}

/// The card with this name in a seat's graveyard, as a target.
fn in_graveyard(driver: &Driver, seat: usize, name: &str) -> Target {
    let state = driver.state();
    let id = state
        .zone(Zone::Graveyard(pid(seat)))
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .expect("the card is in the graveyard");
    Target::Object(state.object(id).expect("the object").reference())
}

/// Every seat passes once, so the top of the stack resolves.
fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..driver.state().players().len() {
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
            driver.pass()?;
        }
    }
    Ok(())
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn permanent(state: &GameState, name: &str) -> Option<ObjectId> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
}

fn is_tapped(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .and_then(|o| o.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped)
}

#[rstest]
#[case::to_the_battlefield(
    "Return target creature card from your graveyard to the battlefield.",
    false
)]
#[case::tapped(
    "Return target creature card from your graveyard to the battlefield tapped.",
    true
)]
fn a_creature_card_returns_to_the_battlefield(
    #[case] text: &str,
    #[case] tapped: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, text)?;
    let target = in_graveyard(&driver, 0, "Ogre");
    driver.cast_at("Reclaim", &[target])?;
    resolve(&mut driver)?;
    let state = driver.state();
    let id = permanent(state, "Ogre").expect("the Ogre returned");
    assert_eq!(is_tapped(state, id), tapped);
    assert_eq!(
        names(state, Zone::Graveyard(pid(0))).len(),
        3,
        "Reclaim, Bear and Plains"
    );
    assert!(names(state, Zone::Graveyard(pid(0))).contains(&"Bear".to_owned()));
    assert_eq!(
        names(state, Zone::Graveyard(pid(1))).len(),
        2,
        "the other graveyard is untouched"
    );
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_of_two(&["Bear", "Ogre"])]
#[case::one_of_two(&["Ogre"])]
#[case::none(&[])]
fn up_to_two_creature_cards_return_to_the_hand(
    #[case] chosen: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(
        seats,
        "Return up to two target creature cards from your graveyard to your hand.",
    )?;
    let targets: Vec<Target> = chosen
        .iter()
        .map(|name| in_graveyard(&driver, 0, name))
        .collect();
    driver.cast_at("Reclaim", &targets)?;
    resolve(&mut driver)?;
    let state = driver.state();
    let hand = names(state, Zone::Hand(pid(0)));
    assert_eq!(hand.len(), chosen.len());
    assert!(chosen.iter().all(|name| hand.contains(&(*name).to_owned())));
    assert!(permanent(state, "Ogre").is_none());
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::any_graveyard(
    "Put target creature card from a graveyard onto the battlefield under your control.",
    "Wolf",
    1,
    false
)]
#[case::your_own(
    "Put target creature card from a graveyard onto the battlefield under your control.",
    "Ogre",
    0,
    false
)]
#[case::an_opponents_tapped(
    "Put target creature card from an opponent's graveyard onto the battlefield tapped under your control.",
    "Wolf",
    1,
    true
)]
fn a_creature_card_of_another_graveyard_enters_under_the_casters_control(
    #[case] text: &str,
    #[case] name: &str,
    #[case] owner: usize,
    #[case] tapped: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, text)?;
    let target = in_graveyard(&driver, owner, name);
    driver.cast_at("Reclaim", &[target])?;
    resolve(&mut driver)?;
    let state = driver.state();
    let id = permanent(state, name).expect("the creature entered");
    assert_eq!(
        mtg_engine::characteristics::controller_of(state, id),
        Some(pid(0))
    );
    assert_eq!(is_tapped(state, id), tapped);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn every_land_card_of_the_graveyard_returns_tapped(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(
        seats,
        "Return all land cards from your graveyard to the battlefield tapped.",
    )?;
    driver.begin_cast("Reclaim")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    resolve(&mut driver)?;
    let state = driver.state();
    let id = permanent(state, "Plains").expect("the Plains returned");
    assert!(is_tapped(state, id));
    assert!(names(state, Zone::Graveyard(pid(0))).contains(&"Bear".to_owned()));
    assert_eq!(
        names(state, Zone::Graveyard(pid(1))),
        ["Wolf", "Plains"],
        "the other graveyard keeps its lands"
    );
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_graveyard_is_exiled(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = table(seats, "Exile target player's graveyard.")?;
    let target = driver.target_player(1);
    driver.cast_at("Reclaim", &[target])?;
    resolve(&mut driver)?;
    let state = driver.state();
    assert_eq!(names(state, Zone::Graveyard(pid(1))).len(), 0);
    assert!(names(state, Zone::Exile).contains(&"Wolf".to_owned()));
    assert!(names(state, Zone::Exile).contains(&"Plains".to_owned()));
    assert_eq!(
        names(state, Zone::Graveyard(pid(0))).len(),
        4,
        "the caster's graveyard stays with Reclaim"
    );
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_card_returns_with_a_minus_counter_on_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(
        seats,
        "Return target nonlegendary creature card from your graveyard to the battlefield with a -1/-1 counter on it.",
    )?;
    let target = in_graveyard(&driver, 0, "Ogre");
    driver.cast_at("Reclaim", &[target])?;
    resolve(&mut driver)?;
    let state = driver.state();
    let id = permanent(state, "Ogre").expect("the Ogre returned");
    let counters = state
        .object(id)
        .and_then(|o| o.permanent())
        .map(|p| p.counters_of(&CounterKind::MinusOneMinusOne));
    assert_eq!(counters, Some(1), "it entered with the counter");
    assert_replays(&driver);
    Ok(())
}

/// Seat 0 holds the spell and controls a Bear; seat 1 controls a Wolf and an Ogre.
fn crowded_table(seats: usize, text: &str) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [reclaim(text)])
        .battlefield(0, [ready(free_creature("Bear", 2, 2))])
        .battlefield(
            1,
            [
                ready(free_creature("Wolf", 2, 2)),
                ready(free_creature("Ogre", 3, 3)),
            ],
        );
    Driver::from_scenario(scenario)
}

#[rstest]
fn exactly_two_target_creatures_return_to_their_owners_hands(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let text = "Return two target creatures to their owners' hands.";
    let mut driver = crowded_table(seats, text)?;
    let one = [driver.target_named("Wolf")?];
    assert!(
        driver.cast_at("Reclaim", &one).is_err(),
        "one target is too few"
    );
    let mut driver = crowded_table(seats, text)?;
    let both = [driver.target_named("Wolf")?, driver.target_named("Ogre")?];
    driver.cast_at("Reclaim", &both)?;
    resolve(&mut driver)?;
    let state = driver.state();
    assert_eq!(
        names(state, Zone::Hand(pid(1))),
        ["Wolf", "Ogre"],
        "both went to their owner's hand"
    );
    assert!(permanent(state, "Bear").is_some(), "the third stays");
    assert_replays(&driver);
    Ok(())
}
