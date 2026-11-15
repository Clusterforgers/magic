//! Goal: at a team table the server makes through the lobby, the view each human seat folds from its snapshot and batches says a creature is summoning sick exactly until its controller's own turn begins (CR 302.6), however the teams sit and whoever plays the turns. A land or other noncreature permanent is never shown sick.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use crate::common::{self, Client, windowed};
use mtg_core::card::CardType;
use mtg_core::house::HouseRules;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::decision::{Answer, Decision, PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_engine::{DecisionKind, Event};
use mtg_protocol::events::EventView;
use mtg_protocol::{ClientMessage, EventBatch, ServerMessage, StateView};
use mtg_server::bots::{BotLevel, BotSeat};
use mtg_server::config::ServerConfig;
use mtg_server::demo;
use mtg_server::registry::{GameOptions, Registry, SeatRequest};
use rstest::rstest;

/// A human who plays a land or a creature whenever it can and passes otherwise, so its seat has creatures to attack with.
fn plays(decision: &Decision, view: Option<&StateView>) -> Answer {
    match (&decision.kind, view) {
        (DecisionKind::Priority(options), Some(view)) => {
            let creature = options.castable.iter().find(|id| {
                view.objects
                    .iter()
                    .any(|object| object.id == **id && object.types.contains(&CardType::Creature))
            });
            match (options.playable_lands.first(), creature) {
                (Some(land), _) => Answer::Priority(PriorityAction::PlayLand(*land)),
                (None, Some(spell)) => Answer::Priority(PriorityAction::CastSpell(*spell)),
                (None, None) => Answer::Priority(PriorityAction::Pass),
            }
        }
        (DecisionKind::PayMana(_), _) => Answer::PayMana(PayChoice::Auto),
        _ => common::passive(decision),
    }
}

fn requests(seats: usize) -> Vec<SeatRequest> {
    (0..seats)
        .map(|n| SeatRequest {
            name: format!("Player {n}"),
            deck: &demo::all()[n % demo::all().len()],
        })
        .collect()
}

/// What one human seat knows of the turns so far and of when each permanent arrived.
struct Ledger {
    teams: Teams,
    /// The active player of each turn that began.
    turns: Vec<PlayerId>,
    /// For each permanent seen arriving, how many turns had begun then.
    arrived: BTreeMap<ObjectId, usize>,
    /// How many attackers the seat was offered.
    attackers: usize,
    /// How many held-back and how many free permanents the view was checked on.
    checked: (usize, usize),
}

impl Ledger {
    fn new(teams: Teams) -> Self {
        Ledger {
            teams,
            turns: Vec::new(),
            arrived: BTreeMap::new(),
            attackers: 0,
            checked: (0, 0),
        }
    }

    /// Whether a turn of `active` is a turn of `player`: their own, or in Two-Headed Giant their team's.
    fn is_turn_of(&self, active: PlayerId, player: PlayerId) -> bool {
        active == player
            || (self.teams.shares_turns()
                && self.teams.team_of(active) == self.teams.team_of(player))
    }

    /// Folds `batch` into `view` and checks what the view says against the turns the batch's events tell. A permanent
    /// that arrives in a batch with a turn boundary is left out, since no turn count says when it arrived.
    fn fold(&mut self, view: &mut StateView, batch: &EventBatch, seat: usize) {
        let on_field = |view: &StateView| -> BTreeSet<ObjectId> {
            view.objects
                .iter()
                .filter(|object| object.zone == Zone::Battlefield)
                .map(|object| object.id)
                .collect()
        };
        let before = on_field(view);
        let counted = self.turns.len();
        let mut turned = false;
        for event in &batch.events {
            if let EventView::Public(Event::TurnBegan { active, .. }) = event {
                self.turns.push(*active);
                turned = true;
            }
        }
        view.apply(batch);
        if !turned {
            for id in on_field(view).difference(&before) {
                self.arrived.insert(*id, counted);
            }
        }
        self.check(view, seat);
    }

    /// Checks every permanent of `view` that arrived under its owner's control and has no haste.
    fn check(&mut self, view: &StateView, seat: usize) {
        for object in &view.objects {
            let Some(permanent) = &object.permanent else {
                continue;
            };
            let Some(&since) = self.arrived.get(&object.id) else {
                continue;
            };
            if object.controller != object.owner
                || object.keywords.iter().any(|word| word == "Haste")
            {
                continue;
            }
            // Only a creature is ever held back (CR 302.6).
            let creature = object.types.contains(&CardType::Creature);
            let held = creature
                && !self.turns[since..]
                    .iter()
                    .any(|active| self.is_turn_of(*active, object.controller));
            assert_eq!(
                permanent.summoning_sick,
                held,
                "seat {seat}'s view of {} of {:?}, arrived after turn {since}, shown at turn {}",
                object.name,
                object.controller,
                self.turns.len()
            );
            if held {
                self.checked.0 += 1;
            } else {
                self.checked.1 += 1;
            }
        }
    }
}

#[rstest]
#[case::teams_apart_second_player(TeamMode::Teams, vec![0, 1, 0, 1], &[2])]
#[case::teams_apart_first_player(TeamMode::Teams, vec![0, 1, 0, 1], &[0])]
#[case::teams_side_by_side(TeamMode::Teams, vec![0, 0, 1, 1], &[1])]
#[case::teams_three_v_three(TeamMode::Teams, vec![0, 1, 0, 1, 0, 1], &[2, 4])]
#[case::giant_first_head(TeamMode::TwoHeadedGiant, vec![0, 0, 1, 1], &[0])]
#[case::giant_second_head(TeamMode::TwoHeadedGiant, vec![0, 0, 1, 1], &[1])]
#[case::shared_second_hero(TeamMode::SharedTurns, vec![0, 0, 0, 1], &[1])]
#[case::shared_heroes_of_four(TeamMode::SharedTurns, vec![0, 0, 0, 0, 1, 1], &[0, 2])]
#[tokio::test(start_paused = true)]
async fn a_humans_view_holds_a_permanent_back_until_its_controllers_turn_begins(
    #[case] mode: TeamMode,
    #[case] seats_teams: Vec<u32>,
    #[case] humans: &[usize],
) {
    let seats = seats_teams.len();
    let teams = Teams {
        mode,
        seats: seats_teams,
    };
    let registry = Registry::new(&ServerConfig {
        game: windowed(),
        bot_delay: Duration::from_millis(50),
        ..ServerConfig::default()
    });
    let options = GameOptions {
        house_rules: HouseRules {
            teams: Some(teams.clone()),
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
        .create_with(&requests(seats), options, None)
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
    let mut ledgers: Vec<Ledger> = clients.iter().map(|_| Ledger::new(teams.clone())).collect();
    let mut views: Vec<Option<StateView>> = clients.iter().map(|_| None).collect();
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
                        let seat = client.seat.index();
                        match views[index].as_mut() {
                            Some(view) => ledgers[index].fold(view, &batch, seat),
                            None => views[index] = StateView::from_first_batch(&batch),
                        }
                    }
                    ServerMessage::Decision { id, decision, .. } => {
                        if ledgers[index].turns.len() >= 60 {
                            return;
                        }
                        // The creatures the seat may attack with are the ones its own view shows free.
                        if let (DecisionKind::DeclareAttackers(options), Some(view)) =
                            (&decision.kind, &views[index])
                        {
                            for attacker in &options.attackers {
                                let object = view
                                    .objects
                                    .iter()
                                    .find(|object| object.id == attacker.object)
                                    .unwrap();
                                assert_eq!(
                                    teams.team_of(object.controller),
                                    teams.team_of(decision.player)
                                );
                                assert!(
                                    object.permanent.as_ref().is_some_and(|p| !p.summoning_sick),
                                    "seat {:?} may attack with {}, which its view shows summoning sick",
                                    decision.player,
                                    object.name
                                );
                                ledgers[index].attackers += 1;
                            }
                        }
                        let answer = plays(&decision, views[index].as_ref());
                        client
                            .say(
                                &handle,
                                ClientMessage::Answer {
                                    decision: id,
                                    answer,
                                },
                            )
                            .await;
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
    for (client, ledger) in clients.iter().zip(&ledgers) {
        assert!(
            ledger.checked.0 > 10 && ledger.checked.1 > 10,
            "too few permanents were checked: {:?}",
            ledger.checked
        );
        // In Two-Headed Giant only the team's first seat declares attackers.
        let declares = teams.members(teams.team_of(client.seat)).next() == Some(client.seat);
        assert!(
            ledger.attackers > 0 || (teams.shares_turns() && !declares),
            "seat {:?} was never offered an attacker",
            client.seat
        );
    }
}
