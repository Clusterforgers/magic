//! Goal: a seat's rules for its own triggers answer for it out of Full Control: an optional trigger's yes or no and the order of triggers that go on together, each only for its own key; a yield to an ability passes a seat at 2 to 4 seats only while that ability is on top, so an opponent's other spell still stops it; Full Control asks anyway, and the rules end with the game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, game_id, roomy, seat};
use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{Choice, Target, TargetChoice};
use mtg_engine::decision::{PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Decision, DecisionKind, Event, GameSetup, GameState};
use mtg_protocol::{
    AutoSkip, AutoYield, ClientMessage, DecisionId, ErrorCode, FullControlMode, ServerMessage,
    TriggerKey,
};
use mtg_server::{Dwell, EventStore, GameConfig, GameHandle, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::cards::{burn, free_creature, gain_life, instant, mountain, pinger};
use mtg_testkit::trigger_cards::{watcher, when};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

/// No clock and no dwell: only the windows and the rules decide who is asked.
const OFF: Pace = Pace {
    auto_pass: true,
    response: None,
    blocks: None,
    think: None,
    active: None,
    extension: None,
    grace: std::time::Duration::ZERO,
    dwell: Dwell::OFF,
};

fn spawn(setup: GameSetup) -> (GameHandle, MemoryStore) {
    let store = MemoryStore::new();
    let config = GameConfig {
        pace: OFF,
        ..roomy()
    };
    let handle = spawn_game_with(game_id(1), setup, store.clone(), config).unwrap();
    (handle, store)
}

async fn send(handle: &GameHandle, from: &Client, message: ClientMessage) {
    from.say(handle, message).await;
    handle.status().await.unwrap();
}

async fn reply(handle: &GameHandle, from: &Client, decision: DecisionId, answer: Answer) {
    send(handle, from, ClientMessage::Answer { decision, answer }).await;
}

/// Lets every queued command run.
async fn quiet(handle: &GameHandle) {
    for _ in 0..20 {
        tokio::task::yield_now().await;
        handle.status().await.unwrap();
    }
}

async fn waiting(handle: &GameHandle) -> Option<PlayerId> {
    handle.status().await.unwrap().waiting_on
}

/// The list in the next `AutoSkips` notice `client` gets.
async fn told(client: &mut Client) -> Vec<AutoSkip> {
    client
        .until(|message| match message {
            ServerMessage::AutoSkips { rules } => Some(rules.clone()),
            _ => None,
        })
        .await
}

/// Whether `messages` hold a decision of the kind `name`.
fn sent_kind(messages: &[ServerMessage], name: &str) -> bool {
    messages.iter().any(|message| {
        matches!(message, ServerMessage::Decision { decision, .. } if decision.kind.name() == name)
    })
}

fn object_named(setup: &GameSetup, name: &str) -> ObjectId {
    let state = GameState::from_setup(setup).unwrap();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap()
}

/// Seat 0 holds a Bolt and a Mountain and burns the Victim of seat 1, which has these `watchers` as well.
fn burn_table(watchers: Vec<std::sync::Arc<mtg_cards::CardDefinition>>) -> GameSetup {
    let mut permanents = vec![ready(free_creature("Victim", 1, 1))];
    permanents.extend(watchers.into_iter().map(ready));
    Scenario::new(2)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, permanents)
        .setup()
        .unwrap()
}

/// Seat 0 casts its Bolt at the Victim; the Victim dies and what it set off triggers.
async fn burn_the_victim(handle: &GameHandle, clients: &mut [Client], victim: ObjectId) {
    let (id, decision) = clients[0].decision().await;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("seat 0 holds priority");
    };
    let bolt = Answer::Priority(PriorityAction::CastSpell(options.castable[0]));
    reply(handle, &clients[0], id, bolt).await;
    let (id, decision) = clients[0].decision().await;
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        panic!("the spell asks for its target, got {decision:?}");
    };
    let victim = options
        .legal
        .iter()
        .copied()
        .find(|t| matches!(t, Target::Object(r) if r.id == victim))
        .unwrap();
    let aim = Answer::ChooseTargets(TargetChoice::Targets(vec![victim]));
    reply(handle, &clients[0], id, aim).await;
    let (id, _) = clients[0].decision().await;
    reply(handle, &clients[0], id, Answer::PayMana(PayChoice::Auto)).await;
}

/// Both seats join; seat 1 takes `mode`, and both say they are ready.
async fn join(handle: &GameHandle, mode: FullControlMode) -> Vec<Client> {
    let mut clients = Vec::new();
    for n in 0..2 {
        clients.push(Client::join(handle, n).await);
    }
    send(handle, &clients[1], ClientMessage::FullControl { mode }).await;
    clients
}

async fn ready_all(handle: &GameHandle, clients: &[Client]) {
    for client in clients {
        send(handle, client, ClientMessage::Ready).await;
    }
}

/// The Owl's optional trigger: when a creature dies, its controller may gain 3 life.
fn owl() -> std::sync::Arc<mtg_cards::CardDefinition> {
    let may = Effect::May {
        who: Ref::You,
        effects: vec![gain_life(3)],
    };
    watcher("Owl", when::any_creature_dies(), vec![may])
}

/// What seat 1 holds for the Owl's trigger when it resolves.
#[derive(Debug, Clone, Copy)]
enum Rule {
    None,
    Yes,
    No,
    OtherAbility,
}

#[rstest]
#[case::no_rule_asks(Rule::None, FullControlMode::Off, true)]
#[case::always_yes(Rule::Yes, FullControlMode::Off, false)]
#[case::always_no(Rule::No, FullControlMode::Off, false)]
#[case::another_abilitys_rule_asks(Rule::OtherAbility, FullControlMode::Off, true)]
#[case::full_control_asks_anyway(Rule::Yes, FullControlMode::Held, true)]
#[tokio::test(start_paused = true)]
async fn an_answer_rule_answers_its_own_optional_trigger_out_of_full_control(
    #[case] rule: Rule,
    #[case] mode: FullControlMode,
    #[case] asked: bool,
) {
    let setup = burn_table(vec![owl()]);
    let victim = object_named(&setup, "Victim");
    let (handle, store) = spawn(setup);
    // Seat 1 starts in Full Control so it is asked with its trigger on the stack and reads the trigger's key there.
    let mut clients = join(&handle, FullControlMode::Held).await;
    ready_all(&handle, &clients).await;
    burn_the_victim(&handle, &mut clients, victim).await;
    let ability = loop {
        let (id, decision) = clients[1].decision().await;
        assert!(matches!(decision.kind, DecisionKind::Priority(_)));
        if let Some(key) = top_key(&handle, &mut clients[1]).await {
            break key;
        }
        let pass = Answer::Priority(PriorityAction::Pass);
        reply(&handle, &clients[1], id, pass).await;
    };
    assert_eq!(ability.name, "Owl");
    let held = match rule {
        Rule::None => None,
        Rule::Yes | Rule::No => Some(AutoSkip::Answer {
            ability: ability.clone(),
            yes: matches!(rule, Rule::Yes),
        }),
        Rule::OtherAbility => Some(AutoSkip::Answer {
            ability: AutoYield {
                name: "Owl".into(),
                text: "Draw a card.".into(),
            },
            yes: true,
        }),
    };
    if let Some(rule) = held.clone() {
        send(&handle, &clients[1], ClientMessage::AutoSkip { rule }).await;
        assert_eq!(told(&mut clients[1]).await, vec![held.unwrap()]);
    }
    clients[1].drain();
    // Turning Full Control off (or keeping it) and passing lets the trigger resolve.
    send(&handle, &clients[1], ClientMessage::FullControl { mode }).await;
    if mode != FullControlMode::Off {
        let (id, _) = clients[1].decision().await;
        reply(
            &handle,
            &clients[1],
            id,
            Answer::Priority(PriorityAction::Pass),
        )
        .await;
    }
    quiet(&handle).await;
    let sent = clients[1].drain();
    assert_eq!(sent_kind(&sent, "yes-no"), asked, "{sent:?}");
    if !asked {
        let logged = store.load(game_id(1)).await.unwrap();
        let yes = matches!(rule, Rule::Yes);
        assert!(
            logged.iter().any(|e| matches!(
                e,
                Event::ChoiceMade { player, choice: Choice::YesNo(said) } if *player == seat(1) && *said == yes
            )),
            "the table answered {yes} for seat 1 and logged it"
        );
    }
}

/// The key of the ability on top of the stack as seat `client` reads it, if the stack holds one.
async fn top_key(handle: &GameHandle, client: &mut Client) -> Option<AutoYield> {
    client.drain();
    send(handle, client, ClientMessage::Resync).await;
    client
        .until(|message| match message {
            ServerMessage::Snapshot { state, .. } => Some(state.clone()),
            _ => None,
        })
        .await
        .and_then(|state| {
            let top = *state.table.stack.cards.last()?;
            let object = state.objects.iter().find(|object| object.id == top)?;
            Some(AutoYield {
                name: object.name.clone(),
                text: object.rules_text.clone().unwrap_or_default(),
            })
        })
}

/// The key of a watcher's trigger.
fn trigger(name: &str) -> TriggerKey {
    TriggerKey {
        name: name.to_owned(),
        ability: 0,
    }
}

#[rstest]
#[case::reversed(Some(vec!["Second", "First"]), FullControlMode::Off, Some(vec![1, 0]))]
#[case::as_triggered(Some(vec!["First", "Second"]), FullControlMode::Off, Some(vec![0, 1]))]
#[case::no_rule_asks(None, FullControlMode::Off, None)]
#[case::another_set_asks(Some(vec!["First", "Third"]), FullControlMode::Off, None)]
#[case::full_control_asks_anyway(Some(vec!["Second", "First"]), FullControlMode::Held, None)]
#[tokio::test(start_paused = true)]
async fn an_order_rule_orders_exactly_its_set_of_triggers_out_of_full_control(
    #[case] rule: Option<Vec<&str>>,
    #[case] mode: FullControlMode,
    #[case] order: Option<Vec<usize>>,
) {
    let first = watcher("First", when::any_creature_dies(), vec![gain_life(1)]);
    let second = watcher("Second", when::any_creature_dies(), vec![gain_life(10)]);
    let setup = burn_table(vec![first, second]);
    let victim = object_named(&setup, "Victim");
    let (handle, store) = spawn(setup);
    let mut clients = join(&handle, mode).await;
    if let Some(names) = &rule {
        let rule = AutoSkip::Order {
            triggers: names.iter().map(|name| trigger(name)).collect(),
        };
        send(&handle, &clients[1], ClientMessage::AutoSkip { rule }).await;
        assert_eq!(told(&mut clients[1]).await.len(), 1);
    }
    ready_all(&handle, &clients).await;
    burn_the_victim(&handle, &mut clients, victim).await;
    let Some(order) = order else {
        loop {
            let (id, decision) = clients[1].decision().await;
            match decision.kind {
                DecisionKind::OrderTriggers(options) => {
                    let keys: Vec<_> = options.triggers.iter().map(|t| t.ability).collect();
                    assert_eq!(keys, vec![0, 0], "a watcher's trigger is its rule 0");
                    return;
                }
                DecisionKind::Priority(_) => {
                    let pass = Answer::Priority(PriorityAction::Pass);
                    reply(&handle, &clients[1], id, pass).await;
                }
                other => panic!("seat 1 orders its triggers, got {other:?}"),
            }
        }
    };
    quiet(&handle).await;
    assert!(!sent_kind(&clients[1].drain(), "order-triggers"));
    let logged = store.load(game_id(1)).await.unwrap();
    assert!(
        logged.iter().any(|e| matches!(
            e,
            Event::TriggersOrdered { player, order: put } if *player == seat(1) && *put == order
        )),
        "the triggers went on in the rule's order {order:?}"
    );
}

/// Seat 0 holds three Pingers (one key between them) and a free instant; the last seat holds a free instant, so an
/// object of seat 0 on the stack is a default window it is asked at; the seats between hold nothing and are passed.
fn pinger_table(seats: usize) -> GameSetup {
    let pingers = [1, 2, 3].map(|_| ready(pinger("Pinger", 1)));
    Scenario::new(seats)
        .battlefield(0, pingers)
        .hand(0, [instant("Foe Zap", 0, &[])])
        .hand(seats - 1, [instant("My Zap", 0, &[])])
        .setup()
        .unwrap()
}

/// Seat 0 plays the first ability or spell it is offered (`spell` picks which), aimed at itself, and passes on it.
async fn foe_plays(handle: &GameHandle, foe: &mut Client, spell: bool) {
    let (id, decision) = foe.decision().await;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("expected priority, got {decision:?}");
    };
    let action = if spell {
        PriorityAction::CastSpell(options.castable[0])
    } else {
        PriorityAction::Activate(options.abilities[0])
    };
    reply(handle, foe, id, Answer::Priority(action)).await;
    loop {
        let (id, decision) = foe.decision().await;
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![Target::Player(seat(0))]))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            other => panic!("unexpected {}", other.name()),
        };
        let pass = matches!(answer, Answer::Priority(_));
        reply(handle, foe, id, answer).await;
        if pass {
            return;
        }
    }
}

/// The ability on top of the stack as `client` sees it.
async fn top_of_stack(handle: &GameHandle, client: &mut Client) -> ObjectId {
    client.drain();
    send(handle, client, ClientMessage::Resync).await;
    client
        .until(|message| match message {
            ServerMessage::Snapshot { state, .. } => state.table.stack.cards.last().copied(),
            _ => None,
        })
        .await
}

#[rstest]
#[case::two_seats(2, false)]
#[case::three_seats(3, false)]
#[case::four_seats(4, false)]
#[case::full_control_overrides(3, true)]
#[tokio::test(start_paused = true)]
async fn a_yield_passes_only_while_its_ability_is_on_top_and_an_opponents_spell_still_stops_the_seat(
    #[case] seats: usize,
    #[case] full_control: bool,
) {
    let (handle, _) = spawn(pinger_table(seats));
    let me = seats - 1;
    let mut clients = Vec::new();
    for n in 0..seats {
        clients.push(Client::join(&handle, n).await);
    }
    let held = FullControlMode::Held;
    send(
        &handle,
        &clients[0],
        ClientMessage::FullControl { mode: held },
    )
    .await;
    ready_all(&handle, &clients).await;
    foe_plays(&handle, &mut clients[0], false).await;
    assert_eq!(
        waiting(&handle).await,
        Some(seat(me)),
        "asked at the first Pinger"
    );
    let object = top_of_stack(&handle, &mut clients[me]).await;
    send(&handle, &clients[me], ClientMessage::AutoYield { object }).await;
    assert_eq!(waiting(&handle).await, Some(seat(0)), "passed at once");
    if full_control {
        send(
            &handle,
            &clients[me],
            ClientMessage::FullControl { mode: held },
        )
        .await;
    }
    clients[me].drain();
    foe_plays(&handle, &mut clients[0], false).await;
    assert_eq!(
        waiting(&handle).await == Some(seat(me)),
        full_control,
        "the second Pinger asks only in Full Control"
    );
    if full_control {
        return;
    }
    assert!(!sent_kind(&clients[me].drain(), "priority"));
    foe_plays(&handle, &mut clients[0], true).await;
    assert_eq!(
        waiting(&handle).await,
        Some(seat(me)),
        "the spell stops the seat"
    );
    let (_, decision) = clients[me].decision().await;
    assert!(matches!(
        decision,
        Decision {
            kind: DecisionKind::Priority(_),
            ..
        }
    ));
}

#[tokio::test(start_paused = true)]
async fn the_rules_end_with_the_game_and_bad_ones_are_refused() {
    let (handle, _) = spawn(burn_table(vec![owl()]));
    let mut clients = join(&handle, FullControlMode::Off).await;
    let bad = AutoSkip::Order {
        triggers: vec![trigger("First")],
    };
    clients[1].drain();
    send(&handle, &clients[1], ClientMessage::AutoSkip { rule: bad }).await;
    assert!(matches!(
        clients[1].drain().as_slice(),
        [ServerMessage::Error {
            code: ErrorCode::BadMessage,
            ..
        }]
    ));
    let rule = AutoSkip::Answer {
        ability: AutoYield {
            name: "Owl".into(),
            text: String::new(),
        },
        yes: true,
    };
    send(
        &handle,
        &clients[1],
        ClientMessage::AutoSkip { rule: rule.clone() },
    )
    .await;
    assert_eq!(told(&mut clients[1]).await, vec![rule.clone()]);
    // The same key with the other answer replaces it; cancelling matches by key.
    let no = AutoSkip::Answer {
        ability: AutoYield {
            name: "Owl".into(),
            text: String::new(),
        },
        yes: false,
    };
    send(
        &handle,
        &clients[1],
        ClientMessage::AutoSkip { rule: no.clone() },
    )
    .await;
    assert_eq!(told(&mut clients[1]).await, vec![no]);
    send(
        &handle,
        &clients[1],
        ClientMessage::CancelAutoSkip { rule: rule.clone() },
    )
    .await;
    assert_eq!(told(&mut clients[1]).await, Vec::new());
    send(
        &handle,
        &clients[1],
        ClientMessage::AutoSkip { rule: rule.clone() },
    )
    .await;
    told(&mut clients[1]).await;
    ready_all(&handle, &clients).await;
    send(&handle, &clients[0], ClientMessage::Concede).await;
    let rules = clients[1]
        .until(|message| match message {
            ServerMessage::AutoSkips { rules } => Some(Some(rules.clone())),
            ServerMessage::GameOver { .. } => Some(None),
            _ => None,
        })
        .await;
    assert_eq!(rules, Some(Vec::new()), "told before the game is over");
    clients[1].drain();
    send(&handle, &clients[1], ClientMessage::AutoSkip { rule }).await;
    assert!(matches!(
        clients[1].drain().as_slice(),
        [ServerMessage::Error {
            code: ErrorCode::BadMessage,
            ..
        }]
    ));
}
