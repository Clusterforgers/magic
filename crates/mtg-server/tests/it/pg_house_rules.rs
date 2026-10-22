//! Goal: the house rules a game was made with are stored in its recipe, so a restart brings the game back under them and it ends exactly as the same game played straight on an engine, a lobby read back from its recipe keeps them for the recipe it writes next, and a game made without rules stores a recipe that has no trace of them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Bot, drive, roomy};
use mtg_core::house::{
    AttackRange, CommanderDamageLimit, DeckRules, EmptyLibrary, HandLimit, Handicap, HouseRules,
    MulliganStyle, SkipFirstDraw,
};
use mtg_core::ids::PlayerId;
use mtg_engine::replay;
use mtg_protocol::ClientMessage;
use mtg_server::registry::{GameOptions, SeatRequest};
use mtg_server::{
    DeckRef, EventStore, GameId, GameRecipe, GameRecords, PgEventStore, RecordStatus, Registry,
    ResumeReport, ServerConfig, demo, migrate,
};
use mtg_testkit::play_game;
use serde_json::json;
use sqlx::PgPool;

const DECKS: [&str; 3] = ["mono-green", "mono-red", "mono-white"];

fn requests(players: usize) -> Vec<SeatRequest> {
    DECKS[..players]
        .iter()
        .enumerate()
        .map(|(i, id)| SeatRequest {
            name: format!("Bot {i}"),
            deck: demo::find(id).unwrap(),
        })
        .collect()
}

fn recipe(players: usize, seed: u64, house: HouseRules) -> GameRecipe {
    let decks = DECKS[..players].iter().map(|id| DeckRef::Demo {
        id: (*id).to_owned(),
    });
    GameRecipe::normal(seed, decks).with_house_rules(house)
}

fn config(seed: u64) -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(seed),
        game: roomy(),
        ..ServerConfig::default()
    }
}

fn some_rules() -> HouseRules {
    HouseRules {
        starting_life: Some(11),
        opening_hand_size: Some(5),
        free_mulligans: Some(100),
        mulligan_cap: Some(4),
        skip_first_draw: Some(SkipFirstDraw::Never),
        max_hand_size: Some(HandLimit::Cards(5)),
        draw_step_cards: Some(2),
        land_plays_per_turn: Some(2),
        mana_burn: Some(true),
        empty_library: Some(EmptyLibrary::ReshuffleGraveyard),
        turn_limit: Some(30),
        winning_life: Some(60),
        poison_threshold: Some(3),
        attack_range: Some(AttackRange::Neighbours),
        open_hands: Some(true),
        revealed_library_top: Some(true),
        opening_hand_smoothing: Some(true),
        mulligan_style: Some(MulliganStyle::Vancouver),
        commander_tax: Some(1),
        commander_damage: Some(CommanderDamageLimit::Lethal(30)),
        deck: DeckRules {
            singleton: Some(false),
            colour_identity: Some(false),
            deck_size: Some(60),
            max_copies: Some(4),
        },
        commanders_per_seat: Some(2),
        handicaps: vec![Handicap {
            seat: PlayerId(1),
            starting_life: Some(9),
            opening_hand_size: Some(4),
        }],
    }
}

#[sqlx::test(migrations = false)]
async fn a_game_made_under_house_rules_comes_back_under_them_and_ends_as_the_same_game_does(
    pool: PgPool,
) {
    migrate(&pool).await.unwrap();
    for (players, seed, answers_before) in [(2, 1, 40), (3, 2, 60)] {
        let config = config(seed);
        let registry = Registry::with_database(&config, pool.clone());
        let options = GameOptions {
            house_rules: some_rules(),
            ..GameOptions::default()
        };
        let created = registry
            .create_with(&requests(players), options, None)
            .await
            .unwrap();
        let game = created.id;
        let tokens: Vec<String> = created
            .seats
            .iter()
            .map(|(_, token)| token.expose().to_owned())
            .collect();
        let handle = registry.handle(game).unwrap();
        let mut bots = Vec::new();
        for seat in 0..players {
            bots.push(Bot::join(&handle, seat, seed).await);
        }
        for bot in &bots {
            handle.send(bot.seat, ClientMessage::Ready).await.unwrap();
        }
        assert_eq!(drive(&mut bots, &handle, answers_before).await, None);
        let stopped = handle.status().await.unwrap();
        drop(handle);
        drop(registry); // the process dies; the rows stay

        let record = GameRecords::new(pool.clone()).get(game).await.unwrap();
        assert_eq!(
            record.unwrap().recipe["house_rules"],
            json!({
                "starting_life": 11,
                "opening_hand_size": 5,
                "free_mulligans": 100,
                "mulligan_cap": 4,
                "skip_first_draw": "never",
                "max_hand_size": { "cards": 5 },
                "draw_step_cards": 2,
                "land_plays_per_turn": 2,
                "mana_burn": true,
                "empty_library": "reshuffle_graveyard",
                "turn_limit": 30,
                "winning_life": 60,
                "poison_threshold": 3,
                "attack_range": "neighbours",
                "open_hands": true,
                "revealed_library_top": true,
                "opening_hand_smoothing": true,
                "mulligan_style": "vancouver",
                "commander_tax": 1,
                "commander_damage": { "lethal": 30 },
                "deck": { "singleton": false, "colour_identity": false, "deck_size": 60, "max_copies": 4 },
                "commanders_per_seat": 2,
                "handicaps": [{ "seat": 1, "starting_life": 9, "opening_hand_size": 4 }],
            })
        );

        let registry = Registry::with_database(&config, pool.clone());
        let report = registry.resume().await.unwrap();
        assert_eq!(
            report,
            ResumeReport {
                resumed: 1,
                ..ResumeReport::default()
            }
        );
        let mut handle = None;
        for (seat, token) in tokens.iter().enumerate() {
            let access = registry.authenticate(game, token).unwrap();
            bots[seat].rejoin(&access.handle).await;
            handle = Some(access.handle);
        }
        let handle = handle.unwrap();
        let resumed = handle.status().await.unwrap();
        assert_eq!(resumed.events, stopped.events);
        assert_eq!(resumed.state_hash, stopped.state_hash);

        let result = drive(&mut bots, &handle, usize::MAX).await;
        let setup = recipe(players, seed, some_rules()).build().unwrap();
        let alone = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(result, Some(alone.result), "{players} players");
        let logged = PgEventStore::new(pool.clone()).load(game).await.unwrap();
        assert_eq!(logged, alone.events);
        let replayed = replay(&setup, &logged).unwrap();
        assert_eq!(replayed.state_hash(), alone.final_hash);
        assert_eq!(replayed.house_rules(), &some_rules());
        let record = GameRecords::new(pool.clone()).get(game).await.unwrap();
        assert_eq!(record.unwrap().status, RecordStatus::Finished);
    }
}

#[sqlx::test(migrations = false)]
async fn a_game_made_without_rules_stores_a_recipe_without_them(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(5), pool.clone());
    let created = registry.create(&requests(2)).await.unwrap();
    let record = GameRecords::new(pool.clone())
        .get(created.id)
        .await
        .unwrap();
    let stored = record.unwrap().recipe;
    assert_eq!(
        stored.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["decks", "fingerprints", "format", "seed"]
    );
}

#[sqlx::test(migrations = false)]
async fn a_recipe_stored_before_house_rules_existed_resumes_under_the_format(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let config = config(6);
    let registry = Registry::with_database(&config, pool.clone());
    let created = registry.create(&requests(2)).await.unwrap();
    let game: GameId = created.id;
    drop(registry);

    // The stored shape of a game made before the field existed is the shape of a game made without rules.
    let before = GameRecords::new(pool.clone()).get(game).await.unwrap();
    let stored = before.unwrap().recipe;
    assert!(stored.get("house_rules").is_none());
    let decoded: GameRecipe = serde_json::from_value(stored).unwrap();
    assert!(decoded.house_rules.is_empty());

    let registry = Registry::with_database(&config, pool.clone());
    let report = registry.resume().await.unwrap();
    assert_eq!(report.resumed, 1);
    assert!(registry.handle(game).is_some());
}
