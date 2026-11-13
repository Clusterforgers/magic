//! Delves: co-op boss-rush runs (`docs/delves.md`). A run is stored as its header and the log of its events; every
//! request folds the log with `mtg-campaign`, applies what the caller does, lets the bot allies answer, makes the
//! boss fight's game when the party reaches one (a normal recorded game with the run's plan in its recipe) and reads
//! a finished fight back into the run.

mod store;

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, StatusCode};
use mtg_campaign::{Brain, Content, FightPlan, HeroDeck, Run, RunEvent, Status, content};
use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::GameSetup;
use mtg_engine::state::GameResult;
use mtg_import::resolve::Lookup;
use mtg_protocol::SeatDeck;
use mtg_protocol::rest::{
    CreateDelve, CustomCardView, DelveAction, DelveBossView, DelveContent, DelveDeckProblem,
    DelveEventView, DelveFightView, DelveList, DelveSummary, DelveView,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::{ApiError, read_json};
use crate::bots::{BotLevel, BotSeat};
use crate::deck_choice::{Refused, resolve, resolve_as};
use crate::game_id::GameId;
use crate::join_code::JoinCode;
use crate::recipe::FormatKind;
use crate::records::Visibility;
use crate::records::{RecordError, RecordStatus};
use crate::registry::GameOptions;
use crate::seat_deck::ChosenDeck;
use crate::server::AppState;
use crate::user_id::UserId;
use store::{DelveStore, Header};

/// The theme every run plays for now.
const THEME: &str = "warrens";
/// The most runs a list shows.
const LIST_LIMIT: i64 = 50;
/// The highest Heat.
const MAX_HEAT: u8 = 2;

/// The runs of a server with a database.
#[derive(Debug)]
pub(crate) struct Delves {
    store: DelveStore,
    /// One request changes runs at a time, so two answers never fold the same log twice.
    turn: Arc<tokio::sync::Mutex<()>>,
}

impl Delves {
    /// The service over `pool`, whose server migrations have run.
    pub(crate) fn new(pool: PgPool) -> Delves {
        Delves {
            store: DelveStore::new(pool),
            turn: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

/// A fight's setup: the plan laid over the decks, with the campaign's content.
pub(crate) fn fight_setup(
    plan: &FightPlan,
    setup: GameSetup,
) -> Result<GameSetup, mtg_campaign::CampaignError> {
    plan.apply(content()?, setup)
}

fn failure(status: StatusCode, code: &'static str, message: impl Into<String>) -> ApiError {
    ApiError::new(status, code, message)
}

fn unavailable(error: &RecordError) -> ApiError {
    tracing::error!(%error, "delve records failed");
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "delves are not available right now; try again later",
    )
}

fn not_found() -> ApiError {
    failure(StatusCode::NOT_FOUND, "not_found", "no such delve")
}

fn refused(message: impl Into<String>) -> ApiError {
    failure(StatusCode::CONFLICT, "refused", message)
}

fn service(app: &AppState) -> Result<&Delves, ApiError> {
    app.delves().ok_or_else(|| {
        failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "no_database",
            "this server runs without a database, so it keeps no delves",
        )
    })
}

fn campaign() -> Result<&'static Content, ApiError> {
    content().map_err(|error| {
        tracing::error!(%error, "the campaign content does not read");
        ApiError::internal()
    })
}

fn id_of(path: Result<Path<String>, PathRejection>) -> Result<Uuid, ApiError> {
    let Path(id) = path.map_err(|_| not_found())?;
    Uuid::parse_str(&id).map_err(|_| not_found())
}

/// The seed code: the seed in base 36.
fn seed_code(seed: u64) -> String {
    let mut n = seed;
    let mut out = Vec::new();
    loop {
        let digit = u32::try_from(n % 36).unwrap_or(0);
        out.push(char::from_digit(digit, 36).unwrap_or('0'));
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.iter().rev().collect::<String>().to_uppercase()
}

fn parse_seed(code: &str) -> Option<u64> {
    u64::from_str_radix(code.trim(), 36).ok()
}

fn status_word(status: &Status) -> &'static str {
    match status {
        Status::Lobby => "lobby",
        Status::Map => "map",
        Status::Fight { .. } => "fight",
        Status::Room => "room",
        Status::Victory => "victory",
        Status::Defeat => "defeat",
        Status::Abandoned => "abandoned",
    }
}

fn bot_level(brain: Brain) -> BotLevel {
    match brain {
        Brain::Easy => BotLevel::Easy,
        Brain::Normal => BotLevel::Normal,
        Brain::Hard => BotLevel::Hard,
        Brain::VeryHard => BotLevel::VeryHard,
    }
}

fn letters(colors: impl Iterator<Item = Color>) -> String {
    let mut out: Vec<char> = colors
        .map(|color| match color {
            Color::White => 'W',
            Color::Blue => 'U',
            Color::Black => 'B',
            Color::Red => 'R',
            Color::Green => 'G',
        })
        .collect();
    out.sort_by_key(|c| "WUBRG".find(*c));
    out.dedup();
    out.into_iter().collect()
}

/// A loaded run: its header, its log and the run they fold to.
struct Loaded {
    header: Header,
    events: Vec<RunEvent>,
    run: Run,
}

async fn load(delves: &Delves, id: Uuid) -> Result<Loaded, ApiError> {
    let header = delves
        .store
        .header(id)
        .await
        .map_err(|e| unavailable(&e))?
        .ok_or_else(not_found)?;
    let events = delves.store.events(id).await.map_err(|e| unavailable(&e))?;
    let run = Run::replay(
        campaign()?,
        &header.theme,
        header.seed,
        header.heat,
        &events,
    )
    .map_err(|error| {
        tracing::error!(%error, delve = %id, "a stored delve does not fold");
        ApiError::internal()
    })?;
    Ok(Loaded {
        header,
        events,
        run,
    })
}

/// Applies `event` to the loaded run, keeping it in the log.
fn push(loaded: &mut Loaded, event: RunEvent) -> Result<(), ApiError> {
    loaded
        .run
        .apply(campaign()?, &event)
        .map_err(|error| refused(error.to_string()))?;
    loaded.events.push(event);
    Ok(())
}

/// Lets the bot allies answer until they have nothing to do.
fn settle_bots(loaded: &mut Loaded) -> Result<(), ApiError> {
    for _ in 0..64 {
        let moves = loaded.run.bot_moves(campaign()?);
        if moves.is_empty() {
            break;
        }
        for event in moves {
            push(loaded, event)?;
        }
    }
    Ok(())
}

/// The accounts of the party.
fn members(run: &Run) -> Vec<Uuid> {
    run.heroes
        .iter()
        .filter_map(|hero| hero.user.as_deref())
        .filter_map(|user| Uuid::parse_str(user).ok())
        .collect()
}

async fn save(delves: &Delves, loaded: &Loaded, from: usize) -> Result<(), ApiError> {
    if loaded.events.len() == from {
        return Ok(());
    }
    delves
        .store
        .append(
            loaded.header.id,
            from,
            &loaded.events[from..],
            status_word(&loaded.run.status),
            &members(&loaded.run),
        )
        .await
        .map_err(|e| unavailable(&e))
}

/// Reads a finished fight back into the run; a fight whose game ended unplayed waits for a new one.
async fn sync_fight(app: &AppState, loaded: &mut Loaded) -> Result<(), ApiError> {
    let Status::Fight { game: Some(game) } = &loaded.run.status else {
        return Ok(());
    };
    let Ok(game) = game.parse::<GameId>() else {
        return Ok(());
    };
    let result = match app.registry().record_result(game).await {
        Ok(Some((RecordStatus::Finished, Some(result)))) => result,
        Ok(Some((RecordStatus::Lobby | RecordStatus::Running, _))) | Err(_) => return Ok(()),
        // Still in the registry: the record is behind the table, which may yet end.
        _ if app.registry().handle(game).is_some() => return Ok(()),
        // Ended unplayed (an admin ended it, it expired, failed or is gone): the fight gets a new game.
        _ => return push(loaded, RunEvent::FightVoided),
    };
    let Ok(Some(seats)) = app.registry().final_seats(game).await else {
        return Ok(());
    };
    let heroes = loaded.run.heroes.len();
    let won = result == GameResult::Team(0);
    let lives = seats.iter().take(heroes).map(|(life, _)| *life).collect();
    let spirits = seats
        .iter()
        .take(heroes)
        .enumerate()
        .filter(|(_, (_, spirit))| *spirit)
        .filter_map(|(at, _)| u8::try_from(at).ok())
        .collect();
    push(
        loaded,
        RunEvent::FightEnded {
            won,
            lives,
            spirits,
        },
    )
}

/// The cards of `names` from the card database, by oracle id, in order; a name the database lacks is left out.
async fn cards_named(
    app: &AppState,
    names: &[String],
) -> Result<Vec<(String, Arc<CardDefinition>)>, ApiError> {
    let source = app.cards().ok_or_else(ApiError::internal)?;
    let index = source.card_index().await.map_err(|error| {
        tracing::error!(%error, "the card index could not be read");
        ApiError::internal()
    })?;
    let ids: Vec<String> = names
        .iter()
        .filter_map(|name| {
            if let Lookup::Found(card) = index.find(name) {
                Some(card.oracle_id.to_string())
            } else {
                tracing::warn!(%name, "a Delves card is not in the database");
                None
            }
        })
        .collect();
    let cards = source.deck(&ids).await.map_err(|error| {
        tracing::error!(%error, "Delves cards could not be assembled");
        ApiError::internal()
    })?;
    Ok(ids.into_iter().zip(cards).collect())
}

fn deck_refused(refusal: Refused) -> ApiError {
    match refusal {
        Refused::Unknown => refused("no such deck"),
        Refused::Blocked(_) => refused("that deck has no cards to play"),
        Refused::Failed(error) => error,
    }
}

/// The deck of a seat on the boss's side, from its card names and copies.
async fn boss_side_deck(
    app: &AppState,
    name: &str,
    deck: &[(String, u32)],
) -> Result<ChosenDeck, ApiError> {
    let names: Vec<String> = deck
        .iter()
        .flat_map(|(card, n)| std::iter::repeat_n(card.clone(), usize::try_from(*n).unwrap_or(0)))
        .collect();
    let (ids, cards): (Vec<String>, Vec<Arc<CardDefinition>>) =
        cards_named(app, &names).await?.into_iter().unzip();
    let note = SeatDeck {
        name: name.to_owned(),
        format: "commander".to_owned(),
        manual_cards: 0,
        warnings: 0,
    };
    ChosenDeck::stored(ids, cards, note).map_err(|error| {
        tracing::error!(%error, "a boss deck could not be fingerprinted");
        ApiError::internal()
    })
}

/// Why a hero's deck does not play now, in words for the run page; a server failure is an error.
fn deck_problem(refusal: Refused) -> Result<String, ApiError> {
    match refusal {
        Refused::Unknown => {
            Ok("the deck is gone: it was deleted or is no longer shared".to_owned())
        }
        Refused::Blocked(_) => Ok("the deck has no cards the game can play".to_owned()),
        Refused::Failed(error) => Err(error),
    }
}

/// The account a hero's deck is read as: their own, or the host's for a bot ally.
fn deck_owner(loaded: &Loaded, at: usize) -> Uuid {
    loaded
        .run
        .heroes
        .get(at)
        .and_then(|hero| hero.user.as_deref())
        .and_then(|user| Uuid::parse_str(user).ok())
        .unwrap_or(loaded.header.host)
}

/// Each hero's chosen deck with their run cards, or the heroes whose decks no longer play.
async fn hero_decks(
    app: &AppState,
    loaded: &Loaded,
) -> Result<Result<Vec<ChosenDeck>, Vec<DelveDeckProblem>>, ApiError> {
    let mut decks = Vec::new();
    let mut problems = Vec::new();
    for (at, hero) in loaded.run.heroes.iter().enumerate() {
        let hero_at = u8::try_from(at).unwrap_or(u8::MAX);
        let Some(deck) = hero.deck.as_ref() else {
            problems.push(DelveDeckProblem {
                hero: hero_at,
                reason: "no deck was chosen".to_owned(),
            });
            continue;
        };
        match resolve_as(app, deck_owner(loaded, at), &deck.id).await {
            Ok(chosen) => {
                let added = cards_named(app, &hero.cards).await?;
                decks.push(chosen.with_run_cards(&added).map_err(|error| {
                    tracing::error!(%error, "a hero's deck could not be fingerprinted");
                    ApiError::internal()
                })?);
            }
            Err(refusal) => problems.push(DelveDeckProblem {
                hero: hero_at,
                reason: deck_problem(refusal)?,
            }),
        }
    }
    Ok(if problems.is_empty() {
        Ok(decks)
    } else {
        Err(problems)
    })
}

/// Makes the boss fight's game, from the decks the heroes chose, when the party is in a fight that has none. The
/// heroes whose decks no longer play come back instead, and the fight waits for them.
async fn make_fight(
    app: &AppState,
    loaded: &mut Loaded,
) -> Result<Vec<DelveDeckProblem>, ApiError> {
    if loaded.run.status != (Status::Fight { game: None }) {
        return Ok(Vec::new());
    }
    let content = campaign()?;
    let Some(plan) = loaded
        .run
        .fight_plan(content)
        .map_err(|error| refused(error.to_string()))?
    else {
        return Ok(Vec::new());
    };
    let plan = FightPlan {
        run: Some(loaded.header.id.to_string()),
        ..plan
    };
    let mut decks: Vec<Option<ChosenDeck>> = match hero_decks(app, loaded).await? {
        Ok(decks) => decks.into_iter().map(Some).collect(),
        Err(problems) => return Ok(problems),
    };
    let mut names = Vec::new();
    let mut accounts = Vec::new();
    let mut bots = Vec::new();
    for (at, hero) in loaded.run.heroes.iter().enumerate() {
        names.push(hero.name.clone());
        accounts.push(
            hero.user
                .as_deref()
                .and_then(|user| Uuid::parse_str(user).ok())
                .map(UserId::from),
        );
        if let Some(brain) = hero.bot {
            bots.push(BotSeat {
                seat: PlayerId::from_index(at),
                level: bot_level(brain),
            });
        }
    }
    // The boss's side: the boss, then its minions, each a bot seat with its deck.
    let side = std::iter::once((plan.boss.name.as_str(), plan.boss.brain))
        .chain(plan.minions.iter().map(|m| (m.name.as_str(), m.brain)));
    for (at, ((name, brain), deck)) in side.zip(plan.boss_decks()).enumerate() {
        decks.push(Some(boss_side_deck(app, name, deck).await?));
        names.push(name.to_owned());
        accounts.push(None);
        bots.push(BotSeat {
            seat: PlayerId::from_index(plan.heroes.len() + at),
            level: bot_level(brain),
        });
    }
    let options = GameOptions {
        format: FormatKind::Commander,
        visibility: Visibility::Invite,
        accounts,
        bots,
        house_rules: plan.house_rules(&mtg_core::house::HouseRules::default()),
        campaign: Some(plan),
        ..GameOptions::default()
    };
    let created = app
        .registry()
        .create_decked(names, decks, options, None)
        .await
        .map_err(|error| {
            tracing::warn!(%error, delve = %loaded.header.id, "a boss fight could not be made");
            refused("the boss fight could not be made")
        })?;
    push(
        loaded,
        RunEvent::FightCreated {
            game: created.id.to_string(),
        },
    )?;
    Ok(Vec::new())
}

/// Brings a loaded run up to date: a finished fight read back, the bots' answers, the next fight made. Answers the
/// heroes whose decks keep the fight from being made.
async fn advance(app: &AppState, loaded: &mut Loaded) -> Result<Vec<DelveDeckProblem>, ApiError> {
    sync_fight(app, loaded).await?;
    settle_bots(loaded)?;
    match make_fight(app, loaded).await {
        Ok(problems) => Ok(problems),
        Err(error) => {
            tracing::warn!(?error, delve = %loaded.header.id, "the boss fight waits");
            Ok(Vec::new())
        }
    }
}

/// The fight the party is in, as the plan its game is made from.
fn fight_view(run: &Run) -> Option<DelveFightView> {
    if !matches!(run.status, Status::Fight { .. }) {
        return None;
    }
    let plan = run.fight_plan(campaign().ok()?).ok()??;
    Some(DelveFightView {
        boss: plan.boss.name.clone(),
        life: plan.boss.life,
        rules: plan.boss.rules.clone(),
        phases: plan.boss.phases.clone(),
        minions: plan
            .minions
            .iter()
            .map(|minion| (minion.name.clone(), minion.life))
            .collect(),
    })
}

fn view(loaded: &Loaded, user: UserId, deck_problems: Vec<DelveDeckProblem>) -> DelveView {
    let me = loaded
        .run
        .heroes
        .iter()
        .position(|hero| hero.user.as_deref() == Some(&user.uuid().to_string()))
        .and_then(|at| u8::try_from(at).ok());
    let boss = loaded
        .run
        .at
        .and_then(|at| loaded.run.map.node(at))
        .and_then(|node| node.boss.clone())
        .or_else(|| {
            loaded
                .run
                .map
                .choices(loaded.run.at)
                .iter()
                .filter_map(|id| loaded.run.map.node(*id))
                .find_map(|node| node.boss.clone())
        });
    DelveView {
        id: loaded.header.id.to_string(),
        code: loaded.header.code.clone(),
        seed: seed_code(loaded.header.seed),
        host: loaded.header.host == user.uuid(),
        me,
        run: loaded.run.clone(),
        boss,
        deck_problems,
        fight: fight_view(&loaded.run),
    }
}

/// `GET /api/delves/content`: the custom cards, bosses and events the web draws.
pub(crate) async fn content_of() -> Result<Json<DelveContent>, ApiError> {
    let content = campaign()?;
    let theme = content.theme(THEME).map_err(|_| ApiError::internal())?;
    Ok(Json(DelveContent {
        theme: theme.name.clone(),
        text: theme.text.clone(),
        cards: content
            .cards
            .values()
            .map(|card| CustomCardView {
                id: card.id.clone(),
                name: card.name.clone(),
                kind: card.kind,
                type_line: card.type_line.clone(),
                text: card.text.clone(),
                art: card.art.clone(),
                role: card.role.clone(),
                price: card.price,
            })
            .collect(),
        bosses: theme
            .bosses
            .iter()
            .map(|boss| DelveBossView {
                id: boss.id.clone(),
                name: boss.name.clone(),
                colors: boss.colors.clone(),
                line: boss.line.clone(),
                card: boss.rules.first().cloned(),
                phases: boss.phases.clone(),
                life: (boss.life, boss.life_per_hero),
            })
            .collect(),
        events: theme
            .events
            .iter()
            .map(|event| DelveEventView {
                id: event.id.clone(),
                title: event.title.clone(),
                text: event.text.clone(),
                options: event.options.iter().map(|o| o.label.clone()).collect(),
            })
            .collect(),
    }))
}

/// Abandons a run that saw no change for `STALE_DAYS` and has no live fight; whether it did.
async fn abandon_stale(
    app: &AppState,
    delves: &Delves,
    loaded: &mut Loaded,
) -> Result<bool, ApiError> {
    if loaded.run.is_over() {
        return Ok(false);
    }
    if let Status::Fight { game: Some(game) } = &loaded.run.status
        && game
            .parse::<GameId>()
            .is_ok_and(|game| app.registry().handle(game).is_some())
    {
        return Ok(false);
    }
    let _turn = delves.turn.lock().await;
    let from = loaded.events.len();
    push(loaded, RunEvent::Abandoned)?;
    // A write that raced a member's action is refused by the log's key; the run then shows as it is.
    Ok(save(delves, loaded, from).await.is_ok())
}

/// `GET /api/delves`: the caller's runs, abandoned ones left out.
pub(crate) async fn list(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DelveList>, ApiError> {
    let delves = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let ids = delves
        .store
        .list_for(user.id, LIST_LIMIT)
        .await
        .map_err(|e| unavailable(&e))?;
    let content = campaign()?;
    let mut out = Vec::new();
    for (id, stale) in ids {
        let Ok(mut loaded) = load(delves, id).await else {
            continue;
        };
        if stale && abandon_stale(&app, delves, &mut loaded).await? {
            continue;
        }
        let run = &loaded.run;
        let is_boss = |id: &u8| run.map.node(*id).is_some_and(|n| n.boss.is_some());
        out.push(DelveSummary {
            id: id.to_string(),
            code: loaded.header.code.clone(),
            theme: content
                .theme(&run.theme)
                .map_or_else(|_| run.theme.clone(), |t| t.name.clone()),
            status: status_word(&run.status).to_owned(),
            heroes: run.heroes.iter().map(|h| h.name.clone()).collect(),
            cleared: u32::try_from(run.cleared.iter().filter(|id| is_boss(id)).count())
                .unwrap_or(0),
            bosses: u32::try_from(run.map.nodes.iter().filter(|n| n.boss.is_some()).count())
                .unwrap_or(0),
        });
    }
    Ok(Json(DelveList { delves: out }))
}

/// `POST /api/delves`: a new run in its lobby, the caller its host and first hero; 201.
pub(crate) async fn create(
    State(app): State<AppState>,
    request: Request,
) -> Result<(StatusCode, Json<DelveView>), ApiError> {
    let delves = service(&app)?;
    let user = app.auth().require_user(request.headers()).await?;
    let body: CreateDelve = read_json(request).await?;
    if body.heat > MAX_HEAT {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Heat is 0 to 2",
        ));
    }
    let seed = match body.seed.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(code) => parse_seed(code).ok_or_else(|| {
            failure(
                StatusCode::BAD_REQUEST,
                "invalid",
                "a seed code is letters and digits",
            )
        })?,
        None => crate::secret::random_seed().map_err(|_| ApiError::internal())?,
    };
    let code = JoinCode::generate().map_err(|_| ApiError::internal())?;
    let header = Header {
        id: Uuid::new_v4(),
        code: code.expose().to_owned(),
        host: user.id.uuid(),
        theme: THEME.to_owned(),
        seed,
        heat: body.heat,
    };
    let _turn = delves.turn.lock().await;
    delves
        .store
        .create(&header)
        .await
        .map_err(|e| unavailable(&e))?;
    let run = Run::new(campaign()?, THEME, seed, body.heat).map_err(|_| ApiError::internal())?;
    let mut loaded = Loaded {
        header,
        events: Vec::new(),
        run,
    };
    push(
        &mut loaded,
        RunEvent::Joined {
            name: user.username.clone(),
            user: Some(user.id.uuid().to_string()),
            bot: None,
        },
    )?;
    save(delves, &loaded, 0).await?;
    Ok((
        StatusCode::CREATED,
        Json(view(&loaded, user.id, Vec::new())),
    ))
}

/// `POST /api/delves/join/{code}`: the caller joins the run's party in its lobby.
pub(crate) async fn join(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<Json<DelveView>, ApiError> {
    let delves = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let Path(code) = path.map_err(|_| not_found())?;
    let id = delves
        .store
        .by_code(code.trim())
        .await
        .map_err(|e| unavailable(&e))?
        .ok_or_else(not_found)?;
    let _turn = delves.turn.lock().await;
    let mut loaded = load(delves, id).await?;
    let from = loaded.events.len();
    let me = user.id.uuid().to_string();
    if !loaded
        .run
        .heroes
        .iter()
        .any(|h| h.user.as_deref() == Some(&me))
    {
        push(
            &mut loaded,
            RunEvent::Joined {
                name: user.username.clone(),
                user: Some(me),
                bot: None,
            },
        )?;
    }
    save(delves, &loaded, from).await?;
    Ok(Json(view(&loaded, user.id, Vec::new())))
}

/// `GET /api/delves/{id}`: the run as the caller sees it, brought up to date.
pub(crate) async fn get_one(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<Json<DelveView>, ApiError> {
    let delves = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let id = id_of(path)?;
    let _turn = delves.turn.lock().await;
    let mut loaded = load(delves, id).await?;
    let from = loaded.events.len();
    let problems = advance(&app, &mut loaded).await?;
    save(delves, &loaded, from).await?;
    Ok(Json(view(&loaded, user.id, problems)))
}

/// The event a member's action makes, checked against who they are.
async fn event_of(
    app: &AppState,
    headers: &HeaderMap,
    loaded: &Loaded,
    user: UserId,
    action: DelveAction,
) -> Result<RunEvent, ApiError> {
    let host = loaded.header.host == user.uuid();
    let me = loaded
        .run
        .heroes
        .iter()
        .position(|hero| hero.user.as_deref() == Some(&user.uuid().to_string()))
        .and_then(|at| u8::try_from(at).ok());
    let host_only = |event: RunEvent| {
        if host {
            Ok(event)
        } else {
            Err(failure(
                StatusCode::FORBIDDEN,
                "forbidden",
                "only the host can do that",
            ))
        }
    };
    let member = me.ok_or_else(|| {
        failure(
            StatusCode::FORBIDDEN,
            "forbidden",
            "you are not in this party",
        )
    })?;
    match action {
        DelveAction::AddBot { level } => {
            let n = loaded.run.heroes.iter().filter(|h| h.bot.is_some()).count() + 1;
            host_only(RunEvent::Joined {
                name: format!("Ally {n}"),
                user: None,
                bot: Some(level),
            })
        }
        DelveAction::Remove { hero } => {
            if hero == 0 {
                return Err(refused("the host stays in the party"));
            }
            if hero == member {
                return Ok(RunEvent::Left { hero });
            }
            host_only(RunEvent::Left { hero })
        }
        DelveAction::ChooseDeck { hero, deck } => {
            let target = loaded
                .run
                .heroes
                .get(usize::from(hero))
                .ok_or_else(|| refused("no such hero"))?;
            if hero != member && !(host && target.bot.is_some()) {
                return Err(failure(
                    StatusCode::FORBIDDEN,
                    "forbidden",
                    "a player chooses their own deck",
                ));
            }
            if loaded.run.status != Status::Lobby {
                let current = target
                    .deck
                    .as_ref()
                    .map(|d| d.id.clone())
                    .unwrap_or_default();
                let plays = resolve_as(app, deck_owner(loaded, usize::from(hero)), &current).await;
                match plays {
                    Ok(_) => {
                        return Err(refused(
                            "decks change only through rewards once the run has started",
                        ));
                    }
                    Err(Refused::Failed(error)) => return Err(error),
                    Err(_) => {}
                }
            }
            let chosen = resolve(app, headers, &deck).await.map_err(deck_refused)?;
            let colors = letters(
                chosen
                    .commanders()
                    .iter()
                    .flat_map(|card| card.color_identity().iter()),
            );
            Ok(RunEvent::DeckChosen {
                hero,
                deck: HeroDeck {
                    id: deck,
                    name: chosen.note().name.clone(),
                    colors,
                },
            })
        }
        DelveAction::Start => host_only(RunEvent::Started),
        DelveAction::Vote { node } => Ok(RunEvent::Voted { hero: member, node }),
        DelveAction::Pick { pick } => Ok(RunEvent::Picked { hero: member, pick }),
        DelveAction::Abandon => host_only(RunEvent::Abandoned),
    }
}

/// `POST /api/delves/{id}/act`: one action of a member; the run brought up to date after it.
pub(crate) async fn act(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<Json<DelveView>, ApiError> {
    let delves = service(&app)?;
    let headers = request.headers().clone();
    let user = app.auth().require_user(&headers).await?;
    let id = id_of(path)?;
    let action: DelveAction = read_json(request).await?;
    let _turn = delves.turn.lock().await;
    let mut loaded = load(delves, id).await?;
    let from = loaded.events.len();
    advance(&app, &mut loaded).await?;
    let event = event_of(&app, &headers, &loaded, user.id, action).await?;
    push(&mut loaded, event)?;
    let problems = advance(&app, &mut loaded).await?;
    save(delves, &loaded, from).await?;
    Ok(Json(view(&loaded, user.id, problems)))
}

#[cfg(test)]
mod tests {
    //! Goal: seed codes round-trip, colour letters come in WUBRG order once each, and every status has its word.
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    proptest! {
        #[test]
        fn seed_codes_round_trip(seed in any::<u64>()) {
            prop_assert_eq!(parse_seed(&seed_code(seed)), Some(seed));
        }
    }

    #[test]
    fn colours_read_in_wubrg_order_once() {
        let colors = [Color::Green, Color::White, Color::Green, Color::Black];
        assert_eq!(letters(colors.into_iter()), "WBG");
        assert_eq!(letters(std::iter::empty()), "");
    }

    #[test]
    fn statuses_have_words() {
        let words: BTreeSet<&str> = [
            Status::Lobby,
            Status::Map,
            Status::Fight { game: None },
            Status::Room,
            Status::Victory,
            Status::Defeat,
            Status::Abandoned,
        ]
        .iter()
        .map(status_word)
        .collect();
        assert_eq!(words.len(), 7);
    }
}
