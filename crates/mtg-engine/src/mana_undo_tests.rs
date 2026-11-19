//! Goal: a mana ability activated at priority (a land, a mana rock, a creature) is undone by default at 2 to 4 seats: the source untaps, the pool empties, the log is cut back with a `ManaUndone` note and replays to the same state, and the undo is offered on the offered decision only, so a rebuilt engine asks the same; it is refused (offered with its reason, or not offered) when its activation triggered or did more than add mana, when its mana was spent, and once priority was passed.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::{ManaUndo, ManaUndoRefusal};
use crate::decision::{Answer, DecisionKind, PriorityAction, PriorityOptions};
use crate::engine::{Engine, replay};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::fixtures::{creature, setup};
use crate::zones::Zone;

const SEAT: PlayerId = PlayerId(0);
const ROCK: &str =
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#;
const DORK: &str =
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#;
const SOL_RING: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]}}}]}}"#;
const SIGNET: &str = r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green","Green"]}}}]}}"#;
const PAINLAND: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}},{"DealDamage":{"to":"You","amount":1}}]}}"#;
const MANABARBS: &str = r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}]}},"effects":[{"DealDamage":{"to":"TriggeringPlayer","amount":1}}]}}"#;

fn forest() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::basic_land(Some(Color::Green)))
}

fn card(name: &str, kind: CardType, rules: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

fn dork() -> Arc<CardDefinition> {
    let body = CardDefinition::new("Llanowar Elves")
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1)
        .with_rules(serde_json::from_str(DORK).unwrap());
    Arc::new(body)
}

/// Seat 0 holds a Forest, Mind Stone, Llanowar Elves, a painland, Sol Ring and a Signet; each other seat a Forest and a
/// Bear, and seat 1 also `extra` when given.
fn game(seats: usize, extra: Option<&Arc<CardDefinition>>) -> Engine {
    let mut battlefields = vec![vec![
        forest(),
        card("Mind Stone", CardType::Artifact, ROCK),
        dork(),
        card("Painland", CardType::Land, PAINLAND),
        card("Sol Ring", CardType::Artifact, SOL_RING),
        card("Signet", CardType::Artifact, SIGNET),
    ]];
    for seat in 1..seats {
        let mut own = vec![forest(), creature("Bear", 2, 2)];
        if seat == 1
            && let Some(extra) = extra
        {
            own.push(Arc::clone(extra));
        }
        battlefields.push(own);
    }
    let hands = vec![Vec::new(); seats];
    Engine::new(setup(hands, battlefields)).unwrap()
}

fn options(engine: &Engine) -> PriorityOptions {
    options_of(engine.decision().cloned())
}

fn options_of(decision: Option<crate::decision::Decision>) -> PriorityOptions {
    match &decision.unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("priority is asked, not {}", other.name()),
    }
}

fn source(engine: &Engine, name: &str) -> ObjectId {
    engine
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            engine.state().name_of(*id).as_deref() == Some(name)
                && engine.state().object(*id).unwrap().controller == SEAT
        })
        .unwrap()
}

fn tap(engine: &mut Engine, name: &str) -> ObjectId {
    let id = source(engine, name);
    let option = options(engine)
        .mana_abilities
        .into_iter()
        .find(|option| option.source.object == id)
        .unwrap_or_else(|| panic!("{name} can be tapped for mana"));
    engine
        .apply(Answer::Priority(PriorityAction::ActivateMana(
            option.source,
        )))
        .unwrap();
    id
}

fn undo(engine: &mut Engine, id: ObjectId) -> Result<Vec<Event>, EngineError> {
    engine.apply(Answer::Priority(PriorityAction::UndoMana(id)))
}

fn tapped(engine: &Engine, id: ObjectId) -> bool {
    engine
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

fn pool(engine: &Engine) -> u32 {
    engine.state().player(SEAT).unwrap().mana().total()
}

/// The log replayed from the setup reaches the engine's state.
fn replays(engine: &Engine) -> bool {
    let state = replay(engine.setup(), engine.events()).unwrap();
    let mut rebuilt = engine.clone();
    rebuilt.restore(rebuilt.checkpoint()).unwrap();
    state.state_hash() == engine.state_hash() && rebuilt.state_hash() == engine.state_hash()
}

#[rstest]
fn a_mana_ability_is_undone_and_the_game_is_as_before(
    #[values("Forest", "Mind Stone", "Llanowar Elves")] name: &str,
    #[values(2, 3, 4)] seats: usize,
) {
    let mut engine = game(seats, None);
    let (hash, log) = (engine.state_hash(), engine.events().to_vec());
    let id = tap(&mut engine, name);
    assert!(
        tapped(&engine, id) && pool(&engine) == 1,
        "{name} made its mana"
    );
    assert_eq!(
        engine.mana_undo(SEAT),
        vec![ManaUndo {
            source: id,
            refusal: None
        }]
    );
    assert_eq!(
        options_of(engine.offered_decision()).undo_mana,
        engine.mana_undo(SEAT),
        "the offered decision lists it"
    );
    let resumed = Engine::resume(engine.setup().clone(), engine.events().to_vec()).unwrap();
    assert_eq!(
        resumed.decision(),
        engine.decision(),
        "a rebuilt engine asks the same"
    );
    assert_eq!(
        resumed.mana_undo(SEAT),
        Vec::new(),
        "and offers no undo, as no take-back"
    );
    let checkpoint = engine.checkpoint();
    let after = undo(&mut engine, id).unwrap();
    assert_eq!(
        after,
        vec![Event::ManaUndone {
            player: SEAT,
            source: id
        }]
    );
    assert_eq!(
        engine.cut_since(&checkpoint),
        Some(log.len()),
        "the log is cut where the tap began"
    );
    assert!(
        !tapped(&engine, id) && pool(&engine) == 0,
        "{name} is untapped and the pool empty"
    );
    assert_eq!(
        engine.state_hash(),
        hash,
        "the state is the one before the tap"
    );
    assert_eq!(engine.events()[..log.len()], log[..]);
    assert!(engine.mana_undo(SEAT).is_empty(), "nothing is left to undo");
    assert!(replays(&engine), "the log replays to the same state");
    assert_eq!(
        engine.decision().unwrap().player,
        SEAT,
        "the seat keeps priority"
    );
}

#[test]
fn either_of_two_taps_is_undone_and_the_other_stays() {
    let mut engine = game(2, None);
    let hash = engine.state_hash();
    let forest = tap(&mut engine, "Forest");
    let stone = tap(&mut engine, "Mind Stone");
    undo(&mut engine, forest).unwrap();
    assert!(!tapped(&engine, forest) && tapped(&engine, stone));
    assert_eq!(pool(&engine), 1, "the Mind Stone's mana stays");
    assert!(replays(&engine));
    assert_eq!(
        engine.mana_undo(SEAT),
        vec![ManaUndo {
            source: stone,
            refusal: None
        }]
    );
    undo(&mut engine, stone).unwrap();
    assert_eq!(engine.state_hash(), hash);
    assert!(replays(&engine));
}

#[test]
fn a_restore_from_before_the_undo_brings_the_tap_back() {
    let mut engine = game(2, None);
    let forest = tap(&mut engine, "Forest");
    let (hash, log) = (engine.state_hash(), engine.events().to_vec());
    let checkpoint = engine.checkpoint();
    undo(&mut engine, forest).unwrap();
    engine.restore(checkpoint).unwrap();
    assert_eq!((engine.state_hash(), engine.events()), (hash, &log[..]));
}

/// What happens between the tap and the press.
#[derive(Debug, Clone, Copy)]
enum Case {
    /// An opponent's Manabarbs triggers on the Forest.
    TapTrigger,
    /// The painland deals its damage.
    MoreThanMana,
    /// Sol Ring's {C}{C}, then a Signet paid with one of them.
    PartlySpent,
}

#[rstest]
#[case::a_tap_trigger(Case::TapTrigger, "Forest", ManaUndoRefusal::Triggered)]
#[case::more_than_mana(Case::MoreThanMana, "Painland", ManaUndoRefusal::DidMore)]
#[case::partly_spent(Case::PartlySpent, "Sol Ring", ManaUndoRefusal::Spent)]
fn an_undo_is_refused_with_its_reason(
    #[case] case: Case,
    #[case] name: &str,
    #[case] reason: ManaUndoRefusal,
    #[values(2, 4)] seats: usize,
) {
    let barbs = matches!(case, Case::TapTrigger)
        .then(|| card("Manabarbs", CardType::Enchantment, MANABARBS));
    let mut engine = game(seats, barbs.as_ref());
    let id = tap(&mut engine, name);
    if matches!(case, Case::PartlySpent) {
        tap(&mut engine, "Signet");
    }
    let offered = engine.mana_undo(SEAT);
    assert!(
        offered.contains(&ManaUndo {
            source: id,
            refusal: Some(reason)
        }),
        "{offered:?}"
    );
    let log = engine.events().len();
    let refused = undo(&mut engine, id).unwrap_err();
    assert_eq!(
        refused,
        EngineError::Illegal(IllegalAnswer::ManaUndo(reason))
    );
    assert!(
        tapped(&engine, id) && engine.events().len() == log,
        "nothing changed"
    );
}

#[rstest]
fn an_undo_is_refused_once_priority_was_passed(#[values(2, 3, 4)] seats: usize) {
    let mut engine = game(seats, None);
    let forest = tap(&mut engine, "Forest");
    engine
        .apply(Answer::Priority(PriorityAction::Pass))
        .unwrap();
    while engine.decision().unwrap().player != SEAT {
        assert!(
            engine
                .mana_undo(engine.decision().unwrap().player)
                .is_empty(),
            "another seat is never offered the undo"
        );
        engine
            .apply(Answer::Priority(PriorityAction::Pass))
            .unwrap();
    }
    assert_eq!(engine.mana_undo(SEAT), Vec::new());
    let refused = undo(&mut engine, forest).unwrap_err();
    assert_eq!(
        refused,
        EngineError::Illegal(IllegalAnswer::NotOffered("mana to undo"))
    );
}
