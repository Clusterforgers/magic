//! Goal: a decision of a spell or ability tells its seat what asks (the stack object, the source, the ability's text with the asked clause marked, the object that triggered it and the place in a run of triggers), the triggers to order carry their effect and cause, and nothing of a hidden or departed object is told.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardIdentity;
use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_cards::trigger::TriggerPattern;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{Target, YesNoChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, DecisionKind};
use mtg_protocol::AskingView;
use mtg_testkit::cards::{burn, creature, forest, free_creature, gain_life, mountain};
use mtg_testkit::trigger_cards::{watcher, when};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const ME: PlayerId = PlayerId(0);

/// "you may gain 1 life."
fn may_gain() -> Vec<Effect> {
    vec![Effect::May {
        who: Ref::You,
        effects: vec![gain_life(1)],
    }]
}

/// What asks the pending decision, as its seat sees it.
fn asking(driver: &Driver) -> Option<AskingView> {
    let decision = driver.decision().unwrap();
    AskingView::of(driver.state(), decision.player, decision)
}

/// Passes priority until a decision that is not priority waits.
fn pass_to_question(driver: &mut Driver) {
    for _ in 0..12 {
        if !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_)) {
            return;
        }
        driver.pass().unwrap();
    }
    panic!("no question came");
}

fn say(driver: &mut Driver, yes: bool) {
    driver.answer(Answer::YesNo(YesNoChoice(yes))).unwrap();
}

/// The words of `asking`'s text its clause marks.
fn marked(asking: &AskingView) -> Option<String> {
    let (start, end) = asking.clause?;
    let units: Vec<u16> = asking.text.as_deref()?.encode_utf16().collect();
    String::from_utf16(&units[start as usize..end as usize]).ok()
}

/// A table whose seat 0 controls a watcher per name, each asking "you may gain 1 life" on `when`, and holds a Cub.
fn watchers(names: &[&str], when: fn() -> TriggerPattern) -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                names
                    .iter()
                    .map(|name| ready(watcher(name, when(), may_gain())))
                    .collect::<Vec<_>>(),
            )
            .hand(0, [free_creature("Cub", 1, 1)]),
    )
    .unwrap()
}

#[test]
fn a_may_trigger_names_its_stack_object_source_clause_and_cause() {
    let mut driver = watchers(&["Mentor"], when::any_creature_enters);
    driver.cast("Cub").unwrap();
    pass_to_question(&mut driver);
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::YesNo(_)
    ));
    let asking = asking(&driver).unwrap();
    let top = *driver.state().stack().last().unwrap();
    assert_eq!(asking.stack, Some(top));
    assert!(asking.resolving);
    assert_eq!(asking.source, Some(driver.id_of("Mentor").unwrap()));
    assert_eq!(asking.name, "Mentor");
    assert_eq!(asking.ability, Some(0));
    let text = asking.text.clone().unwrap();
    assert!(text.contains(" enters"), "{text}");
    assert_eq!(
        marked(&asking).as_deref(),
        Some("you may gain 1 life."),
        "{text}"
    );
    let cause = asking.cause.unwrap();
    assert_eq!(cause.object, driver.id_of("Cub").unwrap());
    assert_eq!(cause.name, "Cub");
    assert!(cause.entered);
    assert_eq!(asking.run, None, "one trigger is no run");
}

#[rstest]
#[case::two(&["Alpha", "Beta"])]
#[case::three(&["Alpha", "Beta", "Gamma"])]
#[case::four(&["Alpha", "Beta", "Gamma", "Delta"])]
fn the_triggers_to_order_carry_their_effect_and_cause_and_each_question_its_place_in_the_run(
    #[case] names: &[&str],
) {
    let mut driver = watchers(names, when::any_creature_enters);
    driver.cast("Cub").unwrap();
    pass_to_question(&mut driver);
    let cub = driver.id_of("Cub").unwrap();
    let DecisionKind::OrderTriggers(options) = &driver.decision().unwrap().kind else {
        panic!("the order is asked first");
    };
    assert_eq!(options.triggers.len(), names.len());
    for entry in &options.triggers {
        assert_eq!(entry.text, "You may gain 1 life.");
        assert_eq!(entry.cause, Some(cub));
    }
    assert_eq!(
        asking(&driver),
        None,
        "the order is asked by no one trigger"
    );
    let sources: Vec<ObjectId> = options
        .triggers
        .iter()
        .map(|entry| entry.source.id)
        .collect();
    let order: Vec<usize> = (0..names.len()).collect();
    driver.order_triggers(&order).unwrap();
    // The last one put on the stack resolves first.
    let resolving: Vec<ObjectId> = sources.iter().rev().copied().collect();
    let mut run_id = None;
    for (at, source) in resolving.iter().enumerate() {
        pass_to_question(&mut driver);
        let asking = asking(&driver).unwrap();
        assert_eq!(asking.source, Some(*source));
        let run = asking.run.clone().unwrap();
        assert_eq!(run.index as usize, at + 1);
        assert_eq!(run.entries.len(), names.len());
        assert_eq!(*run_id.get_or_insert(run.id), run.id, "one run, one id");
        for (place, entry) in run.entries.iter().enumerate() {
            assert_eq!(entry.done, place < at, "{place} of {at}");
            assert_eq!(entry.object.is_some(), place >= at);
            assert_eq!(entry.source, Some(resolving[place]));
            assert_ne!(entry.name, "");
            assert_eq!(
                entry.text.as_deref(),
                (place >= at).then_some("You may gain 1 life.")
            );
        }
        assert_eq!(run.entries[at].object, asking.stack);
        say(&mut driver, at % 2 == 0);
    }
}

/// The printed card a test card is.
fn printed() -> CardIdentity {
    CardIdentity {
        oracle_id: "martyr-oracle".into(),
        printing_id: Some("martyr-print".into()),
    }
}

/// Seat 0 controls a Martyr that asks "you may gain 1 life" as it dies, and holds a Bolt for it.
fn martyr_table(identity: Option<CardIdentity>) -> Driver {
    let mut martyr = (*watcher("Martyr", when::this_dies(), may_gain())).clone();
    martyr.identity = identity;
    Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(Arc::new(martyr)), ready(mountain())])
            .hand(0, [burn("Bolt", 3)]),
    )
    .unwrap()
}

#[rstest]
#[case::printed(Some(printed()))]
#[case::a_token(None)]
fn a_source_that_left_keeps_the_card_it_last_was(#[case] identity: Option<CardIdentity>) {
    let mut driver = martyr_table(identity.clone());
    let martyr = driver
        .state()
        .object(driver.id_of("Martyr").unwrap())
        .unwrap()
        .reference();
    driver.cast_at("Bolt", &[Target::Object(martyr)]).unwrap();
    pass_to_question(&mut driver);
    let asking = asking(&driver).unwrap();
    assert_eq!(asking.source, None);
    assert_eq!(asking.identity, identity);
}

#[test]
fn a_source_still_seen_is_pointed_at_and_not_pictured_again() {
    let mut driver = watchers(&["Mentor"], when::any_creature_enters);
    driver.cast("Cub").unwrap();
    pass_to_question(&mut driver);
    assert!(asking(&driver).unwrap().source.is_some());
    assert_eq!(asking(&driver).unwrap().identity, None);
}

#[test]
fn a_source_that_left_is_named_and_not_pointed_at() {
    let mut driver = martyr_table(None);
    let martyr = driver
        .state()
        .object(driver.id_of("Martyr").unwrap())
        .unwrap()
        .reference();
    driver.cast_at("Bolt", &[Target::Object(martyr)]).unwrap();
    pass_to_question(&mut driver);
    let asking = asking(&driver).unwrap();
    assert_eq!(asking.name, "Martyr");
    assert_eq!(
        asking.source, None,
        "the card in the graveyard is a new object"
    );
    assert!(asking.stack.is_some() && asking.text.is_some());
    assert_eq!(asking.cause, None, "what died lies on no battlefield");
}

#[test]
fn a_card_drawn_is_no_cause_anyone_is_shown() {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 1, Step::Upkeep)
            .battlefield(0, [ready(watcher("Spy", when::anyone_draws(), may_gain()))])
            .library(1, [forest(), forest()]),
    )
    .unwrap();
    pass_to_question(&mut driver);
    let decision = driver.decision().unwrap();
    assert_eq!(decision.player, ME);
    let asking = asking(&driver).unwrap();
    assert_eq!(asking.source, Some(driver.id_of("Spy").unwrap()));
    assert_eq!(asking.cause, None, "the card drawn is in a hand");
    let hidden: Vec<ObjectId> = driver.state().zone(Zone::Hand(PlayerId(1))).to_vec();
    let sent = serde_json::to_string(&asking).unwrap();
    for id in hidden {
        assert!(!sent.contains(&format!(":{}", id.get())), "{sent}");
    }
}

#[test]
fn a_payment_is_asked_by_the_spell_being_cast() {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(forest()), ready(forest())])
            .hand(0, [creature("Ogre", 2, &[], 3, 3)]),
    )
    .unwrap();
    driver.begin_cast("Ogre").unwrap();
    let DecisionKind::PayMana(options) = &driver.decision().unwrap().kind else {
        panic!("the cost is asked");
    };
    let spell = options.spell;
    let asking = asking(&driver).unwrap();
    assert_eq!(asking.name, "Ogre");
    assert_eq!(asking.source, Some(spell));
    assert!(!asking.resolving, "a spell being cast is not resolving");
    assert_eq!(
        (asking.ability, asking.clause, asking.run),
        (None, None, None)
    );
}

#[test]
fn priority_is_asked_by_nothing() {
    let driver = watchers(&["Mentor"], when::any_creature_enters);
    assert_eq!(asking(&driver), None);
}
