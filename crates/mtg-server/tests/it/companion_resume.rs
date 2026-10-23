//! Goal: a game with a companion at every seat that is rebuilt after a server restart keeps the companion at its stage at two, three and four seats: a lobby tells no seat any companion and the opening reveals them all once the table is ready, a revealed companion stays outside the game, and a brought one stays in its owner's hand as the object the log names; the resumed game has the state and the log of the straight one and every seat's snapshot names the stage.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, game_id, roomy};
use mtg_engine::companions::CompanionStage;
use mtg_engine::decision::{Answer, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::{Engine, Event, GameSetup, replay};
use mtg_protocol::{ClientMessage, CompanionView, ServerMessage};
use mtg_server::{EventStore, GameHandle, MemoryStore, resume_game_with};
use mtg_testkit::cards::forest;
use mtg_testkit::companion_cards::{companion_fuzz_setup, small_companion};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const SEED: u64 = 11;
const NAME: &str = "Small Companion";

#[derive(Debug, Clone, Copy)]
enum Stage {
    /// The opening is done and nobody has paid for a companion.
    Revealed,
    /// Seat 0 paid {3} and the companion is a card in its hand.
    Brought,
}

/// The setup and the log of a game stopped at `stage`.
fn stopped(stage: Stage, seats: usize) -> (GameSetup, Vec<Event>) {
    match stage {
        Stage::Revealed => {
            let setup = companion_fuzz_setup(seats, SEED);
            let log = Engine::new(setup.clone()).unwrap().events().to_vec();
            (setup, log)
        }
        Stage::Brought => {
            let mut scenario = Scenario::new(seats)
                .at(1, 0, Step::Main1)
                .battlefield(0, (0..3).map(|_| ready(forest())).collect::<Vec<_>>());
            for seat in 0..seats {
                scenario = scenario.companion(seat, small_companion());
            }
            let mut driver = Driver::from_scenario(scenario).unwrap();
            driver
                .answer(Answer::Priority(PriorityAction::BringCompanion))
                .unwrap();
            driver
                .pay_tapping(&["Forest", "Forest", "Forest"], 0)
                .unwrap();
            (
                driver.engine().setup().clone(),
                driver.engine().events().to_vec(),
            )
        }
    }
}

/// The first snapshot `client` is sent: the companion of each seat as that client sees it.
async fn companions_in(client: &mut Client, seats: usize) -> Vec<CompanionView> {
    let state = client
        .until(|message| match message {
            ServerMessage::Snapshot { state, .. } => Some(state.clone()),
            _ => None,
        })
        .await;
    assert_eq!(state.table.players.len(), seats);
    state
        .table
        .players
        .iter()
        .map(|player| player.companion.clone().expect("every seat named one"))
        .collect()
}

#[rstest]
#[tokio::test]
async fn a_companion_stays_at_the_stage_the_log_reached_after_a_restart(
    #[values(Stage::Revealed, Stage::Brought)] stage: Stage,
    #[values(2, 3, 4)] seats: usize,
) {
    let (setup, log) = stopped(stage, seats);
    let id = game_id(40);
    let store = MemoryStore::new();
    store.append(id, 0, &log).await.unwrap();

    let handle = resume_game_with(store.clone(), id, setup.clone(), roomy())
        .await
        .unwrap();
    let status = handle.status().await.unwrap();
    let straight = replay(&setup, &log).unwrap();
    assert!(status.started);
    assert_eq!(status.state_hash, Some(straight.state_hash()), "{stage:?}");
    assert_eq!(store.load(id).await.unwrap(), log, "nothing was missing");

    for viewer in 0..seats {
        let mut client = Client::join(&handle, viewer).await;
        let companions = companions_in(&mut client, seats).await;
        for (owner, companion) in companions.iter().enumerate() {
            let expected = match stage {
                Stage::Brought if owner == 0 => CompanionStage::Brought,
                _ => CompanionStage::Revealed,
            };
            let at = format!("{stage:?} seat {owner} seen by {viewer}");
            assert_eq!(companion.stage, expected, "{at}");
            assert_eq!(companion.name.as_deref(), Some(NAME), "{at}");
            let card = straight
                .companion(common::seat(owner))
                .map(|info| info.card);
            assert_eq!(
                companion.object,
                card.filter(|_| expected == CompanionStage::Brought),
                "{at}"
            );
        }
    }
}

/// Everything the server sent to `client` so far, as text.
fn text_of(client: &mut Client) -> String {
    client
        .drain()
        .iter()
        .map(|message| serde_json::to_string(message).unwrap())
        .collect()
}

#[rstest]
#[tokio::test]
async fn a_lobby_that_restarts_tells_no_seat_a_companion_until_the_opening_reveals_them(
    #[values(2, 3, 4)] seats: usize,
) {
    let setup = companion_fuzz_setup(seats, SEED);
    let id = game_id(41);
    let store = MemoryStore::new();
    let handle: GameHandle = resume_game_with(store.clone(), id, setup.clone(), roomy())
        .await
        .unwrap();
    assert!(!handle.status().await.unwrap().started, "nothing is stored");

    let mut clients = Vec::new();
    for seat in 0..seats {
        clients.push(Client::join(&handle, seat).await);
    }
    for client in &mut clients {
        let lobby = text_of(client);
        assert!(lobby.to_lowercase().contains("lobby"), "a roster: {lobby}");
        assert!(!lobby.contains(NAME), "no seat is told a card in a lobby");
    }
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    assert!(handle.status().await.unwrap().started);
    let straight = Engine::new(setup.clone()).unwrap();
    assert_eq!(store.load(id).await.unwrap(), straight.events());
    let revealed = store
        .load(id)
        .await
        .unwrap()
        .iter()
        .filter(|event| matches!(event, Event::CompanionRevealed { .. }))
        .count();
    assert_eq!(revealed, seats, "the opening reveals every companion");

    for (seat, client) in clients.iter_mut().enumerate() {
        let seen = text_of(client);
        assert_eq!(seen.matches("CompanionRevealed").count(), seats, "{seat}");
        let mut again = Client::join(&handle, seat).await;
        let companions = companions_in(&mut again, seats).await;
        assert!(
            companions
                .iter()
                .all(|c| c.stage == CompanionStage::Revealed && c.name.as_deref() == Some(NAME))
        );
    }
}
