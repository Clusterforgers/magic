//! Goal: at a table of people, `GoBack` over other people's passes asks each of them (`GoBackAsked` to every seat, the asker offered the step whoever passed since); all agreeing cuts the stored log to the pass with `WentBack` last and a log that replays to the state of the pass, while a decline, silence past the wait or the asker's cancel ends it (`GoBackEnded`) and changes nothing, at two and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, game_id, poke, roomy, seat};
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Event, GameSetup, replay};
use mtg_protocol::{ClientMessage, DecisionId, ServerMessage, TakeBackOutcome};
use mtg_server::{EventStore, GameHandle, MemoryStore, spawn_game_with};
use mtg_testkit::Scenario;
use rstest::rstest;

/// How a seat asked answers: agrees, declines or stays silent.
#[derive(Debug, Clone, Copy)]
enum Vote {
    Allow,
    Decline,
    Silent,
}

/// A table of `seats` people, all joined and ready, every decision sent to them.
struct Table {
    setup: GameSetup,
    handle: GameHandle,
    store: MemoryStore,
    clients: Vec<Client>,
    /// The pending decision, if any, and whose it is.
    pending: Option<(usize, DecisionId)>,
}

impl Table {
    async fn new(seats: usize) -> Table {
        let setup = Scenario::new(seats).setup().unwrap();
        let store = MemoryStore::new();
        let handle = spawn_game_with(game_id(1), setup.clone(), store.clone(), roomy()).unwrap();
        let mut clients = Vec::new();
        for index in 0..seats {
            clients.push(Client::join(&handle, index).await);
        }
        for client in &clients {
            client.say(&handle, ClientMessage::Ready).await;
        }
        handle.status().await.unwrap();
        let mut table = Table {
            setup,
            handle,
            store,
            clients,
            pending: None,
        };
        let seen: Vec<_> = table.clients.iter_mut().map(Client::drain).collect();
        table.note(&seen);
        table
    }

    fn note(&mut self, seen: &[Vec<ServerMessage>]) {
        for (index, messages) in seen.iter().enumerate() {
            let found = messages.iter().rev().find_map(|m| match m {
                ServerMessage::Decision { id, decision, .. }
                    if matches!(decision.kind, DecisionKind::Priority(_)) =>
                {
                    Some(*id)
                }
                _ => None,
            });
            if let Some(id) = found {
                self.pending = Some((index, id));
            }
        }
    }

    async fn poke(&mut self, from: usize, message: ClientMessage) -> Vec<Vec<ServerMessage>> {
        let seen = poke(&self.handle, &mut self.clients, from, message).await;
        self.note(&seen);
        seen
    }

    /// The holder of priority passes.
    async fn pass(&mut self) -> usize {
        let (holder, id) = self.pending.take().expect("someone holds priority");
        let answer = Answer::Priority(PriorityAction::Pass);
        self.poke(
            holder,
            ClientMessage::Answer {
                decision: id,
                answer,
            },
        )
        .await;
        holder
    }

    async fn log(&self) -> Vec<Event> {
        self.store.load(game_id(1)).await.unwrap()
    }

    /// Every seat passes the precombat main phase, seat 0 first, so seat 0 is asked at the beginning of combat; the
    /// log before seat 0's pass.
    async fn pass_main(&mut self) -> Vec<Event> {
        let before = self.log().await;
        for expected in 0..self.clients.len() {
            assert_eq!(self.pass().await, expected);
        }
        assert_eq!(self.pending.map(|(holder, _)| holder), Some(0));
        before
    }
}

fn asked(messages: &[ServerMessage]) -> Option<Vec<PlayerId>> {
    messages.iter().rev().find_map(|m| match m {
        ServerMessage::GoBackAsked { offer, waiting, .. } => {
            assert_eq!((offer.player, offer.step), (seat(0), Step::Main1));
            Some(waiting.clone())
        }
        _ => None,
    })
}

fn ended(messages: &[ServerMessage]) -> Option<TakeBackOutcome> {
    messages.iter().find_map(|m| match m {
        ServerMessage::GoBackEnded { outcome, .. } => Some(outcome.clone()),
        _ => None,
    })
}

fn others(seats: usize) -> Vec<PlayerId> {
    (1..seats).map(seat).collect()
}

#[rstest]
#[case::two_seats_allow(2, vec![Vote::Allow], TakeBackOutcome::Approved)]
#[case::two_seats_decline(2, vec![Vote::Decline], TakeBackOutcome::Declined { by: seat(1) })]
#[case::four_seats_allow(4, vec![Vote::Allow; 3], TakeBackOutcome::Approved)]
#[case::four_seats_one_declines(4, vec![Vote::Allow, Vote::Decline, Vote::Allow], TakeBackOutcome::Declined { by: seat(2) })]
#[case::four_seats_one_silent(4, vec![Vote::Allow, Vote::Silent, Vote::Allow], TakeBackOutcome::TimedOut { by: seat(2) })]
#[tokio::test(start_paused = true)]
async fn every_person_who_passed_since_is_asked(
    #[case] seats: usize,
    #[case] votes: Vec<Vote>,
    #[case] outcome: TakeBackOutcome,
) {
    let mut table = Table::new(seats).await;
    let before = table.pass_main().await;
    let log = table.log().await;

    let open = table.poke(0, ClientMessage::GoBack).await;
    for seen in &open {
        assert_eq!(
            asked(seen),
            Some(others(seats)),
            "every seat is told who is asked"
        );
    }
    assert_eq!(
        table.log().await,
        log,
        "nothing changes while they are asked"
    );

    let mut last = Vec::new();
    for (index, vote) in votes.iter().enumerate() {
        let approve = match vote {
            Vote::Allow => true,
            Vote::Decline => false,
            Vote::Silent => continue,
        };
        last = table
            .poke(index + 1, ClientMessage::GoBackVote { approve })
            .await;
        if ended(&last[0]).is_some() {
            break;
        }
    }
    let mut heard = Vec::new();
    for (index, client) in table.clients.iter_mut().enumerate() {
        let found = match last.get(index).and_then(|seen| ended(seen)) {
            Some(found) => found,
            // Silence: the wait runs out.
            None => {
                client
                    .until(|m| match m {
                        ServerMessage::GoBackEnded { outcome, .. } => Some(outcome.clone()),
                        _ => None,
                    })
                    .await
            }
        };
        heard.push(found);
    }
    assert_eq!(
        heard,
        vec![outcome.clone(); seats],
        "every seat hears how it ended"
    );
    let seen = last;

    let stored = table.log().await;
    if outcome == TakeBackOutcome::Approved {
        assert_eq!(stored[..before.len()], before[..]);
        assert_eq!(stored.len(), before.len() + 1);
        assert!(
            matches!(stored.last(), Some(Event::WentBack { player, step: Step::Main1, .. }) if *player == seat(0))
        );
        assert_eq!(
            replay(&table.setup, &stored).unwrap().state_hash(),
            replay(&table.setup, &before).unwrap().state_hash(),
            "the stored log replays to the game at the pass"
        );
        assert!(
            seen[0]
                .iter()
                .any(|m| matches!(m, ServerMessage::Snapshot { rewound: true, .. })),
            "the asker gets a rewound snapshot"
        );
        assert_eq!(
            table.pending.map(|(holder, _)| holder),
            Some(0),
            "seat 0 is asked at its pass again"
        );
    } else {
        assert_eq!(stored, log, "a refused go-back changes nothing");
    }
}

#[tokio::test]
async fn the_asker_cancels_and_a_stray_vote_or_second_request_is_a_bad_message() {
    let mut table = Table::new(2).await;
    table.pass_main().await;
    let log = table.log().await;
    table.poke(0, ClientMessage::GoBack).await;

    let again = table.poke(0, ClientMessage::GoBack).await;
    assert!(
        again[0]
            .iter()
            .any(|m| matches!(m, ServerMessage::Error { .. }))
    );
    let stray = table
        .poke(0, ClientMessage::GoBackVote { approve: true })
        .await;
    assert!(
        stray[0]
            .iter()
            .any(|m| matches!(m, ServerMessage::Error { .. }))
    );
    let foreign = table.poke(1, ClientMessage::CancelGoBack).await;
    assert!(
        foreign[1]
            .iter()
            .any(|m| matches!(m, ServerMessage::Error { .. }))
    );

    let cancelled = table.poke(0, ClientMessage::CancelGoBack).await;
    for seen in &cancelled {
        assert_eq!(ended(seen), Some(TakeBackOutcome::Withdrawn));
    }
    assert_eq!(table.log().await, log);
}

#[tokio::test]
async fn the_asker_is_offered_the_step_whoever_passed_since() {
    let mut table = Table::new(2).await;
    let mut offers = Vec::new();
    for _ in 0..2 {
        let (holder, id) = table.pending.take().unwrap();
        let answer = Answer::Priority(PriorityAction::Pass);
        let seen = table
            .poke(
                holder,
                ClientMessage::Answer {
                    decision: id,
                    answer,
                },
            )
            .await;
        offers.extend(seen[0].iter().filter_map(|m| match m {
            ServerMessage::GoBackOffer { offer } => Some(offer.as_ref().map(|offer| offer.step)),
            _ => None,
        }));
    }
    assert_eq!(
        offers,
        [Some(Step::Main1)],
        "the other person's pass leaves the offer as it was"
    );
}
