//! The Learn course's tables: a scripted Commander game per lesson against a bot, made by one request, never recorded.
//!
//! Each lesson of [`LESSONS`] names its table, the stops the learner holds, the bot's script and the learner's
//! script (`docs/tutorial.md`). Every card is a real card of the corpus. No clock hurries either seat; the bot waits
//! the beat (`MTG_TUTORIAL_BEAT_MS`) before each answer.

mod bot;
mod lessons;

use std::sync::{Arc, OnceLock};

use axum::Json;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use mtg_cards::{CardDefinition, builtin};
use mtg_core::ids::PlayerId;
use mtg_import::scryfall::{CardRecord, cards};
use mtg_protocol::ClientMessage;
use serde::{Deserialize, Serialize};

pub use bot::spawn as spawn_bot;
pub use lessons::{
    Act, BotAct, BotLine, COMBAT_BOT_LIFE, COMMANDER, Kind, LESSONS, Lesson, Line, lesson,
};

use crate::api::ApiError;
use crate::server::AppState;

/// Scryfall objects (trimmed) of the lessons' cards, each a printing of a Verified corpus card.
const SCRYFALL: &str = include_str!("scryfall.json");

/// The seat the learner sits in.
pub const PLAYER_SEAT: PlayerId = PlayerId(0);
/// The seat the scripted bot plays.
pub const BOT_SEAT: PlayerId = PlayerId(1);
/// The bot's name at the table.
pub const BOT_NAME: &str = "Practice bot";
/// The learner's name without an account.
pub const GUEST_NAME: &str = "Guest";
/// The lesson a request without one starts.
pub const FIRST_LESSON: &str = "seat";
/// The most bytes a request body may hold.
const BODY_LIMIT: usize = 1024;

/// Why a lesson's cards could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TutorialError {
    /// A card of the lesson is missing from the embedded data.
    #[error("the tutorial card {0:?} is missing")]
    MissingCard(&'static str),
}

/// The lessons' cards by name, as this engine runs them.
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

/// The answer to `POST /api/tutorial`: the new game and the learner's seat link.
#[derive(Debug, Serialize)]
pub(crate) struct TutorialBody {
    game: String,
    seat: PlayerId,
    token: String,
}

/// The body of `POST /api/tutorial`; empty means the first lesson.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct TutorialRequest {
    lesson: Option<String>,
}

/// The lesson `request` asks for: its body's `lesson`, the first lesson without one.
async fn asked(request: Request) -> Result<&'static Lesson, ApiError> {
    let bytes = axum::body::to_bytes(request.into_body(), BODY_LIMIT)
        .await
        .map_err(|_| {
            ApiError::invalid(format!("the request body is limited to {BODY_LIMIT} bytes"))
        })?;
    let body: TutorialRequest = if bytes.iter().all(u8::is_ascii_whitespace) {
        TutorialRequest::default()
    } else {
        serde_json::from_slice(&bytes)
            .map_err(|error| ApiError::invalid(format!("not a lesson request: {error}")))?
    };
    let id = body.lesson.as_deref().unwrap_or(FIRST_LESSON);
    lesson(id).ok_or_else(|| ApiError::invalid(format!("there is no lesson {id:?}")))
}

/// `POST /api/tutorial`: starts the lesson the body names (`{"lesson": id}`, the first without one) and gives the
/// learner seat 0, holding the lesson's stops.
pub(crate) async fn start(
    State(app): State<AppState>,
    request: Request,
) -> Result<(StatusCode, Json<TutorialBody>), ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let user = app.auth().session_user(request.headers()).await?;
    let lesson = asked(request).await?;
    let name = user.map_or_else(|| GUEST_NAME.to_owned(), |user| user.username);
    let setup = (lesson.setup)().map_err(|error| {
        tracing::error!(%error, lesson = lesson.id, "tutorial setup");
        ApiError::internal()
    })?;
    let (created, handle) = app
        .registry()
        .register_tutorial(setup, vec![name, BOT_NAME.to_owned()])
        .await?;
    for &(side, step) in lesson.stops {
        let stop = ClientMessage::SetStop {
            side,
            step,
            on: true,
            keep: true,
        };
        handle
            .send(PLAYER_SEAT, stop)
            .await
            .map_err(|_| ApiError::internal())?;
    }
    spawn_bot(handle, lesson.bot, app.config().tutorial_beat);
    let Some((seat, token)) = created.seats.into_iter().next() else {
        return Err(ApiError::internal());
    };
    tracing::info!(game = %created.id, lesson = lesson.id, "tutorial created");
    let body = TutorialBody {
        game: created.id.to_string(),
        seat,
        token: token.expose().to_owned(),
    };
    Ok((StatusCode::CREATED, Json(body)))
}
