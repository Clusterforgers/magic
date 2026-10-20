//! Goal: an activated ability is offered, a triggered ability triggers, a state trigger triggers and a static ability applies exactly while its source stands in the zone it names, for every kind and every zone (the battlefield, a hand, a library, a graveyard, exile, the command zone and the stack), and an ability that works from the battlefield does not work from anywhere else.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use crate::abilities::PutOnStack;
use crate::characteristics::characteristics;
use crate::choose::Choices;
use crate::events::{Event, ResolutionOutcome, ZoneCause};
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::priority::other_abilities;
use crate::state::GameState;
use crate::triggers::{detect, state_triggers};
use crate::turn::Step;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

/// The zone a card stands in, and the name the data gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Battlefield,
    Hand,
    Library,
    Graveyard,
    Exile,
    Command,
    Stack,
}

impl Place {
    /// The name of the zone in the card data.
    const fn data(self) -> &'static str {
        match self {
            Place::Battlefield => "Battlefield",
            Place::Hand => "Hand",
            Place::Library => "Library",
            Place::Graveyard => "Graveyard",
            Place::Exile => "Exile",
            Place::Command => "Command",
            Place::Stack => "Stack",
        }
    }

    /// The zone, for the seat that owns the card.
    const fn zone(self) -> Zone {
        match self {
            Place::Battlefield => Zone::Battlefield,
            Place::Hand => Zone::Hand(ME),
            Place::Library => Zone::Library(ME),
            Place::Graveyard => Zone::Graveyard(ME),
            Place::Exile => Zone::Exile,
            Place::Command => Zone::Command(ME),
            Place::Stack => Zone::Stack,
        }
    }

    /// Whether the zone is one where a player activates the abilities of the cards they own.
    const fn activates(self) -> bool {
        matches!(
            self,
            Place::Battlefield | Place::Hand | Place::Graveyard | Place::Exile | Place::Command
        )
    }
}

/// A 1/1 creature named "Subject" with this ability.
fn subject(ability: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(ability).expect("the ability is data");
    Arc::new(
        CardDefinition::new("Subject")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability),
    )
}

/// A game at the first main step of turn 1 with a 2/2 "Bear" for seat 0 and the subject in `place`.
fn game(card: Arc<CardDefinition>, place: Place) -> (GameState, ObjectId) {
    let mut hands = vec![vec![], vec![]];
    let mut battlefields = vec![vec![creature("Bear", 2, 2)], vec![]];
    if place == Place::Battlefield {
        battlefields[0].push(card);
    } else {
        hands[0].push(card);
    }
    let mut game = state(hands, battlefields);
    let id = game
        .objects()
        .find(|o| o.card().name == "Subject")
        .map(crate::object::GameObject::id)
        .expect("the subject is in the game");
    if !matches!(place, Place::Battlefield | Place::Hand) {
        let moved = Event::ZoneChanged {
            object: id,
            from: Zone::Hand(ME),
            to: place.zone(),
            cause: ZoneCause::Moved,
        };
        apply_event(&mut game, &moved).expect("the card moves");
    }
    (game, id)
}

fn activated(from: Place) -> String {
    format!(
        r#"{{"Activated":{{"cost":[],"from":"{}","effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#,
        from.data()
    )
}

fn triggered(from: Place, when: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{when},"from":"{}","effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#,
        from.data()
    )
}

const UPKEEP: &str = r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#;
const MY_TURN: &str = r#"{"State":{"condition":"YourTurn"}}"#;

#[rstest]
fn an_activated_ability_is_offered_from_the_zone_it_names_to_the_owner_alone(
    #[values(
        Place::Battlefield,
        Place::Hand,
        Place::Library,
        Place::Graveyard,
        Place::Exile,
        Place::Command,
        Place::Stack
    )]
    from: Place,
    #[values(
        Place::Battlefield,
        Place::Hand,
        Place::Library,
        Place::Graveyard,
        Place::Exile,
        Place::Command,
        Place::Stack
    )]
    place: Place,
) {
    let (game, id) = game(subject(&activated(from)), place);
    let offered = |seat: PlayerId| {
        other_abilities(&game, seat)
            .iter()
            .any(|address| address.object == id)
    };
    assert_eq!(offered(ME), from == place && place.activates());
    assert!(!offered(PlayerId(1)), "only its owner activates it");
}

#[rstest]
fn a_triggered_ability_triggers_from_the_zone_it_names(
    #[values(
        Place::Battlefield,
        Place::Hand,
        Place::Library,
        Place::Graveyard,
        Place::Exile,
        Place::Command,
        Place::Stack
    )]
    from: Place,
    #[values(
        Place::Battlefield,
        Place::Hand,
        Place::Library,
        Place::Graveyard,
        Place::Exile,
        Place::Command,
        Place::Stack
    )]
    place: Place,
) {
    let (before, id) = game(subject(&triggered(from, UPKEEP)), place);
    let mut after = before.clone();
    let events = [Event::StepBegan { step: Step::Upkeep }];
    apply_event(&mut after, &events[0]).unwrap();
    let found = detect(&before, &events, &after);
    let own: Vec<_> = found.iter().filter(|t| t.source.id == id).collect();
    assert_eq!(own.len(), usize::from(from == place), "{found:?}");
    if let Some(trigger) = own.first() {
        assert_eq!(trigger.controller, ME);
        assert_eq!(trigger.snapshot.zone, place.zone());
        assert_eq!(trigger.ability, 0);
    }
}

#[rstest]
fn a_state_trigger_triggers_from_the_zone_it_names(
    #[values(
        Place::Battlefield,
        Place::Hand,
        Place::Library,
        Place::Graveyard,
        Place::Exile,
        Place::Command,
        Place::Stack
    )]
    from: Place,
    #[values(
        Place::Battlefield,
        Place::Hand,
        Place::Library,
        Place::Graveyard,
        Place::Exile,
        Place::Command,
        Place::Stack
    )]
    place: Place,
) {
    let (game, id) = game(subject(&triggered(from, MY_TURN)), place);
    let found = state_triggers(&game);
    let own: Vec<_> = found.iter().filter(|t| t.source.id == id).collect();
    assert_eq!(own.len(), usize::from(from == place), "{found:?}");
    if let Some(trigger) = own.first() {
        assert_eq!(trigger.controller, ME);
        assert!(trigger.cause.subjects.is_empty(), "no event is behind it");
    }
}

#[rstest]
fn a_static_ability_applies_from_the_zone_it_names(
    #[values(
        Place::Battlefield,
        Place::Hand,
        Place::Library,
        Place::Graveyard,
        Place::Exile,
        Place::Command,
        Place::Stack
    )]
    from: Place,
    #[values(
        Place::Battlefield,
        Place::Hand,
        Place::Library,
        Place::Graveyard,
        Place::Exile,
        Place::Command,
        Place::Stack
    )]
    place: Place,
) {
    let ability = format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"You"}},"Other"]}},"mods":[{{"ModifyPT":{{"power":1,"toughness":0}}}}],"from":"{}"}}}}"#,
        from.data()
    );
    let (game, _) = game(subject(&ability), place);
    let bear = game
        .objects()
        .find(|o| o.card().name == "Bear")
        .map(crate::object::GameObject::id)
        .unwrap();
    let power = characteristics(&game, bear).unwrap().power;
    assert_eq!(power, Some(if from == place { 3 } else { 2 }));
}

#[rstest]
fn a_state_trigger_does_not_watch_events_and_an_event_trigger_does_not_watch_states(
    #[values(Place::Battlefield, Place::Graveyard)] place: Place,
) {
    let (before, id) = game(subject(&triggered(place, MY_TURN)), place);
    let mut after = before.clone();
    let events = [Event::StepBegan { step: Step::Upkeep }];
    apply_event(&mut after, &events[0]).unwrap();
    assert!(detect(&before, &events, &after).is_empty());
    let (game, _) = game(subject(&triggered(place, UPKEEP)), place);
    assert!(state_triggers(&game).iter().all(|t| t.source.id != id));
}

#[rstest]
#[case::the_state_holds(true, 1)]
#[case::the_state_fails(false, 0)]
fn a_state_trigger_waits_for_its_state(#[case] my_turn: bool, #[case] expected: usize) {
    let (mut game, _) = game(
        subject(&triggered(Place::Battlefield, MY_TURN)),
        Place::Battlefield,
    );
    if !my_turn {
        // Turn 2 is the other seat's.
        let events = [
            Event::TurnBegan {
                number: 2,
                active: PlayerId(1),
            },
            Event::StepBegan { step: Step::Main1 },
        ];
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
    }
    assert_eq!(state_triggers(&game).len(), expected);
}

#[test]
fn a_state_trigger_does_not_trigger_again_while_an_instance_waits_for_the_stack() {
    let (mut game, id) = game(
        subject(&triggered(Place::Graveyard, MY_TURN)),
        Place::Graveyard,
    );
    let found = state_triggers(&game);
    assert_eq!(found.len(), 1);
    apply_event(
        &mut game,
        &Event::AbilityTriggered(Box::new(found[0].clone())),
    )
    .unwrap();
    assert_eq!(game.pending_triggers().len(), 1);
    assert!(
        state_triggers(&game).is_empty(),
        "{id:?} waits in the queue"
    );
    // The same card as another object (it moved and came back) is not the one that triggered.
    let back = |from: Zone, to: Zone| Event::ZoneChanged {
        object: id,
        from,
        to,
        cause: ZoneCause::Moved,
    };
    apply_event(&mut game, &back(Zone::Graveyard(ME), Zone::Hand(ME))).unwrap();
    apply_event(&mut game, &back(Zone::Hand(ME), Zone::Graveyard(ME))).unwrap();
    assert_eq!(
        state_triggers(&game).len(),
        1,
        "a new object triggers on its own"
    );
}

#[derive(Debug, Clone, Copy)]
enum Left {
    /// The trigger was taken out of the queue (CR 603.3d).
    Removed,
    /// The ability went on the stack and was countered.
    Countered,
}

#[rstest]
fn a_state_trigger_triggers_again_once_its_ability_has_left_the_stack(
    #[values(Left::Removed, Left::Countered)] how: Left,
) {
    let (mut game, id) = game(
        subject(&triggered(Place::Graveyard, MY_TURN)),
        Place::Graveyard,
    );
    let trigger = state_triggers(&game).remove(0);
    apply_event(
        &mut game,
        &Event::AbilityTriggered(Box::new(trigger.clone())),
    )
    .unwrap();
    assert!(state_triggers(&game).is_empty(), "it waits in the queue");
    match how {
        Left::Removed => {
            let removed = Event::TriggerRemoved {
                source: trigger.source,
                ability: trigger.ability,
                origin: trigger.origin,
                delayed: None,
                cause: trigger.cause.clone(),
            };
            apply_event(&mut game, &removed).unwrap();
        }
        Left::Countered => {
            let object = game.next_object();
            let put = PutOnStack {
                object,
                source: trigger.source,
                snapshot: trigger.snapshot.clone(),
                ability: trigger.ability,
                origin: trigger.origin,
                controller: trigger.controller,
                choices: Choices::default(),
                cause: Some(trigger.cause.clone()),
                delayed: None,
            };
            apply_event(&mut game, &Event::AbilityPutOnStack(Box::new(put))).unwrap();
            assert!(game.pending_triggers().is_empty());
            assert!(state_triggers(&game).is_empty(), "it is on the stack");
            let countered = Event::ResolutionEnded {
                object,
                outcome: ResolutionOutcome::Countered,
            };
            apply_event(&mut game, &countered).unwrap();
        }
    }
    let again = state_triggers(&game);
    assert_eq!(again.len(), 1, "{id:?}: the state is still true");
    assert_eq!(again[0].source, trigger.source, "the same object, so the same ability");
}
