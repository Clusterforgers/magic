//! Goal: an ability the Ring gives its bearer (levels 2 to 4) is the Ring's on the wire, whoever the bearer is and whoever looks: its trigger, put-on-stack and delayed-trigger events carry the Ring as their origin, the object on the stack is named "The Ring" with the Ring-bearer as its source and its rules in the Ring's words, an ability the bearer prints keeps the bearer's own name, and the card the loot draws is in the log before the discard is asked.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::abilities::AbilitySource;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, Granter, HiddenEvent, ObjectView, StateView, project_events};
use mtg_testkit::cards::{cost, free_creature, island};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const OWNER: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);

/// An instant for `{U}` with "The Ring tempts you."
fn tempt(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Blue]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![Effect::RingTempts { who: Ref::You }],
            }),
    )
}

/// A creature with an attack trigger of its own: "Whenever this creature attacks, you gain 1 life."
fn gainer(name: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(CardDefinition::clone(&free_creature(name, 2, 4)).with_rules(rule))
}

/// What a fight left behind: the state ahead of every step with the events the step made, and the abilities each seat
/// saw on the stack.
struct Fight {
    driver: Driver,
    steps: Vec<(GameState, Vec<Event>)>,
    seen: BTreeMap<(PlayerId, ObjectId), ObjectView>,
}

impl Fight {
    /// Runs `act` on the driver and keeps what it did and what each seat sees of the stack afterwards.
    fn step(&mut self, act: impl FnOnce(&mut Driver) -> Vec<Event>) {
        let before = self.driver.state().clone();
        let events = act(&mut self.driver);
        self.steps.push((before, events));
        for seat in [OWNER, FOE] {
            for object in StateView::for_player(self.driver.state(), seat).objects {
                if object.zone == Zone::Stack && object.kind == ObjectKind::Ability {
                    self.seen.insert((seat, object.id), object);
                }
            }
        }
    }

    fn events(&self) -> impl Iterator<Item = &Event> {
        self.steps.iter().flat_map(|(_, events)| events)
    }

    /// The abilities on the stack the seat saw.
    fn abilities(&self, seat: PlayerId) -> Vec<&ObjectView> {
        self.seen
            .iter()
            .filter(|((viewer, _), _)| *viewer == seat)
            .map(|(_, view)| view)
            .collect()
    }
}

/// Seat 0 tempts `level` times with `bearer` ("Hero") as its only creature, then attacks with it; seat 1 blocks with
/// "Small" when `blocked`.
fn fight(level: u8, bearer: Arc<CardDefinition>, blocked: bool) -> Fight {
    let tempts = usize::from(level);
    let mut battlefield: Vec<_> = (0..tempts).map(|_| ready(island())).collect();
    battlefield.push(ready(bearer));
    let scenario = Scenario::new(2)
        .hand(0, (0..tempts).map(|n| tempt(&format!("Tempt {n}"))))
        .library(
            0,
            [free_creature("Spare", 1, 1), free_creature("Extra", 1, 1)],
        )
        .battlefield(0, battlefield)
        .battlefield(1, [ready(free_creature("Small", 1, 4))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(1, Step::Main1).unwrap();
    let mut fight = Fight {
        driver,
        steps: Vec::new(),
        seen: BTreeMap::new(),
    };
    for n in 0..tempts {
        fight.step(|driver| driver.cast(&format!("Tempt {n}")).unwrap());
        for _ in 0..12 {
            if fight.driver.state().stack().is_empty() {
                break;
            }
            fight.step(|driver| driver.pass().unwrap());
        }
    }
    assert_eq!(fight.driver.state().ring_level(OWNER), level);
    for _ in 0..60 {
        let kind = fight.driver.decision().unwrap().kind.clone();
        match kind {
            DecisionKind::DeclareAttackers(_) => {
                fight.step(|driver| driver.attack(&[("Hero", 1)]).unwrap());
            }
            DecisionKind::DeclareBlockers(_) => {
                let blocks: &[(&str, &str)] = if blocked { &[("Small", "Hero")] } else { &[] };
                fight.step(|driver| driver.block(blocks).unwrap());
            }
            DecisionKind::OrderTriggers(options) => {
                let order: Vec<usize> = (0..options.triggers.len()).collect();
                fight.step(|driver| driver.order_triggers(&order).unwrap());
            }
            DecisionKind::Discard(options) => {
                let card = options.hand[0];
                fight.step(|driver| driver.answer(Answer::Discard(vec![card])).unwrap());
            }
            DecisionKind::ChooseCards(options) => {
                let card = options.from[0];
                fight.step(|driver| {
                    driver
                        .answer(Answer::ChooseCards(CardsChoice(vec![card])))
                        .unwrap()
                });
            }
            _ if fight.driver.state().turn().step() > Step::EndCombat => break,
            _ => fight.step(|driver| driver.pass().unwrap()),
        }
    }
    fight
}

/// The Ring's trigger at `level`, as the card writer words it.
fn ring_text(level: u8) -> &'static str {
    match level {
        2 => "Whenever your Ring-bearer attacks, draw a card, then discard a card.",
        3 => {
            "Whenever your Ring-bearer becomes blocked by a creature, that creature's controller sacrifices it at end of combat."
        }
        _ => {
            "Whenever your Ring-bearer deals combat damage to a player, each opponent loses 3 life."
        }
    }
}

#[rstest]
#[case::level_two_loots(2, false, 1)]
#[case::level_three_sacrifices_the_blocker_at_end_of_combat(3, true, 2)]
#[case::level_four_drains(4, false, 1)]
fn the_ring_is_the_origin_and_the_name_of_what_it_gives_the_bearer(
    #[case] level: u8,
    #[case] blocked: bool,
    #[case] triggers: usize,
) {
    let fight = fight(level, free_creature("Hero", 2, 4), blocked);
    let hero = fight.driver.id_of("Hero").unwrap();
    // Every trigger the Ring gave, at the level and below it (the delayed one level 3 sets up included), names the Ring.
    let triggered: Vec<_> = fight
        .events()
        .filter_map(|event| match event {
            Event::AbilityTriggered(trigger) if trigger.origin.ring().is_some() => Some(trigger),
            _ => None,
        })
        .collect();
    for trigger in &triggered {
        let (player, from) = trigger.origin.ring().unwrap();
        assert_eq!((player, trigger.source.id), (OWNER, hero));
        assert!((2..=level).contains(&from));
    }
    let here = |origin: AbilitySource| origin.ring() == Some((OWNER, level));
    let at_level: Vec<_> = triggered.iter().filter(|t| here(t.origin)).collect();
    assert_eq!(at_level.len(), triggers);
    assert_eq!(
        at_level.iter().filter(|t| t.delayed.is_some()).count(),
        triggers - 1
    );
    let put = fight
        .events()
        .filter(|event| matches!(event, Event::AbilityPutOnStack(put) if here(put.origin)))
        .count();
    assert_eq!(put, triggers);
    // Every seat reads the same name, source and words.
    for seat in [OWNER, FOE] {
        let views = fight.abilities(seat);
        assert_eq!(views.len(), triggered.len(), "{seat:?}");
        for view in &views {
            assert_eq!(view.name, "The Ring");
            assert_eq!(view.source, Some(hero));
            assert_eq!(view.controller, OWNER);
        }
        let at_level: Vec<_> = views
            .iter()
            .filter(|view| view.granter == Some(Granter::Ring { level }))
            .collect();
        assert_eq!(at_level.len(), triggers, "{seat:?}");
        assert_eq!(at_level[0].rules_text.as_deref(), Some(ring_text(level)));
        // The delayed trigger a level sets up does what it sets up, not what the level says.
        for later in &at_level[1..] {
            assert_ne!(later.rules_text.as_deref(), Some(ring_text(level)));
        }
    }
}

#[test]
fn an_ability_the_bearer_prints_keeps_the_bearers_name() {
    let fight = fight(2, gainer("Hero"), false);
    let abilities = fight.abilities(FOE);
    let names: Vec<&str> = abilities.iter().map(|view| view.name.as_str()).collect();
    assert_eq!(abilities.len(), 2, "{names:?}");
    assert!(names.contains(&"The Ring"));
    let own = abilities.iter().find(|view| view.name == "Hero").unwrap();
    assert_eq!(own.granter, None);
    assert!(
        own.rules_text
            .as_deref()
            .unwrap()
            .starts_with("Whenever Hero attacks")
    );
}

#[test]
fn the_card_the_loot_draws_is_in_the_log_before_the_discard_is_asked() {
    let fight = fight(2, free_creature("Hero", 2, 4), false);
    // The step in which the Ring's trigger resolves: it ends with the question about the discard.
    let (before, events) = fight
        .steps
        .iter()
        .find(|(_, events)| {
            events
                .iter()
                .any(|event| matches!(event, Event::CardDrawn { player, .. } if *player == OWNER))
        })
        .expect("the loot draws");
    let began = events
        .iter()
        .position(|event| matches!(event, Event::ResolutionBegan { .. }))
        .unwrap();
    let drew = events
        .iter()
        .position(|event| matches!(event, Event::CardDrawn { .. }))
        .unwrap();
    assert!(began < drew);
    assert!(
        !events.iter().any(|event| matches!(
            event,
            Event::ZoneChanged {
                to: Zone::Graveyard(_),
                ..
            }
        )),
        "the discard comes after the question, in a later step"
    );
    let batches = project_events(before, events, &[OWNER, FOE], 0).unwrap();
    // The owner reads which card was drawn, the other seat that one was.
    assert!(batches[0].events.iter().any(|view| matches!(
        view,
        EventView::Public(Event::CardDrawn { player, .. }) if *player == OWNER
    )));
    assert!(batches[1].events.iter().any(|view| matches!(
        view,
        EventView::Hidden(HiddenEvent::CardDrawn { player }) if *player == OWNER
    )));
}
