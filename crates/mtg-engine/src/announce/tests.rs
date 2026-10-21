//! Goal: a cast or activation asks its questions in the order of the rules, targets before payment, passes over a specification that needs no choice, gives up on one that cannot be met, completes an activation that costs only a tap with the events that pay and announce it, asks the payment of one that costs mana (never from its own source when it taps it, announcing between the life and the mana), and pays for an effect with no way back.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use rstest::rstest;

use mtg_core::ids::PlayerId;

use super::{Next, Pending, What, announce};
use crate::abilities::AbilityRef;
use crate::answers::Outcome;
use crate::cast::{pay_answer, pay_choice, payment_decision};
use crate::choose::{Choice, Target};
use crate::decision::{Decision, DecisionKind, PayChoice};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::priority::mana_options;
use crate::state::GameState;
use crate::zones::Zone;

fn spell(targets: &str) -> Arc<CardDefinition> {
    let rules = format!(r#"{{"Spell":{{"targets":{targets},"effects":[{{"Draw":{{}}}}]}}}}"#);
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{1}").unwrap())
            .with_rules(serde_json::from_str(&rules).unwrap()),
    )
}

fn pending_for(game: &GameState) -> Pending {
    let card = game.zone(Zone::Hand(PlayerId(0)))[0];
    Pending::new(PlayerId(0), What::Spell(card))
}

fn asks(next: &Next) -> &'static str {
    match next {
        Next::Ask(Decision {
            kind: DecisionKind::ChooseTargets(_),
            ..
        }) => "targets",
        Next::Ask(Decision {
            kind: DecisionKind::PayMana(_),
            ..
        }) => "payment",
        Next::Ask(_) => "something else",
        Next::Commit(_) => "commit",
        Next::Drop => "drop",
    }
}

#[rstest]
#[case::one_specification(r#"[{"what":"Player"}]"#, 1)]
#[case::two_specifications(r#"[{"what":"Player"},{"what":"AnyTarget"}]"#, 2)]
fn targets_are_asked_one_specification_at_a_time_and_the_payment_follows(
    #[case] targets: &str,
    #[case] specs: usize,
) {
    let game = state(
        vec![vec![spell(targets)], vec![]],
        vec![vec![forest()], vec![]],
    );
    let mut pending = pending_for(&game);
    for index in 0..specs {
        let next = announce(&game, &mut pending);
        let Next::Ask(Decision {
            kind: DecisionKind::ChooseTargets(options),
            player,
        }) = next
        else {
            panic!("a target decision was expected for specification {index}");
        };
        assert_eq!((options.spec, player), (index, PlayerId(0)));
        assert!(options.can_cancel, "the cast has cost nothing yet");
        pending.choices.targets.push(vec![options.legal[0]]);
    }
    assert_eq!(asks(&announce(&game, &mut pending)), "payment");
}

#[test]
fn a_specification_that_needs_nothing_is_passed_over_and_one_that_cannot_be_met_ends_the_cast() {
    // "Up to one target creature" with no creature to target needs no decision.
    let optional = r#"[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":1}}]"#;
    let game = state(
        vec![vec![spell(optional)], vec![]],
        vec![vec![forest()], vec![]],
    );
    let mut pending = pending_for(&game);
    assert_eq!(asks(&announce(&game, &mut pending)), "payment");
    assert_eq!(pending.choices.targets, [Vec::<Target>::new()]);
    // "Target creature" with none to target cannot be announced at all.
    let required = r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#;
    let game = state(vec![vec![spell(required)], vec![]], vec![vec![], vec![]]);
    let mut pending = pending_for(&game);
    assert_eq!(asks(&announce(&game, &mut pending)), "drop");
    // With a creature in play it can.
    let game = state(
        vec![vec![spell(required)], vec![]],
        vec![vec![forest()], vec![creature("Bear", 2, 2)]],
    );
    let mut pending = pending_for(&game);
    assert_eq!(asks(&announce(&game, &mut pending)), "targets");
}

#[test]
fn a_spell_without_targets_goes_straight_to_the_payment() {
    let game = state(
        vec![vec![spell("[]")], vec![]],
        vec![vec![forest()], vec![]],
    );
    let mut pending = pending_for(&game);
    assert_eq!(asks(&announce(&game, &mut pending)), "payment");
}

fn tapper() -> Arc<CardDefinition> {
    let rules = r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#;
    Arc::new(
        CardDefinition::new("Tapper")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

#[test]
fn an_activation_that_costs_a_tap_is_completed_by_the_events_that_pay_and_announce_it() {
    let game = state(vec![vec![], vec![]], vec![vec![tapper()], vec![]]);
    let source = game.zone(Zone::Battlefield)[0];
    let mut pending = Pending::new(PlayerId(0), What::Ability(AbilityRef::printed(source, 0)));
    let Next::Commit(events) = announce(&game, &mut pending) else {
        panic!("a tap needs no decision");
    };
    let [
        Event::PermanentTapped { object },
        Event::AbilityActivated {
            player,
            source: activated,
            ability,
            ..
        },
        Event::AbilityPutOnStack(put),
    ] = &events[..]
    else {
        panic!("tap, activation, ability on the stack: {events:?}");
    };
    assert_eq!((*object, *player, *ability), (source, PlayerId(0), 0));
    assert_eq!(*activated, game.object(source).unwrap().reference());
    assert_eq!(put.object, game.next_object());
    assert_eq!(put.source, *activated);
    assert_eq!(put.snapshot.zone, Zone::Battlefield);
    assert!(put.cause.is_none() && put.choices.is_empty());
}

fn gadget(cost: &str) -> Arc<CardDefinition> {
    let rules = format!(r#"{{"Activated":{{"cost":{cost},"effects":[{{"Draw":{{}}}}]}}}}"#);
    Arc::new(
        CardDefinition::new("Gadget")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(&rules).unwrap()),
    )
}

fn forest() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::basic_land(Some(
        mtg_core::color::Color::Green,
    )))
}

/// Seat 0 controls a Gadget with this cost and two Forests.
fn with_gadget(cost: &str) -> (GameState, Pending) {
    let game = state(
        vec![vec![], vec![]],
        vec![vec![gadget(cost), forest(), forest()], vec![]],
    );
    let source = game.zone(Zone::Battlefield)[0];
    let ability = AbilityRef::printed(source, 0);
    (game, Pending::new(PlayerId(0), What::Ability(ability)))
}

#[test]
fn an_activation_that_costs_mana_asks_for_the_payment_and_cannot_use_its_own_source_when_it_taps() {
    for (cost, sources) in [(r#""{1}""#, 2), (r#"[{"Mana":"{1}"},"Tap"]"#, 2)] {
        let (game, mut pending) = with_gadget(cost);
        let Next::Ask(Decision {
            player,
            kind: DecisionKind::PayMana(options),
        }) = announce(&game, &mut pending)
        else {
            panic!("a payment was expected for {cost}");
        };
        assert_eq!(player, PlayerId(0));
        assert_eq!(options.cost, ManaCost::parse("{1}").unwrap());
        assert_eq!(options.sources.len(), sources, "{cost}");
        assert!(options.can_cancel && options.auto_available);
        assert_eq!(options.spell, game.zone(Zone::Battlefield)[0]);
    }
}

#[rstest]
#[case::only_mana(r#""{1}""#, false)]
#[case::mana_and_tap(r#"[{"Mana":"{1}"},"Tap"]"#, true)]
fn the_payment_of_an_activation_announces_it_between_the_life_and_the_mana(
    #[case] cost: &str,
    #[case] taps: bool,
) {
    let (game, mut pending) = with_gadget(cost);
    let Next::Ask(Decision {
        kind: DecisionKind::PayMana(options),
        ..
    }) = announce(&game, &mut pending)
    else {
        panic!("a payment was expected");
    };
    let Ok(Outcome::Events(events)) = pay_answer(
        &game,
        PlayerId(0),
        &options,
        PayChoice::Auto,
        Some(&pending),
    ) else {
        panic!("the payment must be accepted");
    };
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| match event {
            Event::PermanentTapped { .. } => "tapped",
            Event::AbilityActivated { .. } => "activated",
            Event::AbilityPutOnStack(_) => "on the stack",
            Event::ManaAdded { .. } => "mana",
            Event::ManaSpent { .. } => "spent",
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    let mut expected = if taps { vec!["tapped"] } else { Vec::new() };
    expected.extend(["activated", "on the stack", "tapped", "mana", "spent"]);
    assert_eq!(kinds, expected);
    // The source is photographed before anything is tapped.
    let put = events.iter().find_map(|event| match event {
        Event::AbilityPutOnStack(put) => Some(put),
        _ => None,
    });
    assert!(put.is_some_and(|put| put.snapshot.zone == Zone::Battlefield));
}

#[test]
fn a_payment_for_an_effect_is_logged_as_an_answer_and_cannot_be_cancelled() {
    let (game, _) = with_gadget(r#""{1}""#);
    let tapped = Vec::new();
    let options = payment_decision(
        &game,
        PlayerId(0),
        game.zone(Zone::Battlefield)[0],
        ManaCost::parse("{1}").unwrap(),
        mana_options(&game, PlayerId(0)),
        crate::spend::Spend::other(),
        false,
        0,
    )
    .unwrap();
    // Nothing resolves here, so no effect asked for it.
    assert!(matches!(
        pay_answer(&game, PlayerId(0), &options, PayChoice::Auto, None),
        Err(EngineError::InvalidEvent(_))
    ));
    let announced = vec![Event::ChoiceMade {
        player: PlayerId(0),
        choice: Choice::YesNo(true),
    }];
    assert_eq!(
        pay_choice(
            PlayerId(0),
            &options,
            &tapped,
            PayChoice::Cancel,
            announced.clone()
        )
        .unwrap_err(),
        EngineError::Illegal(IllegalAnswer::NotOffered("cancel"))
    );
    let Ok(Outcome::Events(events)) =
        pay_choice(PlayerId(0), &options, &tapped, PayChoice::Auto, announced)
    else {
        panic!("the payment must be accepted");
    };
    assert!(matches!(
        events[0],
        Event::ChoiceMade {
            choice: Choice::YesNo(true),
            ..
        }
    ));
    assert!(matches!(events.last(), Some(Event::ManaSpent { .. })));
}
