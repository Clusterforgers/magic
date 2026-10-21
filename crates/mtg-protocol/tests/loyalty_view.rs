//! Goal: the loyalty of a planeswalker is public (its loyalty counters reach every seat), each loyalty ability of one on the battlefield is listed with its cost under the address the holder of priority is offered, damage to a planeswalker shows as loyalty lost and no damage marked, a planeswalker that is not on the battlefield lists no abilities, and the field is left out of the JSON while it says nothing so a game without planeswalkers keeps its wire form.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::LoyaltyCost;
use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::DecisionKind;
use mtg_engine::abilities::AbilityRef;
use mtg_engine::zones::Zone;
use mtg_protocol::{ObjectView, StateView};
use mtg_testkit::cards::bear;
use mtg_testkit::loyalty_cards::dawn_walker;
use mtg_testkit::{Driver, Scenario, ready, with_counters};
use rstest::rstest;

/// Seat 0 controls Dawn Walker (+1, 0 and -3) with three loyalty counters and a Bear; seat 1 holds a second Dawn Walker.
fn table(seats: usize) -> Driver {
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                with_counters(ready(dawn_walker().unwrap()), CounterKind::Loyalty, 3),
                ready(bear()),
            ],
        )
        .hand(1, [dawn_walker().unwrap()]);
    for seat in 2..seats {
        scenario = scenario.hand(seat, [bear()]);
    }
    Driver::from_scenario(scenario).unwrap()
}

fn view(driver: &Driver, viewer: usize) -> StateView {
    StateView::for_player(driver.state(), PlayerId::from_index(viewer))
}

fn named(view: &StateView, name: &str, zone: Zone) -> Option<ObjectView> {
    view.objects
        .iter()
        .find(|o| o.name == name && o.zone == zone)
        .cloned()
}

#[rstest]
fn the_loyalty_and_the_abilities_of_a_planeswalker_reach_every_seat_alike(
    #[values(2, 3, 4)] seats: usize,
) {
    let driver = table(seats);
    let id = driver.id_of("Dawn Walker").unwrap();
    for viewer in 0..seats {
        let walker = named(&view(&driver, viewer), "Dawn Walker", Zone::Battlefield).unwrap();
        assert_eq!(
            walker.counters,
            [(CounterKind::Loyalty, 3)],
            "seat {viewer}"
        );
        let listed: Vec<(AbilityRef, LoyaltyCost)> = walker
            .loyalty_abilities
            .iter()
            .map(|a| (a.ability, a.cost))
            .collect();
        assert_eq!(
            listed,
            [
                (AbilityRef::printed(id, 0), LoyaltyCost::Plus(1)),
                (AbilityRef::printed(id, 1), LoyaltyCost::Zero),
                (AbilityRef::printed(id, 2), LoyaltyCost::Minus(3)),
            ],
            "seat {viewer}"
        );
        assert_eq!(walker.permanent.unwrap().damage, 0);
    }
}

#[test]
fn the_abilities_a_player_is_offered_are_among_those_the_view_lists() {
    let driver = table(2);
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    let walker = named(&view(&driver, 0), "Dawn Walker", Zone::Battlefield).unwrap();
    let listed: Vec<AbilityRef> = walker.loyalty_abilities.iter().map(|a| a.ability).collect();
    assert!(!options.abilities.is_empty());
    for offered in &options.abilities {
        assert!(
            listed.contains(offered),
            "{offered:?} is offered but not listed"
        );
    }
}

#[test]
fn a_planeswalker_off_the_battlefield_and_every_other_permanent_list_none() {
    let driver = table(2);
    let seen = view(&driver, 1);
    let in_hand = named(&seen, "Dawn Walker", Zone::Hand(PlayerId(1))).unwrap();
    assert!(in_hand.loyalty_abilities.is_empty() && in_hand.counters.is_empty());
    let bear = named(&seen, "Grizzly Bears", Zone::Battlefield).unwrap();
    assert!(bear.loyalty_abilities.is_empty());
    // The field is left out while it says nothing, and the abilities of the walker are not.
    assert!(
        serde_json::to_value(&bear)
            .unwrap()
            .get("loyalty_abilities")
            .is_none()
    );
    let walker = named(&seen, "Dawn Walker", Zone::Battlefield).unwrap();
    assert_eq!(
        serde_json::to_value(&walker).unwrap()["loyalty_abilities"][2],
        serde_json::json!({"ability": {"object": walker.id, "ability": 2}, "cost": {"Minus": 3}})
    );
}

#[test]
fn damage_to_a_planeswalker_is_loyalty_lost_and_marks_nothing() {
    let driver = table(2);
    let walker = driver.id_of("Dawn Walker").unwrap();
    let bear = driver.id_of("Grizzly Bears").unwrap();
    let mut state = driver.state().clone();
    let damage = mtg_engine::Event::DamageDealt {
        source: bear,
        target: mtg_engine::combat::DamageTarget::Object(walker),
        amount: 2,
        deathtouch: false,
    };
    mtg_engine::apply_event(&mut state, &damage).unwrap();
    for viewer in 0..2 {
        let seen = StateView::for_player(&state, PlayerId::from_index(viewer));
        let shown = named(&seen, "Dawn Walker", Zone::Battlefield).unwrap();
        assert_eq!(shown.counters, [(CounterKind::Loyalty, 1)]);
        assert_eq!(shown.permanent.unwrap().damage, 0);
    }
}
