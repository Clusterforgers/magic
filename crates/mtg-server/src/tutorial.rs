//! The tutorial table: a fixed Commander game against a scripted bot, made by one request, never recorded.
//!
//! The setup skips the opening: the human (seat 0) starts in their first main phase with a known hand and library, the
//! bot (seat 1) with a Swamp and a Dregscape Zombie to cast. Every card is a real card of the corpus. The player holds
//! the stops of [`PLAYER_STOPS`]; the bot plays the first land and the first spell it is offered, attacks with
//! everything it can and never blocks, so the lesson in `web/src/features/learn` meets the same decisions every time.
//! No clock hurries either seat.

use std::sync::{Arc, OnceLock};

use axum::Json;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use mtg_cards::{CardDefinition, builtin};
use mtg_core::ids::PlayerId;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{PayChoice, PriorityAction};
use mtg_engine::setup::{GameSetup, PlayerSetup, PresetPermanent, Start};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Decision, DecisionKind};
use mtg_import::scryfall::{CardRecord, cards};
use mtg_protocol::{ClientMessage, ServerMessage, StopSide};
use serde::Serialize;
use tokio::task::JoinHandle;

use crate::api::ApiError;
use crate::handle::GameHandle;
use crate::recipe::FormatKind;
use crate::server::AppState;

/// Scryfall objects (trimmed) of the tutorial's cards, each a printing of a Verified corpus card.
const SCRYFALL: &str = include_str!("tutorial/scryfall.json");

/// The seat the player sits in.
pub const PLAYER_SEAT: PlayerId = PlayerId(0);
/// The seat the scripted bot plays.
pub const BOT_SEAT: PlayerId = PlayerId(1);
/// The bot's name at the table.
pub const BOT_NAME: &str = "Practice bot";
/// The bot's starting life: two hits of the lesson's attacks win the game.
pub const BOT_LIFE: i32 = 6;
/// The player's starting life.
pub const PLAYER_LIFE: i32 = 20;
/// The steps of the player's own turns where the player is given priority whether they can act or not: the web's
/// default stops, held from the start so the lesson's decisions never depend on saved preferences.
pub const PLAYER_STOPS: [Step; 2] = [Step::DeclareAttackers, Step::Main2];
/// The seed; the setup skips every shuffle, so it only fixes what a card's own randomness does.
const SEED: u64 = 1;

/// Why the tutorial's cards could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TutorialError {
    /// A card of the lesson is missing from the embedded data.
    #[error("the tutorial card {0:?} is missing")]
    MissingCard(&'static str),
}

/// The tutorial's cards by name, as this engine runs them.
fn card(name: &'static str) -> Result<Arc<CardDefinition>, TutorialError> {
    static CARDS: OnceLock<Vec<(String, Arc<CardDefinition>)>> = OnceLock::new();
    let all = CARDS.get_or_init(|| {
        let (Ok(rules), caps) = (builtin::library(), mtg_engine::caps()) else {
            return Vec::new();
        };
        cards(SCRYFALL.as_bytes())
            .filter_map(Result::ok)
            .map(|record: CardRecord| {
                let printed = crate::demo::printed_card(&record);
                let assembled = rules.assemble(&printed, &caps);
                let printing = record.printing_id.to_string();
                let identified =
                    crate::cards::identified(assembled.definition, &record.oracle_id.to_string());
                let definition = crate::cards::reprinted(&identified, Some(&printing));
                (record.name, Arc::new(definition))
            })
            .collect()
    });
    all.iter()
        .find(|(known, _)| known == name)
        .map(|(_, card)| Arc::clone(card))
        .ok_or(TutorialError::MissingCard(name))
}

/// The lesson's game: the player's G/W deck with Giada, Font of Hope in the command zone, the bot's black one.
///
/// The player holds Forest, Grizzly Bears and Hero in Training over two Forests in play, so nothing white can be cast
/// before the Plains comes; their library (top first) is Plains, Giant Growth (drawn by Hero in Training), Forest,
/// then Forests. The bot holds Swamps and a Dregscape Zombie over one Swamp in play.
pub fn setup() -> Result<GameSetup, TutorialError> {
    let forest = card("Forest")?;
    let swamp = card("Swamp")?;
    let mut player_library = vec![card("Plains")?, card("Giant Growth")?, forest.clone()];
    player_library.extend(std::iter::repeat_n(forest.clone(), 12));
    let player = PlayerSetup {
        library: player_library,
        hand: vec![
            forest.clone(),
            card("Grizzly Bears")?,
            card("Hero in Training")?,
        ],
        // Two Forests, so Hero in Training on turn 3 leaves one open for Giant Growth on the bot's turn.
        battlefield: vec![
            PresetPermanent::new(forest.clone()),
            PresetPermanent::new(forest),
        ],
        life: Some(PLAYER_LIFE),
        commanders: vec![card("Giada, Font of Hope")?],
        ..PlayerSetup::default()
    };
    let bot = PlayerSetup {
        library: std::iter::repeat_n(swamp.clone(), 15).collect(),
        hand: vec![
            swamp.clone(),
            card("Dregscape Zombie")?,
            swamp.clone(),
            swamp.clone(),
        ],
        battlefield: vec![PresetPermanent::new(swamp)],
        life: Some(BOT_LIFE),
        ..PlayerSetup::default()
    };
    let mut setup = GameSetup::new(FormatKind::Commander.rules(), SEED, Vec::new());
    setup.players = vec![player, bot];
    setup.start = Start::At {
        turn: 1,
        active: PLAYER_SEAT,
        step: Step::Main1,
    };
    setup.validate_decks = false;
    Ok(setup)
}

/// The bot's answer: the first land it may play, else the first spell it may cast, else a pass; every creature it may
/// attack with, at the player; no blocks; the engine's own payment; the table's fallback for anything else.
pub fn bot_answer(decision: &Decision) -> Option<Answer> {
    match &decision.kind {
        DecisionKind::Priority(options) => Some(Answer::Priority(
            match (options.playable_lands.first(), options.castable.first()) {
                (Some(&land), _) => PriorityAction::PlayLand(land),
                (None, Some(&spell)) => PriorityAction::CastSpell(spell),
                (None, None) => PriorityAction::Pass,
            },
        )),
        DecisionKind::DeclareAttackers(options) => Some(Answer::DeclareAttackers(
            options
                .attackers
                .iter()
                .filter(|option| {
                    option.tax.is_empty()
                        && option
                            .defenders
                            .contains(&AttackTarget::Player(PLAYER_SEAT))
                })
                .map(|option| (option.object, AttackTarget::Player(PLAYER_SEAT)))
                .collect(),
        )),
        DecisionKind::DeclareBlockers(_) => Some(Answer::DeclareBlockers(Vec::new())),
        DecisionKind::PayMana(options) if options.auto_available => {
            Some(Answer::PayMana(PayChoice::Auto))
        }
        _ => crate::pace::fallback(decision),
    }
}

/// Plays the bot's seat of `handle` until the game ends or closes: readies both seats, then answers each decision
/// sent to the bot with [`bot_answer`].
pub fn spawn_bot(handle: GameHandle) -> JoinHandle<()> {
    tokio::spawn(async move {
        let Ok(mut inbox) = handle.join(BOT_SEAT).await else {
            return;
        };
        for seat in [BOT_SEAT, PLAYER_SEAT] {
            if handle.send(seat, ClientMessage::Ready).await.is_err() {
                return;
            }
        }
        while let Some(message) = inbox.recv().await {
            match message {
                ServerMessage::Decision { id, decision, .. } => {
                    let Some(answer) = bot_answer(&decision) else {
                        continue;
                    };
                    let reply = ClientMessage::Answer {
                        decision: id,
                        answer,
                    };
                    if handle.send(BOT_SEAT, reply).await.is_err() {
                        return;
                    }
                }
                ServerMessage::GameOver { .. } | ServerMessage::EndedByAdmin { .. } => return,
                _ => {}
            }
        }
    })
}

/// The answer to `POST /api/tutorial`: the new game and the player's seat link.
#[derive(Debug, Serialize)]
pub(crate) struct TutorialBody {
    game: String,
    seat: PlayerId,
    token: String,
}

/// `POST /api/tutorial`: starts a tutorial game, plays it already begun, and gives the player seat 0. No body.
pub(crate) async fn start(
    State(app): State<AppState>,
    request: Request,
) -> Result<(StatusCode, Json<TutorialBody>), ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let user = app.auth().session_user(request.headers()).await?;
    let name = user.map_or_else(|| "You".to_owned(), |user| user.username);
    let setup = setup().map_err(|error| {
        tracing::error!(%error, "tutorial setup");
        ApiError::internal()
    })?;
    let (created, handle) = app
        .registry()
        .register_tutorial(setup, vec![name, BOT_NAME.to_owned()])
        .await?;
    for step in PLAYER_STOPS {
        let stop = ClientMessage::SetStop {
            side: StopSide::Own,
            step,
            on: true,
            keep: true,
        };
        handle
            .send(PLAYER_SEAT, stop)
            .await
            .map_err(|_| ApiError::internal())?;
    }
    spawn_bot(handle);
    let Some((seat, token)) = created.seats.into_iter().next() else {
        return Err(ApiError::internal());
    };
    tracing::info!(game = %created.id, "tutorial created");
    let body = TutorialBody {
        game: created.id.to_string(),
        seat,
        token: token.expose().to_owned(),
    };
    Ok((StatusCode::CREATED, Json(body)))
}
