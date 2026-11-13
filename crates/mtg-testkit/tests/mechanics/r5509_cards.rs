//! Goal: the r55-09 cards play as their rulings say at two to four seats: a face-down pile is exiled, shuffled and
//! manifested, cloaked tapped or put back on top of the library (Ghastly Conscription, Become Anonymous, Triumph of Saint
//! Katherine, whose pile does nothing when the library is short: "If you have fewer than six cards in your library ... no
//! cards will be exiled"), Braided Net's prohibition lasts while the permanent stays tapped, and The Foretold
//! Soldier, Overclocked Electromancer and Wildgrowth Archaic do what they print.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::PresetPermanent;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// Priority passes until the stack is empty.
fn r5509_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

fn r5509_lands(make: fn() -> Arc<CardDefinition>, count: usize) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(make())).collect()
}

fn r5509_seat(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

/// The face-down permanents `seat` controls.
fn r5509_face_down(driver: &Driver, seat: usize) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.face_down().is_some() && o.controller() == r5509_seat(seat))
        })
        .collect()
}

fn r5509_shuffled(events: &[Event]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, Event::ChosenAtRandom { .. }))
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 3)]
fn r5509_ghastly_conscription_manifests_the_creature_cards_of_a_graveyard(
    #[case] seats: usize,
    #[case] victim: usize,
) -> Result<(), ScriptError> {
    let mut lands = r5509_lands(swamp, 7);
    lands.truncate(7);
    let mut scenario = Scenario::new(seats)
        .battlefield(0, lands)
        .hand(0, [card("Ghastly Conscription").unwrap()])
        .graveyard(
            victim,
            [
                free_creature("Ghoul", 3, 3),
                free_creature("Zombie", 2, 2),
                island(),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, vec![ready(free_creature("Pip", 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    let target = driver.target_player(victim);
    let mut events = driver.cast_at("Ghastly Conscription", &[target])?;
    for _ in 0..60 {
        if driver.state().stack().is_empty() {
            break;
        }
        events.extend(driver.pass()?);
    }
    let manifested = r5509_face_down(&driver, 0);
    assert_eq!(manifested.len(), 2, "both creature cards are manifested");
    for id in &manifested {
        let object = driver.state().object(*id).unwrap();
        assert_eq!(
            object.owner(),
            r5509_seat(victim),
            "the owner keeps the card"
        );
    }
    assert_eq!(
        driver
            .state()
            .zone(Zone::Graveyard(r5509_seat(victim)))
            .len(),
        1,
        "the land stays"
    );
    assert!(r5509_shuffled(&events), "the pile is shuffled");
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn r5509_become_anonymous_cloaks_the_pile_tapped(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, {
            let mut permanents = r5509_lands(island, 4);
            permanents.push(ready(free_creature("Pip", 1, 1)));
            permanents
        })
        .hand(0, [card("Become Anonymous").unwrap()])
        .library(0, (0..5).map(|_| forest()).collect::<Vec<_>>());
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, vec![ready(free_creature("Pip", 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    let pip = *driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|o| o.card().name == "Pip" && o.controller() == r5509_seat(0))
        })
        .unwrap();
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(pip).unwrap().reference());
    driver.cast_at("Become Anonymous", &[target])?;
    r5509_settle(&mut driver)?;
    let cloaked = r5509_face_down(&driver, 0);
    assert_eq!(cloaked.len(), 3, "the creature and two cards");
    for id in cloaked {
        let object = driver.state().object(id).unwrap();
        assert!(object.permanent().unwrap().tapped(), "they enter tapped");
        assert!(
            mtg_engine::keywords::keywords_of(driver.state(), id)
                .iter()
                .any(|k| matches!(k, mtg_cards::keyword::Keyword::Ward(_))),
            "a cloaked permanent has ward"
        );
    }
    assert_eq!(driver.state().zone(Zone::Library(r5509_seat(0))).len(), 3);
    Ok(())
}

#[rstest]
#[case::whole_two_seats(2, 10, true)]
#[case::whole_three_seats(3, 6, true)]
#[case::short_library_two_seats(2, 5, false)]
#[case::short_library_four_seats(4, 3, false)]
fn r5509_triumph_goes_back_on_top_with_six_cards_or_not_at_all(
    #[case] seats: usize,
    #[case] library: usize,
    #[case] whole: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, {
            let mut permanents = r5509_lands(swamp, 2);
            permanents.push(ready(card("Triumph of Saint Katherine").unwrap()));
            permanents
        })
        .hand(0, [card("Doom Blade").unwrap()])
        .library(0, (0..library).map(|_| plains()).collect::<Vec<_>>());
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, vec![ready(free_creature("Pip", 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    let target = driver.target_named("Triumph of Saint Katherine")?;
    driver.cast_at("Doom Blade", &[target])?;
    r5509_settle(&mut driver)?;
    let me = r5509_seat(0);
    let in_library = driver.state().zone(Zone::Library(me)).len();
    let in_graveyard =
        driver.state().zone(Zone::Graveyard(me)).iter().any(|id| {
            driver.state().object(*id).unwrap().card().name == "Triumph of Saint Katherine"
        });
    if whole {
        assert_eq!(in_library, library + 1, "the pile is back on top");
        assert!(!in_graveyard);
        let top: Vec<ObjectId> = driver
            .state()
            .zone(Zone::Library(me))
            .iter()
            .rev()
            .take(7)
            .copied()
            .collect();
        assert!(
            top.iter()
                .any(|id| driver.state().object(*id).unwrap().card().name
                    == "Triumph of Saint Katherine")
        );
    } else {
        assert_eq!(in_library, library, "nothing is exiled");
        assert!(in_graveyard, "Triumph stays in the graveyard");
    }
    Ok(())
}

/// A creature with "{0}: You gain 1 life.", an ability without {T}.
fn r5509_totem() -> Arc<CardDefinition> {
    let ability: mtg_cards::ability::Ability = serde_json::from_str(
        r#"{"Activated":{"cost":"{0}","effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Totem")
            .with_type(mtg_core::card::CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability),
    )
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5509_braided_net_bans_activations_while_the_permanent_stays_tapped(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        vec![
            with_counters(
                ready(card("Braided Net // Braided Quipu").unwrap()),
                mtg_cards::value::CounterKind::Other("net".into()),
                3,
            ),
            ready(r5509_totem()),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()).collect::<Vec<_>>());
    }
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    assert!(!driver.offered_abilities("Totem")?.is_empty());
    let target = driver.target_named("Totem")?;
    driver.begin_activation("Braided Net")?;
    driver.choose_targets(&[target])?;
    r5509_settle(&mut driver)?;
    let totem = driver.id_of("Totem")?;
    assert!(
        driver
            .state()
            .object(totem)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert!(
        driver.offered_abilities("Totem")?.is_empty(),
        "banned while tapped"
    );
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    assert!(
        !driver
            .state()
            .object(totem)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert!(
        !driver.offered_abilities("Totem")?.is_empty(),
        "the ban ends as it untaps"
    );
    Ok(())
}

/// Answers passively until the pending decision is one `wanted` accepts.
fn r5509_until(driver: &mut Driver, wanted: fn(&DecisionKind) -> bool) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if wanted(&driver.decision()?.kind) {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the wanted decision".into()))
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5509_the_foretold_soldier_is_foretold_after_it_deals_damage(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario =
        Scenario::new(seats).battlefield(0, vec![ready(card("The Foretold Soldier").unwrap())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()).collect::<Vec<_>>());
    }
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    let soldier = driver.id_of("The Foretold Soldier")?;
    r5509_until(&mut driver, |kind| {
        matches!(kind, DecisionKind::DeclareAttackers(_))
    })?;
    driver.attack(&[("The Foretold Soldier", 1)])?;
    driver.idle_until(1, Step::Main2)?;
    let object = driver.state().object(soldier).unwrap();
    assert_eq!(object.zone(), Zone::Exile);
    assert!(
        matches!(
            object.face_down(),
            Some(mtg_engine::object::FaceDown::Foretold { turn: 1 })
        ),
        "foretold this turn: {:?}",
        object.face_down()
    );
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5509_overclocked_electromancer_gets_the_excess_as_energy(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(card("Overclocked Electromancer").unwrap())])
        .battlefield(1, vec![ready(free_creature("Pip", 1, 1))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()).collect::<Vec<_>>());
    }
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    r5509_until(&mut driver, |kind| {
        matches!(kind, DecisionKind::DeclareAttackers(_))
    })?;
    driver.attack(&[("Overclocked Electromancer", 1)])?;
    r5509_until(&mut driver, |kind| {
        matches!(kind, DecisionKind::DeclareBlockers(_))
    })?;
    driver.block(&[("Pip", "Overclocked Electromancer")])?;
    driver.idle_until(1, Step::Main2)?;
    let energy = driver
        .state()
        .player(r5509_seat(0))
        .unwrap()
        .counters_of(&mtg_cards::value::CounterKind::Other("energy".into()));
    // The attack doubled its power to 4; the 1/1 took 3 damage beyond lethal.
    assert_eq!(energy, 3);
    Ok(())
}

#[rstest]
#[case::two_colors_two_seats(2, true, 2)]
#[case::two_colors_four_seats(4, true, 2)]
#[case::one_color_three_seats(3, false, 1)]
fn r5509_wildgrowth_archaic_adds_a_counter_per_color_spent(
    #[case] seats: usize,
    #[case] red: bool,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let second = if red {
        ready(mtg_testkit::cards::mountain())
    } else {
        ready(forest())
    };
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            vec![
                // A 0/0 that entered with its converge counters.
                with_counters(
                    ready(card("Wildgrowth Archaic").unwrap()),
                    mtg_cards::value::CounterKind::PlusOnePlusOne,
                    2,
                ),
                ready(forest()),
                second,
            ],
        )
        .hand(0, [card("Grizzly Bears").unwrap()]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()).collect::<Vec<_>>());
    }
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    driver.cast("Grizzly Bears")?;
    r5509_settle(&mut driver)?;
    let bears = driver.id_of("Grizzly Bears")?;
    let on = driver
        .state()
        .object(bears)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne);
    assert_eq!(on, counters);
    Ok(())
}
