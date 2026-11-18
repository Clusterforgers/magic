//! Goal: `ObjectView.trigger` tells copies of one trigger apart on the stack for every seat: each of Mr. House's triggers names the player who rolled and its own die's result, an activated ability and a spell leave it out, and the JSON leaves it out where it is empty.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_protocol::{StateView, TriggerView};
use mtg_testkit::cards::land;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const HOUSE: &str = "Mr. House, President and CEO";

/// An artifact Treasure: "{T}, Sacrifice this artifact: Add one mana of any color."
fn treasure() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Treasure")
            .with_type(CardType::Artifact)
            .with_subtype("Treasure")
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

#[rstest]
fn each_copy_of_a_trigger_shows_what_it_triggered_on(
    #[values(2, 3)] seats: usize,
    #[values(1, 2, 3, 4)] seed: u64,
) {
    let mut mine = vec![ready(card(HOUSE).unwrap()), ready(land(Color::Red))];
    mine.extend((0..3).map(|_| ready(treasure())));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .seed(seed)
            .at(2, 0, Step::Main1)
            .battlefield(0, mine),
    )
    .unwrap();
    driver.begin_activation(HOUSE).unwrap();
    let mut dice = Vec::new();
    let take = |dice: &mut Vec<u32>, events: Vec<Event>| {
        dice.extend(events.iter().filter_map(|event| match event {
            Event::DieRolled { result, .. } => Some(*result),
            _ => None,
        }));
    };
    take(
        &mut dice,
        driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap(),
    );
    // The activation is on the stack: an activated ability says nothing of a trigger.
    for viewer in 0..seats {
        let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
        let top = view
            .objects
            .iter()
            .find(|o| o.zone == mtg_engine::zones::Zone::Stack);
        assert_eq!(top.map(|o| o.trigger.clone()), Some(None));
    }
    // Resolve it: the triggers of the dice of 4 or higher go on the stack together.
    for _ in 0..50 {
        let state = driver.state();
        let triggered = state.stack().iter().any(|id| {
            state
                .object(*id)
                .and_then(|object| object.ability())
                .is_some_and(|ability| ability.cause().is_some())
        });
        let settled = state.stack().is_empty() && state.pending_triggers().is_empty();
        if triggered || (!dice.is_empty() && settled) {
            break;
        }
        take(&mut dice, driver.answer_passively().unwrap());
    }
    assert_eq!(driver.state().turn().step(), Step::Main1);
    assert_eq!(dice.len(), 4, "a die and one for each Treasure");
    let mut wanted: Vec<u32> = dice.iter().copied().filter(|die| *die >= 4).collect();
    wanted.sort_unstable();
    for viewer in 0..seats {
        let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
        let mut shown: Vec<u32> = view
            .objects
            .iter()
            .filter(|o| o.zone == mtg_engine::zones::Zone::Stack && o.name == HOUSE)
            .map(|o| {
                let trigger = o
                    .trigger
                    .clone()
                    .expect("a triggered ability says what it triggered on");
                assert_eq!(trigger.players, vec![PlayerId(0)]);
                assert_eq!(trigger.objects, Vec::<String>::new());
                trigger.amount.unwrap()
            })
            .collect();
        shown.sort_unstable();
        assert_eq!(shown, wanted, "viewer {viewer}, dice {dice:?}");
    }
}

#[test]
fn an_empty_trigger_view_is_left_out_of_the_json() {
    let view = TriggerView {
        players: vec![PlayerId(1)],
        objects: Vec::new(),
        amount: None,
    };
    assert_eq!(serde_json::to_string(&view).unwrap(), r#"{"players":[1]}"#);
}
