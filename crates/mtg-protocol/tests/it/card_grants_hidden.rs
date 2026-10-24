//! Goal: a keyword a static ability gives to the cards of a hand changes nothing another seat can see of those cards, at two to four seats and to a spectator. The owner reads the grant on a card of its hand, no other seat lists the card or reads the keyword anywhere in its view, a card of a library gets none, and the events that follow (a card cycled for the grant) name no hand card and no keyword that was given. A grant to the cards of a public zone (a graveyard, exile, the stack) is read by every seat.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState, apply_event};
use mtg_protocol::{ObjectView, StateView, project_events};
use mtg_testkit::cards::{bear, creature, forest, island, mountain};
use mtg_testkit::corpus::card as corpus_card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::hidden_info::{assert_clean, hidden_across};

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Every seat and a spectator after them.
fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..=seats).map(seat).collect()
}

/// The first card called `name` in `zone`.
fn card_in(state: &GameState, zone: Zone, name: &str) -> ObjectId {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap()
}

/// Seat 0 has Rhet-Tomb Mystic and two lands in play and a Grizzly Bears in hand; every seat has a library of its own,
/// and the other seats hold a Foe Bear of their own.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(corpus_card("Rhet-Tomb Mystic").unwrap()),
                ready(island()),
                ready(mountain()),
            ],
        )
        .hand(0, [bear()])
        .library(
            0,
            (0..8).map(|n| creature(&format!("Library Bear {n}"), 1, &[], 1, 1)),
        );
    for other in 1..seats {
        scenario = scenario
            .hand(
                other,
                [creature(&format!("Foe Bear {other}"), 2, &[], 2, 2)],
            )
            .library(other, vec![forest(); 8]);
    }
    scenario
}

fn grants_cycling(view: &ObjectView) -> bool {
    view.keywords.iter().any(|keyword| keyword == "Cycling")
}

#[rstest]
fn the_owner_reads_a_grant_on_a_hand_card_and_no_other_seat_reads_it(
    #[values(2, 3, 4)] seats: usize,
) {
    let driver = Driver::from_scenario(table(seats)).unwrap();
    let state = driver.state();
    let mine = card_in(state, Zone::Hand(seat(0)), "Grizzly Bears");
    let theirs = card_in(state, Zone::Hand(seat(1)), "Foe Bear 1");
    let owner = ObjectView::of(state, seat(0), mine).unwrap();
    assert!(grants_cycling(&owner), "the owner reads the grant");
    assert!(
        ObjectView::of(state, seat(0), theirs).is_none(),
        "the owner does not list a card of another hand"
    );
    for viewer in viewers(seats).into_iter().skip(1) {
        assert!(
            ObjectView::of(state, viewer, mine).is_none(),
            "{viewer:?} does not list the card"
        );
        let text = serde_json::to_string(&StateView::for_player(state, viewer)).unwrap();
        assert!(
            !text.contains("Cycling"),
            "{viewer:?} reads no keyword that was given"
        );
    }
    let own = ObjectView::of(state, seat(1), theirs).unwrap();
    assert!(
        !grants_cycling(&own),
        "a card of another hand is not covered by a grant that says \"your\""
    );
}

#[rstest]
fn a_library_card_is_given_nothing(#[values(2, 3, 4)] seats: usize) {
    let driver = Driver::from_scenario(table(seats)).unwrap();
    let state = driver.state();
    let top = *state.zone(Zone::Library(seat(0))).last().unwrap();
    assert!(
        !mtg_engine::keywords::keywords_of(state, top)
            .iter()
            .any(|keyword| matches!(keyword, mtg_cards::keyword::Keyword::Cycling(_)))
    );
}

#[rstest]
fn the_events_of_a_cycling_for_a_grant_name_no_hand_card_and_no_given_keyword(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let before = driver.state().clone();
    let logged = driver.engine().events().len();
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    let ability = options
        .abilities
        .iter()
        .find(|ability| {
            driver.state().object(ability.object).unwrap().card().name == "Grizzly Bears"
        })
        .copied()
        .unwrap();
    driver
        .answer(Answer::Priority(PriorityAction::Activate(ability)))
        .unwrap();
    driver.pay_tapping(&["Island", "Mountain"], 0).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let after = driver.state().clone();
    let events = driver.engine().events()[logged..].to_vec();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Cycled { .. }))
    );
    let all = viewers(seats);
    let batches = project_events(&before, &events, &all, u64::try_from(logged).unwrap()).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        let hidden = hidden_across(&before, &after, *viewer);
        assert_clean(batch, &hidden);
        if *viewer == seat(0) {
            continue;
        }
        // Activating the cycling is public, as it is for a printed one: the ability goes on the stack with a snapshot of
        // its source. No other event, no table and no object names the keyword.
        let others: Vec<String> = batch
            .events
            .iter()
            .map(|view| serde_json::to_string(view).unwrap())
            .filter(|text| {
                !text.contains("AbilityPutOnStack") && !text.contains("AbilityActivated")
            })
            .collect();
        let text = serde_json::to_string(&(&others, &batch.table, &batch.objects)).unwrap();
        assert!(
            !text.contains("Cycling"),
            "{viewer:?} reads a keyword that was given to a card of a hand"
        );
        assert!(
            batch.events.iter().any(|view| serde_json::to_string(view)
                .unwrap()
                .contains("AbilityPutOnStack")),
            "{viewer:?} sees the ability go on the stack"
        );
    }
}

/// A creature that gives Flying to the creature cards in a graveyard and in exile and to its controller's spells.
fn public_lord() -> Arc<CardDefinition> {
    let statics = [
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"In":"Graveyard"}]},"mods":[{"AddKeyword":"Flying"}]}}"#,
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"In":"Exile"}]},"mods":[{"AddKeyword":"Flying"}]}}"#,
        r#"{"Static":{"affected":{"Each":["Spell",{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Flying"}]}}"#,
    ];
    let mut card = CardDefinition::new("Public Lord")
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1);
    for text in statics {
        card = card.with_rules(serde_json::from_str(text).unwrap());
    }
    Arc::new(card)
}

#[rstest]
fn a_grant_to_the_cards_of_a_public_zone_is_read_by_every_seat(#[values(2, 3, 4)] seats: usize) {
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            [ready(public_lord()), ready(mountain()), ready(mountain())],
        )
        .hand(0, [creature("Two", 2, &[], 2, 2), bear()])
        .graveyard(0, [creature("Buried", 1, &[], 1, 1)]);
    for any in 0..seats {
        scenario = scenario.library(any, vec![forest(); 8]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Two").unwrap();
    let mut state = driver.state().clone();
    let exiled = card_in(&state, Zone::Hand(seat(0)), "Grizzly Bears");
    apply_event(
        &mut state,
        &Event::ZoneChanged {
            object: exiled,
            from: Zone::Hand(seat(0)),
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        },
    )
    .unwrap();
    let buried = card_in(&state, Zone::Graveyard(seat(0)), "Buried");
    let spell = card_in(&state, Zone::Stack, "Two");
    for viewer in viewers(seats) {
        for (id, place) in [(buried, "graveyard"), (exiled, "exile"), (spell, "stack")] {
            let view = ObjectView::of(&state, viewer, id).unwrap();
            assert!(
                view.keywords.iter().any(|keyword| keyword == "Flying"),
                "{viewer:?} reads the grant on the card in the {place}"
            );
        }
    }
}
