//! Goal: a cost that exiles a spell the payer controls (`CostItem::ExileSpell`) takes only a matching spell of theirs from
//! the stack, which then never resolves: Nivmagus Elemental exiles an instant or sorcery spell it controls for two +1/+1
//! counters and cannot be activated with only a creature spell or an opponent's spell there, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Priority passes until the stack is empty, payments made automatically and other questions answered passively.
fn r5512_settle(driver: &mut Driver) {
    for _ in 0..80 {
        match driver.decision().unwrap().kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

#[rstest]
#[case::instant("Lightning Bolt", true)]
#[case::creature("Grizzly Bears", false)]
fn r5512_nivmagus_exiles_an_instant_or_sorcery_spell(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] exiles: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Nivmagus Elemental").unwrap()),
                ready(mountain()),
                ready(forest()),
            ],
        )
        .hand(0, [card(spell).unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let life = driver.state().players()[1].life();
    if spell == "Lightning Bolt" {
        let face = driver.target_player(1);
        driver.cast_at(spell, &[face]).unwrap();
    } else {
        driver.cast(spell).unwrap();
    }
    while !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_)) {
        driver.answer_passively().unwrap();
    }
    assert_eq!(
        driver.state().stack().len(),
        1,
        "the spell waits on the stack"
    );
    let activated = driver.begin_activation("Nivmagus Elemental").is_ok();
    assert_eq!(activated, exiles);
    r5512_settle(&mut driver);
    let nivmagus = driver.id_of("Nivmagus Elemental").unwrap();
    let power = characteristics(driver.state(), nivmagus).unwrap().power;
    let state = driver.state();
    let zone = [Zone::Exile, Zone::Battlefield, Zone::Stack]
        .into_iter()
        .find(|zone| {
            state.zone(*zone).iter().any(|id| {
                state
                    .object(*id)
                    .is_some_and(|object| object.card().name == spell)
            })
        })
        .unwrap();
    if exiles {
        assert_eq!(power, Some(3), "two +1/+1 counters on a 1/2");
        assert_eq!(zone, Zone::Exile);
        assert_eq!(
            driver.state().players()[1].life(),
            life,
            "the bolt never resolved"
        );
    } else {
        assert_eq!(power, Some(1));
        assert_eq!(zone, Zone::Battlefield);
    }
}

#[rstest]
fn r5512_nivmagus_cannot_exile_an_opponents_spell(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(3, 1, Step::Main1)
        .battlefield(0, [ready(card("Nivmagus Elemental").unwrap())])
        .battlefield(1, [ready(mountain())])
        .hand(1, [card("Lightning Bolt").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let face = driver.target_player(0);
    driver.cast_at("Lightning Bolt", &[face]).unwrap();
    while !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_)) {
        driver.answer_passively().unwrap();
    }
    let me = driver.state().players()[0].id();
    while driver.decision().unwrap().player != me {
        driver.pass().unwrap();
    }
    // Seat 0 holds priority with the opponent's bolt on the stack.
    assert_eq!(driver.state().stack().len(), 1);
    assert!(driver.begin_activation("Nivmagus Elemental").is_err());
}

#[rstest]
fn r5512_synthesis_pod_reads_the_mana_value_of_the_exiled_spell(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Synthesis Pod").unwrap()),
                ready(island()),
                ready(island()),
                ready(mountain()),
            ],
        )
        .hand(0, [card("Lightning Bolt").unwrap()])
        .library(
            1,
            [
                card("Llanowar Elves").unwrap(),
                forest(),
                card("Grizzly Bears").unwrap(),
                forest(),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let life = driver.state().players()[1].life();
    let face = driver.target_player(1);
    driver.begin_cast("Lightning Bolt").unwrap();
    driver.choose_targets(&[face]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    while !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_)) {
        driver.answer_passively().unwrap();
    }
    driver.begin_activation("Synthesis Pod").unwrap();
    if matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseTargets(_)
    ) {
        let opponent = driver.target_player(1);
        driver.choose_targets(&[opponent]).unwrap();
    }
    r5512_settle(&mut driver);
    let state = driver.state();
    let opponent = state.players()[1].id();
    let names_in = |zone: Zone| -> Vec<String> {
        state
            .zone(zone)
            .iter()
            .filter_map(|id| state.object(*id).map(|object| object.card().name.clone()))
            .collect()
    };
    let library = names_in(Zone::Library(opponent));
    assert!(
        !library.contains(&"Grizzly Bears".to_owned()),
        "the card of mana value 2 left"
    );
    assert!(
        library.contains(&"Llanowar Elves".to_owned()),
        "the card of mana value 1 stays"
    );
    assert_eq!(library.len(), 3);
    assert!(names_in(Zone::Exile).contains(&"Lightning Bolt".to_owned()));
    assert_eq!(state.players()[1].life(), life, "the bolt never resolved");
}
