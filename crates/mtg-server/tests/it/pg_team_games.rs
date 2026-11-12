//! Goal: team games of Commander run through the server from the lobby to a team's result: every seat is told the teams in the lobby and at the table; in Two-Headed Giant (CR 810) the teams alternate shared turns, the team that starts skips its first draw, both teammates draw, play lands and act on their turn, share one life total and lose together; in a team game of separate turns (CR 808) each seat takes its own turn in seat order, only the active player draws and plays a land, every seat acts on its own turn, and a team wins once all its opponents have lost.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{net, pg_commander};

use std::collections::BTreeSet;

use mtg_core::ids::PlayerId;
use mtg_core::teams::Teams;
use mtg_engine::DecisionKind;
use mtg_engine::events::Event;
use mtg_engine::state::GameResult;
use mtg_engine::turn::Step;
use mtg_protocol::{ClientMessage, EventView, HiddenEvent, ServerMessage, TableView};
use mtg_server::{GameHandle, ServerConfig};
use mtg_testkit::RandomBot;
use net::{PASSWORD, TestServer, bot_config, register, windowed_config};
use pg_commander::{create_body, join, led_by, store};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::mpsc::Receiver;

const NAMES: [&str; 6] = ["ann", "bob", "cyd", "dee", "eve", "fay"];
const LEADERS: [&str; 3] = [
    "Questing Beast",
    "The Thing, Ben Grimm",
    "Rosheen Meanderer",
];

/// The pace of the web client: idle seats are passed for, no clocks.
fn config(seed: u64) -> ServerConfig {
    let mut config = windowed_config();
    config.fixed_seed = Some(seed);
    config.auth.limits.register_per_ip = 10_000;
    config.auth.limits.lobby_requests_per_ip = 10_000;
    config
}

/// A Commander lobby of signed-in players under `teams`, every seat joined with its own deck.
async fn team_table(server: &TestServer, teams: &Value) -> GameHandle {
    let seats = teams["seats"].as_array().unwrap().len();
    let mut decks = Vec::new();
    let mut players = Vec::new();
    for (index, name) in NAMES[..seats].iter().enumerate() {
        let browser = register(server.addr, name, PASSWORD).await;
        decks.push(store(server, &browser, &led_by(LEADERS[index % LEADERS.len()])).await);
        players.push(browser);
    }
    let mut seat_list = vec![json!({"name": "ann", "deck": decks[0]})];
    seat_list.resize(seats, json!({}));
    let mut body = create_body(&seat_list, 1);
    body["house_rules"] = json!({ "teams": teams, "starting_life": 8 });
    let (game, _) = players[0].create_game(server.addr, &body).await;
    for (who, deck) in players.iter().zip(&decks).skip(1) {
        let joined = join(server, who, &game, deck).await;
        assert_eq!(joined.status, 200, "{}", joined.text());
    }
    server.registry.handle(game.parse().unwrap()).unwrap()
}

/// A priority decision with something to do in a main phase.
#[derive(Debug, Clone, Copy)]
struct Acted {
    seat: usize,
    turn: u32,
    active: usize,
    land: bool,
}

/// What the seats of a team game saw from the lobby to the end.
#[derive(Debug, Default)]
struct Seen {
    lobby_teams: Vec<Option<Teams>>,
    table_teams: Vec<Option<Teams>>,
    /// Every turn as seat 0 heard it: number and active seat.
    turns: Vec<(u32, usize)>,
    /// Every draw in a draw step as seat 0 heard it: turn, active seat, who drew.
    draws: Vec<(u32, usize, usize)>,
    /// Every loss as seat 0 heard it: turn and seat.
    losses: Vec<(u32, usize)>,
    acted: Vec<Acted>,
    /// The tables seat 0 was sent once the game began.
    tables: Vec<TableView>,
    results: Vec<Option<GameResult>>,
    /// The turn, its active seat and whether its draw step runs, as seat 0 last heard.
    turn: u32,
    active: usize,
    drawing: bool,
}

impl Seen {
    /// Follows the turns, steps, draws and losses of a batch seat 0 heard.
    fn hear(&mut self, events: &[EventView]) {
        for event in events {
            match event {
                EventView::Public(Event::TurnBegan { number, active, .. }) => {
                    (self.turn, self.active) = (*number, active.index());
                    self.turns.push((self.turn, self.active));
                }
                EventView::Public(Event::PlayerLost { player, .. }) => {
                    self.losses.push((self.turn, player.index()));
                }
                EventView::Public(Event::StepBegan { step }) => self.drawing = *step == Step::Draw,
                EventView::Public(Event::CardDrawn { player, .. })
                | EventView::Hidden(HiddenEvent::CardDrawn { player })
                    if self.drawing =>
                {
                    self.draws.push((self.turn, self.active, player.index()));
                }
                _ => {}
            }
        }
    }
}

/// Readies every seat and answers every decision with a seeded random player until each seat is told the result.
async fn play_out(handle: &GameHandle, seats: usize, seed: u64) -> Seen {
    let mut inboxes: Vec<Receiver<ServerMessage>> = Vec::new();
    let mut brains = Vec::new();
    for seat in 0..seats {
        inboxes.push(handle.join(PlayerId::from_index(seat)).await.unwrap());
        brains.push(RandomBot::new(seed, seat as u64));
    }
    for seat in 0..seats {
        handle
            .send(PlayerId::from_index(seat), ClientMessage::Ready)
            .await
            .unwrap();
    }
    let mut story = Seen {
        lobby_teams: vec![None; seats],
        table_teams: vec![None; seats],
        results: vec![None; seats],
        ..Seen::default()
    };
    let mut tables: Vec<Option<TableView>> = vec![None; seats];
    let mut answers = 0usize;
    loop {
        handle.status().await.unwrap();
        let mut busy = false;
        for seat in 0..seats {
            while let Ok(message) = inboxes[seat].try_recv() {
                busy = true;
                match message {
                    ServerMessage::Lobby { teams, .. } => story.lobby_teams[seat] = teams,
                    ServerMessage::Events(batch) => {
                        if seat == 0 {
                            story.hear(&batch.events);
                            story.tables.push((*batch.table).clone());
                        }
                        story.table_teams[seat] = batch
                            .table
                            .house_rules
                            .as_ref()
                            .and_then(|rules| rules.teams.clone());
                        tables[seat] = Some(*batch.table);
                    }
                    ServerMessage::Decision { id, decision, .. } => {
                        assert_eq!(decision.player.index(), seat, "a decision for another seat");
                        if let (DecisionKind::Priority(options), Some(table)) =
                            (&decision.kind, &tables[seat])
                            && options.can_act
                            && matches!(table.turn.step, Step::Main1 | Step::Main2)
                        {
                            story.acted.push(Acted {
                                seat,
                                turn: table.turn.number,
                                active: table.turn.active.index(),
                                land: !options.playable_lands.is_empty(),
                            });
                        }
                        let answer = brains[seat].choose(&decision);
                        handle
                            .send(
                                PlayerId::from_index(seat),
                                ClientMessage::Answer {
                                    decision: id,
                                    answer,
                                },
                            )
                            .await
                            .unwrap();
                        answers += 1;
                        assert!(answers < 40_000, "the game never ends");
                    }
                    ServerMessage::GameOver { result } => story.results[seat] = Some(result),
                    _ => {}
                }
            }
        }
        if story.results.iter().all(Option::is_some) {
            return story;
        }
        if !busy {
            let status = handle.status().await.unwrap();
            panic!(
                "the table went quiet before the end, waiting on {:?} (turn {})",
                status.waiting_on, story.turn
            );
        }
    }
}

/// The checks every team game passes: the teams are shown, decisions reach their seat, and a team (or nobody) wins.
fn common_checks(story: &Seen, teams: &Teams) {
    for (seat, shown) in story.lobby_teams.iter().enumerate() {
        assert_eq!(
            shown.as_ref(),
            Some(teams),
            "seat {seat}: the lobby shows the teams"
        );
    }
    for (seat, shown) in story.table_teams.iter().enumerate() {
        assert_eq!(
            shown.as_ref(),
            Some(teams),
            "seat {seat}: the table shows the teams"
        );
    }
    let result = story.results[0].unwrap();
    assert!(
        story.results.iter().all(|r| *r == Some(result)),
        "one result for everyone: {:?}",
        story.results
    );
    assert!(
        matches!(result, GameResult::Team(t) if t < teams.count()) || result == GameResult::Draw,
        "a team game ends with a team or a draw, not {result:?}"
    );
    assert!(story.turns.len() > 2, "the game played turns");
}

/// Whether the seat has lost, as seat 0 last saw it.
fn lost(story: &Seen, seat: usize) -> bool {
    story
        .tables
        .last()
        .is_some_and(|table| table.players[seat].lost.is_some())
}

/// Seats that had lost before `turn` began.
fn gone_before(story: &Seen, turn: u32) -> BTreeSet<usize> {
    story
        .losses
        .iter()
        .filter(|(at, _)| *at < turn)
        .map(|(_, seat)| *seat)
        .collect()
}

#[sqlx::test(migrations = false)]
async fn two_headed_giant_teams_share_their_turn_life_and_loss_when_every_seat_answers(
    pool: PgPool,
) {
    let mut config = config(5);
    config.game = bot_config().game;
    two_headed_giant(pool, config).await;
}

#[sqlx::test(migrations = false)]
async fn two_headed_giant_teams_share_their_turn_life_and_loss_under_auto_pass(pool: PgPool) {
    two_headed_giant(pool, config(5)).await;
}

/// A Two-Headed Giant game (CR 810) played to its end, with its checks.
async fn two_headed_giant(pool: PgPool, config: ServerConfig) {
    let seed = config.fixed_seed.unwrap();
    let server = pg_commander::start_with(&pool, config).await;
    let teams = json!({"mode": "two_headed_giant", "seats": [0, 0, 1, 1]});
    let handle = team_table(&server, &teams).await;
    let teams: Teams = serde_json::from_value(teams).unwrap();
    let story = play_out(&handle, 4, seed).await;
    common_checks(&story, &teams);
    let team_of = |seat: usize| teams.team_of(PlayerId::from_index(seat));

    // Teammates sit side by side and share one total from the start (CR 810.4, 810.9).
    for table in &story.tables {
        for team in 0..2 {
            let lives: Vec<i32> = teams
                .members(team)
                .map(|seat| table.players[seat.index()].life)
                .collect();
            assert_eq!(lives[0], lives[1], "team {team} shares its life: {lives:?}");
        }
    }
    assert_eq!(story.tables[0].players[0].life, 8);

    // The teams alternate turns (CR 810.6).
    for pair in story.turns.windows(2) {
        assert_ne!(team_of(pair[0].1), team_of(pair[1].1), "{pair:?}");
    }
    // The team that starts skips its first draw (CR 103.8b); later, each living teammate draws (CR 810.5).
    let (first_turn, _) = story.turns[0];
    assert!(story.draws.iter().all(|(turn, _, _)| *turn != first_turn));
    for &(turn, active) in story.turns.iter().skip(1) {
        let gone = gone_before(&story, turn);
        let drew: BTreeSet<usize> = story
            .draws
            .iter()
            .filter(|(t, _, _)| *t == turn)
            .map(|(_, _, who)| *who)
            .collect();
        let expected: BTreeSet<usize> = teams
            .members(team_of(active))
            .map(PlayerId::index)
            .filter(|seat| !gone.contains(seat))
            .collect();
        assert_eq!(drew, expected, "turn {turn}: the active team draws");
    }
    // Both teammates act and play lands on their team's turn, never on the other team's (CR 810.2, 505.6b).
    for seat in 0..4 {
        assert!(
            story.acted.iter().any(|a| a.seat == seat && a.land),
            "seat {seat} never got to play a land"
        );
    }
    for acted in &story.acted {
        if acted.land {
            assert_eq!(
                team_of(acted.seat),
                team_of(acted.active),
                "seat {} was offered a land on the other team's turn {}",
                acted.seat,
                acted.turn
            );
        }
    }
    // A team loses as one (CR 810.8a).
    for team in 0..2 {
        let members: Vec<bool> = teams
            .members(team)
            .map(|s| lost(&story, s.index()))
            .collect();
        assert_eq!(members[0], members[1], "team {team} lost together");
    }
}

#[sqlx::test(migrations = false)]
async fn two_against_two_take_separate_turns_and_a_team_wins(pool: PgPool) {
    separate_turns(pool, json!({"mode": "teams", "seats": [0, 1, 0, 1]}), 6).await;
}

#[sqlx::test(migrations = false)]
async fn three_against_three_take_separate_turns_and_a_team_wins(pool: PgPool) {
    separate_turns(
        pool,
        json!({"mode": "teams", "seats": [0, 1, 0, 1, 0, 1]}),
        7,
    )
    .await;
}

/// A team game of separate turns (CR 808) played to its end, with its checks.
async fn separate_turns(pool: PgPool, teams: Value, seed: u64) {
    let server = pg_commander::start_with(&pool, config(seed)).await;
    let handle = team_table(&server, &teams).await;
    let teams: Teams = serde_json::from_value(teams).unwrap();
    let seats = teams.seats.len();
    let story = play_out(&handle, seats, seed).await;
    common_checks(&story, &teams);

    // Every seat takes its own turn in seat order, past the seats that have lost.
    for pair in story.turns.windows(2) {
        let ((turn, from), (_, to)) = (pair[0], pair[1]);
        let gone = gone_before(&story, turn + 1);
        let next = (1..=seats)
            .map(|step| (from + step) % seats)
            .find(|seat| !gone.contains(seat))
            .unwrap();
        assert_eq!(to, next, "after turn {turn} of seat {from}");
    }
    // Only the active player draws (CR 504.1), and only on their own turn.
    for (turn, active, who) in &story.draws {
        assert_eq!(
            who, active,
            "seat {who} drew on turn {turn} of seat {active}"
        );
    }
    // Every seat acts and plays lands on its own turn, and is never offered a land on a teammate's turn.
    for seat in 0..seats {
        let own: Vec<u32> = story
            .turns
            .iter()
            .filter(|(_, active)| *active == seat)
            .map(|(turn, _)| *turn)
            .collect();
        if own.is_empty() {
            continue;
        }
        assert!(
            story
                .acted
                .iter()
                .any(|a| a.seat == seat && a.active == seat),
            "seat {seat} never acted on its own turns {own:?}"
        );
    }
    for acted in &story.acted {
        if acted.land {
            assert_eq!(
                acted.seat, acted.active,
                "seat {} was offered a land on turn {} of seat {}",
                acted.seat, acted.turn, acted.active
            );
        }
    }
    // A team that won has no opponent left (CR 808.6).
    if let Some(GameResult::Team(winner)) = story.results[0] {
        let alive_opponents: Vec<usize> = (0..seats)
            .filter(|&seat| teams.team_of(PlayerId::from_index(seat)) != winner)
            .filter(|&seat| !lost(&story, seat))
            .collect();
        assert!(
            alive_opponents.is_empty(),
            "team {winner} won with opponents {alive_opponents:?} left"
        );
    }
}
