//! Goal: `ObjectView.modifiers` lists everything that changes an object now with its source, worded, identical sources of one name added together with their count (anthems, counters), an Aura's and an emblem's grants, a pump with its duration, a control change and a copy effect, the same to every seat, and a face-down source named only to the seat that may look at it, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::GameState;
use mtg_engine::copies::BecameCopy;
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::events::Event;
use mtg_engine::fold::apply_event;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::zones::Zone;
use mtg_protocol::{ModifierView, ObjectView, StateView};
use mtg_testkit::cards::{bear, plains};
use mtg_testkit::{Driver, Scenario, attached_to, corpus, ready, with_counters};
use rstest::rstest;

fn view_of(state: &GameState, viewer: usize, id: ObjectId) -> ObjectView {
    StateView::for_player(state, PlayerId::from_index(viewer))
        .objects
        .into_iter()
        .find(|o| o.id == id)
        .unwrap()
}

/// The modifiers as (effect, source, count, duration), the ids left aside.
fn lines(view: &ObjectView) -> Vec<(String, String, u32, Option<String>)> {
    view.modifiers
        .iter()
        .map(|m| {
            (
                m.effect.clone(),
                m.source.clone(),
                m.count,
                m.duration.clone(),
            )
        })
        .collect()
}

fn line(
    effect: &str,
    source: &str,
    count: u32,
    duration: Option<&str>,
) -> (String, String, u32, Option<String>) {
    (
        effect.to_owned(),
        source.to_owned(),
        count,
        duration.map(str::to_owned),
    )
}

/// An emblem whose static ability gives the creatures of its owner +1/+0 and menace, as a Delves boss passive does.
fn passive() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":0}},{"AddKeyword":"Menace"}]}}"#,
    )
    .unwrap();
    Arc::new(emblem_definition("Boss Passive", &[ability]))
}

/// A table of `seats` where seat 0 has a Bear (with `counters`) and then `others`, and `emblem` when given.
fn table(
    seats: usize,
    counters: u32,
    others: Vec<mtg_engine::setup::PresetPermanent>,
    emblem: bool,
) -> Driver {
    let first = if counters == 0 {
        ready(bear())
    } else {
        with_counters(ready(bear()), CounterKind::PlusOnePlusOne, counters)
    };
    let mut permanents = vec![first];
    permanents.extend(others);
    let mut scenario = Scenario::new(seats).battlefield(0, permanents);
    if emblem {
        scenario = scenario.emblems(0, [passive()]);
    }
    Driver::from_scenario(scenario).unwrap()
}

fn anthem() -> mtg_engine::setup::PresetPermanent {
    ready(corpus::card("Glorious Anthem").unwrap())
}

#[rstest]
#[case::nothing(0, vec![], false, vec![])]
#[case::one_anthem(0, vec![anthem()], false, vec![line("+1/+1", "Glorious Anthem", 1, None)])]
#[case::anthems_add_up(0, vec![anthem(), anthem()], false, vec![line("+2/+2", "Glorious Anthem", 2, None)])]
#[case::counters_add_up(3, vec![], false, vec![line("+3/+3", "+1/+1 counter", 3, None)])]
#[case::aura(0, vec![attached_to(ready(corpus::card("Rancor").unwrap()), 0, 0)], false, vec![line("+2/+0, Trample", "Rancor", 1, None)])]
#[case::emblem(0, vec![], true, vec![line("+1/+0, Menace", "Boss Passive", 1, None)])]
#[case::all_at_once(2, vec![anthem(), anthem()], true, vec![
    line("+2/+2", "Glorious Anthem", 2, None),
    line("+1/+0, Menace", "Boss Passive", 1, None),
    line("+2/+2", "+1/+1 counter", 2, None),
])]
fn a_permanent_lists_its_modifiers_by_source(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] others: Vec<mtg_engine::setup::PresetPermanent>,
    #[case] emblem: bool,
    #[case] expected: Vec<(String, String, u32, Option<String>)>,
) {
    let driver = table(seats, counters, others, emblem);
    let bear = driver.id_of("Grizzly Bears").unwrap();
    for viewer in 0..seats {
        let mut found = lines(&view_of(driver.state(), viewer, bear));
        let mut wanted = expected.clone();
        found.sort();
        wanted.sort();
        assert_eq!(found, wanted, "seat {viewer}");
    }
}

#[test]
fn identical_sources_name_each_object_once() {
    let driver = table(2, 0, vec![anthem(), anthem()], false);
    let bear = driver.id_of("Grizzly Bears").unwrap();
    let view = view_of(driver.state(), 1, bear);
    let anthems: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == "Glorious Anthem")
        .collect();
    let mut sources = view.modifiers[0].sources.clone();
    sources.sort();
    assert_eq!(sources, anthems);
}

#[rstest]
#[case::pump(
    Modification::ModifyPowerToughness { power: 3, toughness: 3 },
    Duration::UntilEndOfTurn,
    line("+3/+3", "Glorious Anthem", 1, Some("until end of turn")),
)]
#[case::control(
    Modification::SetController(PlayerId(1)),
    Duration::WhileSourceOnBattlefield,
    line("Control changed", "Glorious Anthem", 1, None)
)]
#[case::cant_block(
    Modification::CantBlock,
    Duration::UntilYourNextTurn,
    line("Can't block", "Glorious Anthem", 1, Some("until your next turn"))
)]
fn an_effect_names_its_source_and_how_long_it_lasts(
    #[values(2, 3)] seats: usize,
    #[case] modification: Modification,
    #[case] duration: Duration,
    #[case] expected: (String, String, u32, Option<String>),
) {
    let mut driver = table(seats, 0, vec![anthem()], false);
    let bear = driver.id_of("Grizzly Bears").unwrap();
    let source = driver.id_of("Glorious Anthem").unwrap();
    let on = driver.on(bear);
    driver
        .add_effect_from(source, on, modification, duration)
        .unwrap();
    for viewer in 0..seats {
        let found = lines(&view_of(driver.state(), viewer, bear));
        assert!(found.contains(&expected), "seat {viewer}: {found:?}");
    }
}

#[rstest]
fn a_copy_effect_names_what_it_copies(#[values(2, 3)] seats: usize) {
    let angel = corpus::card("Serra Angel").unwrap();
    let driver = table(seats, 0, vec![ready(Arc::clone(&angel))], false);
    let bear = driver.id_of("Grizzly Bears").unwrap();
    let original = driver
        .state()
        .object(driver.id_of("Serra Angel").unwrap())
        .unwrap()
        .reference();
    let mut state = driver.state().clone();
    let copy = Event::BecameCopy(BecameCopy {
        object: bear,
        of: original,
        definition: angel,
        until_end_of_turn: false,
        until_next_turn_of: None,
        attached_by: None,
        r6104_while_tapped: None,
        r707_static: None,
        r115_until_face_down: false,
    });
    apply_event(&mut state, &copy).unwrap();
    for viewer in 0..seats {
        let found = view_of(&state, viewer, bear);
        assert_eq!(
            found.modifiers[0].effect, "Copy of Serra Angel",
            "seat {viewer}"
        );
        assert_eq!(found.modifiers[0].sources, vec![original.id]);
    }
}

/// Seat 0 manifests the top of its library (a Serra Angel) with Mastery of the Unseen, and the face-down card pumps a
/// Bear until end of turn: the Bear's view for `viewer`.
fn pumped_by_a_face_down_card(seats: usize) -> (Driver, ObjectId) {
    let mut battlefield = vec![
        ready(corpus::card("Mastery of the Unseen").unwrap()),
        ready(bear()),
    ];
    battlefield.extend((0..9).map(|_| ready(plains())));
    let mut scenario = Scenario::new(seats).battlefield(0, battlefield);
    for seat in 0..seats {
        let mut library = vec![corpus::card("Serra Angel").unwrap()];
        library.extend((0..6).map(|_| bear()));
        if seat != 0 {
            library[0] = bear();
        }
        scenario = scenario.library(seat, library);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Mastery of the Unseen").unwrap();
    driver
        .answer(mtg_engine::decision::Answer::PayMana(
            mtg_engine::decision::PayChoice::Auto,
        ))
        .unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let hidden = driver
        .state()
        .objects()
        .find(|object| object.face_down().is_some())
        .unwrap()
        .id();
    let bear = driver.id_of("Grizzly Bears").unwrap();
    let on = driver.on(bear);
    driver
        .add_effect_from(
            hidden,
            on,
            Modification::ModifyPowerToughness {
                power: 1,
                toughness: 1,
            },
            Duration::UntilEndOfTurn,
        )
        .unwrap();
    (driver, bear)
}

#[rstest]
fn a_face_down_source_is_named_only_to_the_seat_that_may_look_at_it(
    #[values(2, 3, 4)] seats: usize,
) {
    let (driver, bear) = pumped_by_a_face_down_card(seats);
    for viewer in 0..seats {
        let view = view_of(driver.state(), viewer, bear);
        let pump: &ModifierView = view
            .modifiers
            .iter()
            .find(|m| m.duration.is_some())
            .unwrap();
        let expected = if viewer == 0 {
            "Serra Angel"
        } else {
            "A face-down card"
        };
        assert_eq!(pump.source, expected, "seat {viewer}");
        assert_eq!(pump.effect, "+1/+1");
        if viewer != 0 {
            let json = serde_json::to_string(&view).unwrap();
            assert!(
                !json.contains("Serra Angel"),
                "seat {viewer} learns the face-down card: {json}"
            );
        }
    }
}

#[test]
fn effects_of_no_object_that_last_alike_are_one_line_with_no_duration_for_good() {
    let mut driver = table(2, 0, vec![], false);
    let bear = driver.id_of("Grizzly Bears").unwrap();
    for modification in [
        Modification::AddKeyword(mtg_cards::keyword::Keyword::Flying),
        Modification::ModifyPowerToughness {
            power: 1,
            toughness: 1,
        },
    ] {
        let on = driver.on(bear);
        driver
            .add_effect(on, modification, Duration::Forever)
            .unwrap();
    }
    assert_eq!(
        lines(&view_of(driver.state(), 1, bear)),
        vec![line("+1/+1, Flying", "An effect", 1, None)]
    );
}

#[test]
fn the_field_is_left_out_of_the_json_while_empty() {
    let driver = table(2, 0, vec![], false);
    let bear = driver.id_of("Grizzly Bears").unwrap();
    let json = serde_json::to_value(view_of(driver.state(), 0, bear)).unwrap();
    assert!(json.get("modifiers").is_none());
}
