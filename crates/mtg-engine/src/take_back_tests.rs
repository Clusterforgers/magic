//! Goal: a take-back rebuilds the exact state before a player's last cast or land play (hand, untapped lands, mana pool, log) with a `TookBack` note naming only the card, and is refused for each reason (house rule off, nothing done, another player acted, the game moved on, a card drawn, revealed or searched for, a library shuffled, anything random); backing out of a cast also undoes the mana tapped for it just before, wherever the cast is backed out of.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::house::HouseRules;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use rstest::rstest;

use super::{TakeBackKind, TakeBackRefusal, refusal};
use crate::choose::TargetChoice;
use crate::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use crate::engine::Engine;
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, setup};
use crate::turn::Step;
use crate::zones::Zone;

const SEAT: PlayerId = PlayerId(0);
const OTHER: PlayerId = PlayerId(1);
const SHOCK: &str = r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":2}}]}}"#;

fn forest() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::basic_land(Some(Color::Green)))
}

fn shock() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Shock")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{1}").unwrap())
            .with_rules(serde_json::from_str(SHOCK).unwrap()),
    )
}

/// Seat 0 holds Shock and a Forest over two Forests and a Bear; seat 1 holds Shock over a Forest.
fn game(rules: HouseRules) -> Engine {
    let mut setup = setup(
        vec![vec![shock(), forest()], vec![shock()]],
        vec![
            vec![forest(), forest(), creature("Bear", 2, 2)],
            vec![forest()],
        ],
    );
    setup.house_rules = rules;
    Engine::new(setup).unwrap()
}

fn hand(engine: &Engine, seat: PlayerId) -> Vec<ObjectId> {
    engine.state().zone(Zone::Hand(seat)).to_vec()
}

fn named(engine: &Engine, seat: PlayerId, name: &str) -> ObjectId {
    hand(engine, seat)
        .into_iter()
        .find(|id| engine.state().name_of(*id).as_deref() == Some(name))
        .unwrap()
}

fn priority(engine: &mut Engine, action: PriorityAction) {
    engine.apply(Answer::Priority(action)).unwrap();
}

/// Begins casting the seat's Shock: the target question is asked.
fn begin_shock(engine: &mut Engine, seat: PlayerId) {
    let spell = named(engine, seat, "Shock");
    priority(engine, PriorityAction::CastSpell(spell));
}

/// Answers the target question with seat 1 and the payment automatically.
fn finish_shock(engine: &mut Engine) {
    let DecisionKind::ChooseTargets(options) = &engine.decision().unwrap().kind else {
        panic!("the target is asked first");
    };
    let target = options.legal[0];
    engine
        .apply(Answer::ChooseTargets(TargetChoice::Targets(vec![target])))
        .unwrap();
    engine.apply(Answer::PayMana(PayChoice::Auto)).unwrap();
}

fn cast_shock(engine: &mut Engine, seat: PlayerId) {
    begin_shock(engine, seat);
    finish_shock(engine);
}

fn tap_a_forest(engine: &mut Engine) {
    let DecisionKind::Priority(options) = &engine.decision().unwrap().kind else {
        panic!("the seat has priority");
    };
    let source = options.mana_abilities[0].source;
    priority(engine, PriorityAction::ActivateMana(source));
}

/// The state, log and decision a take-back must return to.
fn fingerprint(engine: &Engine) -> (u64, Vec<Event>, Vec<ObjectId>, String) {
    (
        engine.state_hash(),
        engine.events().to_vec(),
        hand(engine, SEAT),
        engine.decision().unwrap().kind.name().to_owned(),
    )
}

#[test]
fn a_cast_taken_back_leaves_the_game_as_it_was_with_a_note_that_names_the_card() {
    let mut engine = game(HouseRules::default());
    let before = fingerprint(&engine);
    cast_shock(&mut engine, SEAT);
    let offer = engine.take_back_offer(SEAT).unwrap();
    assert_eq!(
        (offer.kind, offer.name.as_str()),
        (TakeBackKind::Cast, "Shock")
    );
    let events = engine.take_back(SEAT).unwrap();
    assert_eq!(
        events,
        vec![Event::TookBack {
            player: SEAT,
            kind: TakeBackKind::Cast,
            name: "Shock".to_owned(),
        }]
    );
    let after = fingerprint(&engine);
    assert_eq!(
        after.0, before.0,
        "the state hash is the one before the cast"
    );
    assert_eq!(
        after.1[..before.1.len()],
        before.1[..],
        "the log before it stays"
    );
    assert_eq!(
        after.1.len(),
        before.1.len() + 1,
        "only the note follows it"
    );
    assert_eq!(
        (after.2, after.3),
        (before.2, before.3),
        "the card is back and the seat has priority"
    );
    assert_eq!(engine.take_back_offer(SEAT), Err(TakeBackRefusal::Nothing));
}

#[test]
fn mana_tapped_before_a_cast_is_taken_back_with_it() {
    let mut engine = game(HouseRules::default());
    let before = engine.state_hash();
    tap_a_forest(&mut engine);
    cast_shock(&mut engine, SEAT);
    engine.take_back(SEAT).unwrap();
    assert_eq!(
        engine.state_hash(),
        before,
        "the Forest is untapped and the pool empty"
    );
}

#[test]
fn a_land_play_is_taken_back_and_may_be_played_again() {
    let mut engine = game(HouseRules::default());
    let before = engine.state_hash();
    let land = named(&engine, SEAT, "Forest");
    priority(&mut engine, PriorityAction::PlayLand(land));
    assert_eq!(
        engine.take_back_offer(SEAT).map(|offer| offer.kind),
        Ok(TakeBackKind::Land)
    );
    engine.take_back(SEAT).unwrap();
    assert_eq!(engine.state_hash(), before);
    let land = named(&engine, SEAT, "Forest");
    priority(&mut engine, PriorityAction::PlayLand(land));
}

#[test]
fn the_casters_own_pass_does_not_stop_a_take_back() {
    let mut engine = game(HouseRules::default());
    let before = engine.state_hash();
    cast_shock(&mut engine, SEAT);
    priority(&mut engine, PriorityAction::Pass);
    assert_eq!(engine.decision().unwrap().player, OTHER);
    engine.take_back(SEAT).unwrap();
    assert_eq!(engine.state_hash(), before);
    assert_eq!(engine.decision().unwrap().player, SEAT);
}

#[test]
fn a_take_back_can_be_restored_from_a_checkpoint_taken_before_it() {
    let mut engine = game(HouseRules::default());
    let start = engine.events().len();
    cast_shock(&mut engine, SEAT);
    let (hash, log) = (engine.state_hash(), engine.events().to_vec());
    let checkpoint = engine.checkpoint();
    engine.take_back(SEAT).unwrap();
    assert_eq!(engine.cut_since(&checkpoint), Some(start));
    engine.restore(checkpoint).unwrap();
    assert_eq!((engine.state_hash(), engine.events()), (hash, &log[..]));
    assert!(
        engine.take_back_offer(SEAT).is_ok(),
        "the record comes back too"
    );
}

/// How the game goes on after seat 0 casts Shock.
#[derive(Debug, Clone, Copy)]
enum After {
    /// Nothing: the table plays without take-backs.
    HouseRuleOff,
    /// Nothing was cast.
    NothingCast,
    /// Seat 0 passes and seat 1 casts its own Shock in response.
    OtherResponds,
    /// Both pass and Shock resolves.
    Resolves,
}

#[rstest]
#[case::off(After::HouseRuleOff, TakeBackRefusal::Off)]
#[case::nothing(After::NothingCast, TakeBackRefusal::Nothing)]
#[case::another_player_acted(After::OtherResponds, TakeBackRefusal::OtherPlayerActed)]
#[case::it_resolved(After::Resolves, TakeBackRefusal::MovedOn)]
fn a_take_back_is_refused_and_changes_nothing(
    #[case] after: After,
    #[case] reason: TakeBackRefusal,
) {
    let rules = HouseRules {
        take_backs: Some(!matches!(after, After::HouseRuleOff)),
        ..HouseRules::default()
    };
    let mut engine = game(rules);
    if !matches!(after, After::NothingCast) {
        cast_shock(&mut engine, SEAT);
    }
    match after {
        After::OtherResponds => {
            priority(&mut engine, PriorityAction::Pass);
            cast_shock(&mut engine, OTHER);
        }
        After::Resolves => {
            priority(&mut engine, PriorityAction::Pass);
            priority(&mut engine, PriorityAction::Pass);
        }
        After::HouseRuleOff | After::NothingCast => {}
    }
    let before = fingerprint(&engine);
    assert_eq!(engine.take_back_offer(SEAT), Err(reason), "{after:?}");
    assert_eq!(engine.take_back(SEAT), Err(EngineError::TakeBack(reason)));
    assert_eq!(fingerprint(&engine), before, "{after:?}");
}

#[test]
fn only_the_player_who_acted_may_take_it_back() {
    let mut engine = game(HouseRules::default());
    cast_shock(&mut engine, SEAT);
    assert_eq!(
        engine.take_back_offer(OTHER),
        Err(TakeBackRefusal::OtherPlayerActed)
    );
}

fn moved(from: Zone) -> Event {
    Event::ZoneChanged {
        object: ObjectId(99),
        from,
        to: Zone::Graveyard(SEAT),
        cause: ZoneCause::Moved,
    }
}

#[rstest]
#[case::a_draw(Event::CardDrawn { player: SEAT, object: ObjectId(99) }, Some(TakeBackRefusal::CardDrawn))]
#[case::a_card_out_of_a_library(moved(Zone::Library(OTHER)), Some(TakeBackRefusal::CardRevealed))]
#[case::a_card_out_of_another_hand(moved(Zone::Hand(OTHER)), Some(TakeBackRefusal::CardRevealed))]
#[case::a_card_out_of_the_own_hand(moved(Zone::Hand(SEAT)), None)]
#[case::a_shuffle(Event::LibraryShuffled { player: SEAT, order: Vec::new() }, Some(TakeBackRefusal::LibraryShuffled))]
#[case::a_die(Event::DieRolled { player: SEAT, sides: 6, result: 3 }, Some(TakeBackRefusal::Random))]
#[case::a_random_pick(Event::ChosenAtRandom { cards: Vec::new() }, Some(TakeBackRefusal::Random))]
#[case::a_new_step(Event::StepBegan { step: Step::End }, Some(TakeBackRefusal::MovedOn))]
#[case::a_pass(Event::PriorityPassed { player: OTHER }, None)]
fn each_event_since_the_action_is_judged(
    #[case] event: Event,
    #[case] expected: Option<TakeBackRefusal>,
) {
    let engine = game(HouseRules::default());
    let rng = engine.state().rng_index();
    assert_eq!(refusal(&[event], SEAT, rng, engine.state()), expected);
}

#[test]
fn a_draw_of_the_generator_is_random_even_without_an_event() {
    let engine = game(HouseRules::default());
    let rng = engine.state().rng_index();
    assert_eq!(
        refusal(&[], SEAT, rng.wrapping_sub(1), engine.state()),
        Some(TakeBackRefusal::Random)
    );
}

/// Where the cast is backed out of.
#[derive(Debug, Clone, Copy)]
enum Backed {
    AtTheTarget,
    AtThePayment,
}

#[rstest]
#[case::at_the_target(Backed::AtTheTarget, Answer::Undo)]
#[case::at_the_target_with_cancel(Backed::AtTheTarget, Answer::ChooseTargets(TargetChoice::Cancel))]
#[case::at_the_payment(Backed::AtThePayment, Answer::Undo)]
#[case::at_the_payment_with_cancel(Backed::AtThePayment, Answer::PayMana(PayChoice::Cancel))]
fn backing_out_of_a_cast_untaps_the_mana_tapped_for_it(#[case] at: Backed, #[case] back: Answer) {
    let mut engine = game(HouseRules::default());
    let before = fingerprint(&engine);
    tap_a_forest(&mut engine);
    tap_a_forest(&mut engine);
    begin_shock(&mut engine, SEAT);
    if matches!(at, Backed::AtThePayment) {
        let DecisionKind::ChooseTargets(options) = &engine.decision().unwrap().kind else {
            panic!("the target is asked first");
        };
        let target = options.legal[0];
        engine
            .apply(Answer::ChooseTargets(TargetChoice::Targets(vec![target])))
            .unwrap();
        assert_eq!(engine.decision().unwrap().kind.name(), "pay-mana");
    }
    let checkpoint = engine.checkpoint();
    let mark = engine.events().len();
    assert!(engine.apply(back).unwrap().is_empty(), "{at:?}");
    assert_eq!(
        fingerprint(&engine),
        before,
        "{at:?}: both Forests untapped, the pool empty"
    );
    assert_eq!(engine.cut_since(&checkpoint), Some(before.1.len()));
    assert!(mark > before.1.len());
}

#[test]
fn mana_tapped_and_then_left_is_not_undone_by_a_later_cancel() {
    let mut engine = game(HouseRules::default());
    tap_a_forest(&mut engine);
    let land = named(&engine, SEAT, "Forest");
    priority(&mut engine, PriorityAction::PlayLand(land));
    let tapped = engine.state_hash();
    begin_shock(&mut engine, SEAT);
    engine.apply(Answer::Undo).unwrap();
    assert_eq!(
        engine.state_hash(),
        tapped,
        "the land play ended the run of mana"
    );
}
