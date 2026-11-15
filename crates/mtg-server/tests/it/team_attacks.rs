//! Goal: at a Two-Headed Giant table made through the lobby both players of the active team receive their own attack question in the same declare-attackers step, in turn order, the second seeing what the first chose; one player declining never keeps the teammate's creatures at home, a bot teammate answers for itself while the human is still asked, and the team's one declaration is exactly the players' choices together (CR 805.10).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::time::Duration;

use crate::common::{Client, windowed};
use crate::team_sickness::plays;
use mtg_core::house::HouseRules;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::Answer;
use mtg_engine::{Decision, DecisionKind, Event};
use mtg_protocol::events::EventView;
use mtg_protocol::{ClientMessage, EventBatch, ServerMessage, StateView};
use mtg_server::bots::{BotLevel, BotSeat};
use mtg_server::config::ServerConfig;
use mtg_server::demo;
use mtg_server::registry::{GameOptions, Registry, SeatRequest};
use rstest::rstest;

/// A human's attack question: who was asked, what it offered and the attacks it saw the teammates choose before.
type Question = (PlayerId, Vec<ObjectId>, Vec<ObjectId>);

/// What the table showed of one team turn's combat.
#[derive(Default)]
struct Turn {
    /// The questions the human seats were asked, in order.
    asked: Vec<Question>,
    /// The attacks each player of the team chose before the last, in turn order; the last player's choice is part of
    /// the declaration only.
    chosen: Vec<(PlayerId, Vec<ObjectId>)>,
    /// The team's declarations.
    declared: Vec<Vec<ObjectId>>,
    /// The active player, who declared.
    active: Option<PlayerId>,
    /// Whether the team's choice was refused at some point.
    refused: bool,
}

/// Notes the attacks the events of `batch` show, each in the turn it happened in: a batch runs on to the next decision,
/// which can be a turn later than the attack.
fn note(turns: &mut BTreeMap<u32, Turn>, current: &mut u32, batch: &EventBatch) {
    let ids = |list: &[(ObjectId, mtg_engine::combat::AttackTarget)]| -> Vec<ObjectId> {
        list.iter().map(|(object, _)| *object).collect()
    };
    for event in &batch.events {
        let EventView::Public(event) = event else {
            continue;
        };
        let turn = turns.entry(*current).or_default();
        match event {
            Event::TurnBegan { number, .. } => *current = *number,
            Event::AttackersChosen {
                player, attackers, ..
            } => turn.chosen.push((*player, ids(attackers))),
            Event::AttackChoicesRefused { .. } => turn.refused = true,
            Event::AttackersDeclared { player, attackers } => {
                turn.active = Some(*player);
                turn.declared.push(ids(attackers));
            }
            _ => {}
        }
    }
}

/// A human's answer: a declaration of attackers is noted in `turn` and, from a seat that is `declining`, empty; the
/// seat attacks with every creature it is offered otherwise.
fn respond(
    turn: &mut Turn,
    decision: &Decision,
    declining: bool,
    view: Option<&StateView>,
) -> Answer {
    let options = match &decision.kind {
        DecisionKind::DeclareAttackers(options) => options,
        // An optional trigger of a creature the human cast: declined.
        DecisionKind::YesNo(_) => return Answer::YesNo(YesNoChoice(false)),
        _ => return plays(decision, view),
    };
    let party = options
        .party
        .as_ref()
        .expect("a team's attack question is a shared one");
    turn.asked.push((
        decision.player,
        options.attackers.iter().map(|each| each.object).collect(),
        party
            .declared
            .iter()
            .map(|(_, object, _)| *object)
            .collect(),
    ));
    if declining {
        return Answer::DeclareAttackers(Vec::new());
    }
    Answer::DeclareAttackers(
        options
            .attackers
            .iter()
            .map(|each| (each.object, each.defenders[0]))
            .collect(),
    )
}

/// Plays a Two-Headed Giant game with human seats `humans` (the rest are bots) until turn 40 and returns what the table
/// showed of each turn's combat; the humans in `declining` never attack.
async fn play(humans: &[usize], declining: &[usize]) -> BTreeMap<u32, Turn> {
    let seats = 4;
    let registry = Registry::new(&ServerConfig {
        game: windowed(),
        bot_delay: Duration::from_millis(50),
        ..ServerConfig::default()
    });
    let requests: Vec<SeatRequest> = (0..seats)
        .map(|n| SeatRequest {
            name: format!("Player {n}"),
            deck: &demo::all()[n % demo::all().len()],
        })
        .collect();
    let options = GameOptions {
        house_rules: HouseRules {
            teams: Some(Teams::even(TeamMode::TwoHeadedGiant, 2, 2)),
            ..HouseRules::default()
        },
        bots: (0..seats)
            .filter(|seat| !humans.contains(seat))
            .map(|seat| BotSeat {
                seat: PlayerId::from_index(seat),
                level: BotLevel::Easy,
            })
            .collect(),
        ..GameOptions::default()
    };
    let created = registry
        .create_with(&requests, options, None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    let mut clients = Vec::new();
    for seat in humans {
        clients.push(Client::join(&handle, *seat).await);
    }
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    let mut views: Vec<Option<StateView>> = clients.iter().map(|_| None).collect();
    // The turn each client's last batch named.
    let mut at: Vec<u32> = clients.iter().map(|_| 0).collect();
    let mut turns: BTreeMap<u32, Turn> = BTreeMap::new();
    let mut current = 1;
    let ended = tokio::time::timeout(Duration::from_secs(6 * 60 * 60), async {
        loop {
            let mut progressed = false;
            for (index, client) in clients.iter_mut().enumerate() {
                let Ok(message) = client.inbox.try_recv() else {
                    continue;
                };
                progressed = true;
                match message {
                    ServerMessage::Snapshot { state, .. } => views[index] = Some(*state),
                    ServerMessage::Events(batch) => {
                        match views[index].as_mut() {
                            Some(view) => view.apply(&batch),
                            None => views[index] = StateView::from_first_batch(&batch),
                        }
                        at[index] = batch.table.turn.number;
                        if index == 0 {
                            note(&mut turns, &mut current, &batch);
                        }
                    }
                    ServerMessage::Decision { id, decision, .. } => {
                        if at[index] > 40 {
                            return;
                        }
                        let answer = respond(
                            turns.entry(at[index]).or_default(),
                            &decision,
                            declining.contains(&client.seat.index()),
                            views[index].as_ref(),
                        );
                        let message = ClientMessage::Answer {
                            decision: id,
                            answer,
                        };
                        client.say(&handle, message).await;
                    }
                    ServerMessage::GameOver { .. } => return,
                    _ => {}
                }
            }
            if !progressed {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    })
    .await;
    assert!(ended.is_ok(), "the team game stalled");
    turns
}

#[rstest]
#[case::humans_the_first_head_declines(&[0, 1, 2, 3], &[0, 2])]
#[case::humans_the_second_head_declines(&[0, 1, 2, 3], &[1, 3])]
#[case::a_human_and_a_bot_teammate(&[0, 2], &[])]
#[tokio::test(start_paused = true)]
async fn both_players_of_a_giant_team_choose_their_own_attacks(
    #[case] humans: &[usize],
    #[case] declining: &[usize],
) {
    let teams = Teams::even(TeamMode::TwoHeadedGiant, 2, 2);
    let turns = play(humans, declining).await;
    // Turns where both players of the team were asked, and where one declined and the other still attacked.
    let (mut both, mut kept, mut questions) = (0, 0, 0);
    for (number, turn) in &turns {
        if turn.declared.is_empty() || turn.refused {
            continue;
        }
        // The team's players choose in turn order from the active player, who need not sit first.
        let active = turn.active.unwrap();
        let mut team: Vec<PlayerId> = teams.members(teams.team_of(active)).collect();
        team.sort_by_key(|player| (player.index() + 4 - active.index()) % 4);
        let chose: Vec<PlayerId> = turn.chosen.iter().map(|(player, _)| *player).collect();
        // The last player's choice is the declaration itself, so no `AttackersChosen` shows it.
        assert_eq!(
            chose,
            team[..team.len() - 1],
            "turn {number}: each player but the last chose, in order"
        );
        assert_eq!(turn.declared.len(), 1, "turn {number}: one declaration");
        let earlier: Vec<ObjectId> = turn
            .chosen
            .iter()
            .flat_map(|(_, attackers)| attackers.iter().copied())
            .collect();
        assert!(
            turn.declared[0].starts_with(&earlier),
            "turn {number}: the declaration holds every earlier choice"
        );
        // A player with no creature to attack with is not asked; the others see the choices before theirs.
        for (player, offered, seen) in &turn.asked {
            assert!(
                !offered.is_empty(),
                "turn {number}: {player:?} has nothing to attack with"
            );
            let place = team.iter().position(|member| member == player).unwrap();
            let before: Vec<ObjectId> = turn.chosen[..place]
                .iter()
                .flat_map(|(_, attackers)| attackers.iter().copied())
                .collect();
            assert_eq!(
                seen, &before,
                "turn {number}: {player:?} sees the choices before theirs"
            );
            questions += 1;
        }
        both += usize::from(turn.asked.len() == team.len());
        let declined = turn
            .asked
            .iter()
            .any(|(player, ..)| declining.contains(&player.index()));
        kept += usize::from(declined && !turn.declared[0].is_empty());
    }
    assert!(questions >= 2, "too few attack questions: {questions}");
    if humans.len() == 4 {
        assert!(both >= 1, "no turn asked both players of a team");
    }
    if !declining.is_empty() {
        assert!(
            kept >= 1,
            "no turn had one player decline while the other attacked"
        );
    }
}
