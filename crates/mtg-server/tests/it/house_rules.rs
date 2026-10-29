//! Goal: `POST /api/games` takes house rules, plays the game under them, lists them in the game browser and in the table the seats see, keeps a value equal to the format's as given, and refuses a value out of range with a 400 that names the rule, while a request without them is the game it always was.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{self, Client};
use crate::net;

use mtg_core::house::{HandLimit, Handicap, HouseRules, SkipFirstDraw};
use mtg_core::ids::PlayerId;
use mtg_protocol::ServerMessage;
use mtg_server::registry::{GameOptions, Registry, SeatRequest};
use mtg_server::{ServerConfig, Visibility, demo};
use net::lobby::{call, lobbies};
use net::{TestServer, bot_config};
use rstest::rstest;
use serde_json::{Value, json};

async fn server() -> TestServer {
    let mut config = bot_config();
    config.auth.limits.lobby_requests_per_ip = 100_000;
    TestServer::start(config).await
}

/// A public two-seat game with the creator's `house_rules`, as the request carries them.
fn request(house_rules: &Value) -> String {
    json!({
        "format": "normal",
        "visibility": "public",
        "seats": [
            { "name": "Ann", "deck": "mono-green" },
            { "name": "Bo", "deck": "mono-red" },
        ],
        "house_rules": house_rules,
    })
    .to_string()
}

async fn create(server: &TestServer, house_rules: Value) -> net::Reply {
    call(
        server.addr,
        "POST",
        "/api/games",
        Some(&request(&house_rules)),
        &[],
    )
    .await
}

/// The `house_rules` of every listed game, newest first; `None` for a game without any.
async fn listed_rules(server: &TestServer) -> Vec<Option<Value>> {
    let reply = lobbies(server.addr, &[]).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()["games"]
        .as_array()
        .unwrap()
        .iter()
        .map(|game| game.get("house_rules").cloned())
        .collect()
}

#[tokio::test]
async fn rules_set_at_creation_are_listed_with_the_game() {
    let server = server().await;
    let rules = json!({
        "starting_life": 40,
        "opening_hand_size": 5,
        "free_mulligans": 100,
        "mulligan_cap": 3,
        "skip_first_draw": "never",
        "max_hand_size": { "cards": 9 },
    });
    let reply = create(&server, rules.clone()).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    assert_eq!(listed_rules(&server).await, [Some(rules)]);
}

#[tokio::test]
async fn the_rules_of_the_turn_and_life_are_listed_with_the_game() {
    let server = server().await;
    let rules = json!({
        "draw_step_cards": 2,
        "land_plays_per_turn": 3,
        "mana_burn": true,
        "empty_library": "reshuffle_graveyard",
        "turn_limit": 12,
        "winning_life": 60,
        "poison_threshold": 5,
        "handicaps": [
            { "seat": 0, "opening_hand_size": 5 },
            { "seat": 1, "starting_life": 30, "opening_hand_size": 9 },
        ],
    });
    let reply = create(&server, rules.clone()).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    assert_eq!(listed_rules(&server).await, [Some(rules)]);
}

/// The lookup of an invite-only game, which no game browser lists, made with `house_rules` (none when `None`).
async fn lookup_of_invite_game(server: &TestServer, house_rules: Option<Value>) -> Value {
    let mut body = json!({
        "format": "normal",
        "visibility": "invite",
        "seats": [
            { "name": "Ann", "deck": "mono-green" },
            { "name": "Bo", "deck": "mono-red" },
        ],
    });
    if let Some(rules) = house_rules {
        body["house_rules"] = rules;
    }
    let made = call(
        server.addr,
        "POST",
        "/api/games",
        Some(&body.to_string()),
        &[],
    )
    .await;
    assert_eq!(made.status, 201, "{}", made.text());
    let id = made.json()["game"].as_str().unwrap().to_owned();
    let reply = call(server.addr, "GET", &format!("/api/games/{id}"), None, &[]).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()
}

#[tokio::test]
async fn the_lookup_of_an_invite_only_game_names_its_rules() {
    let server = server().await;
    let rules = json!({ "starting_life": 30, "mulligan_style": "vancouver" });
    let lookup = lookup_of_invite_game(&server, Some(rules.clone())).await;
    assert_eq!(lookup["house_rules"], rules);
    assert_eq!(listed_rules(&server).await, Vec::<Option<Value>>::new());
}

#[rstest]
#[case::absent(None)]
#[case::empty(Some(json!({})))]
#[tokio::test]
async fn the_lookup_of_a_game_without_rules_names_none(#[case] rules: Option<Value>) {
    let server = server().await;
    let lookup = lookup_of_invite_game(&server, rules).await;
    assert!(lookup.get("house_rules").is_none(), "{lookup}");
}

#[rstest]
#[case::absent(None)]
#[case::empty(Some(json!({})))]
#[case::only_nulls(Some(json!({ "starting_life": null, "max_hand_size": null })))]
#[tokio::test]
async fn a_game_without_rules_lists_none(#[case] rules: Option<Value>) {
    let server = server().await;
    let mut body: Value = serde_json::from_str(&request(&json!(null))).unwrap();
    match rules {
        Some(rules) => body["house_rules"] = rules,
        None => {
            body.as_object_mut().unwrap().remove("house_rules");
        }
    }
    let reply = call(
        server.addr,
        "POST",
        "/api/games",
        Some(&body.to_string()),
        &[],
    )
    .await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    assert_eq!(listed_rules(&server).await, [None]);
}

#[rstest]
#[case::life(json!({ "starting_life": 20 }))]
#[case::hand(json!({ "opening_hand_size": 7, "free_mulligans": 0 }))]
#[case::limit(json!({ "max_hand_size": { "cards": 7 } }))]
#[case::the_formats_range_and_style(json!({ "attack_range": "any", "mulligan_style": "london", "open_hands": false }))]
#[case::the_rules_of_the_table(json!({ "attack_range": "left", "open_hands": true, "revealed_library_top": true, "opening_hand_smoothing": true, "mulligan_style": "vancouver" }))]
#[case::the_rules_for_commander(json!({ "commander_tax": 2, "commander_damage": { "lethal": 21 }, "commanders_per_seat": 2 }))]
#[case::the_deck_check(json!({ "deck": { "singleton": false, "colour_identity": false, "deck_size": 100, "max_copies": 3 } }))]
#[case::no_commander_damage(json!({ "commander_damage": "off", "commander_tax": 0 }))]
#[tokio::test]
async fn a_value_equal_to_the_formats_is_kept_as_given(#[case] rules: Value) {
    let server = server().await;
    assert_eq!(create(&server, rules.clone()).await.status, 201);
    assert_eq!(listed_rules(&server).await, [Some(rules)]);
}

#[rstest]
#[case::life_zero(json!({ "starting_life": 0 }), "starting_life")]
#[case::life_negative(json!({ "starting_life": -5 }), "starting_life")]
#[case::life_high(json!({ "starting_life": 1001 }), "starting_life")]
#[case::hand_zero(json!({ "opening_hand_size": 0 }), "opening_hand_size")]
#[case::hand_high(json!({ "opening_hand_size": 21 }), "opening_hand_size")]
#[case::free_high(json!({ "free_mulligans": 101 }), "free_mulligans")]
#[case::cap_high(json!({ "mulligan_cap": 101 }), "mulligan_cap")]
#[case::limit_high(json!({ "max_hand_size": { "cards": 51 } }), "max_hand_size")]
#[case::draws_none(json!({ "draw_step_cards": 0 }), "draw_step_cards")]
#[case::draws_high(json!({ "draw_step_cards": 11 }), "draw_step_cards")]
#[case::lands_none(json!({ "land_plays_per_turn": 0 }), "land_plays_per_turn")]
#[case::lands_high(json!({ "land_plays_per_turn": 11 }), "land_plays_per_turn")]
#[case::limit_none(json!({ "turn_limit": 0 }), "turn_limit")]
#[case::limit_high(json!({ "turn_limit": 1001 }), "turn_limit")]
#[case::wins_none(json!({ "winning_life": 0 }), "winning_life")]
#[case::poison_none(json!({ "poison_threshold": 0 }), "poison_threshold")]
#[case::poison_high(json!({ "poison_threshold": 101 }), "poison_threshold")]
#[case::tax_high(json!({ "commander_tax": 11 }), "commander_tax")]
#[case::lethal_none(json!({ "commander_damage": { "lethal": 0 } }), "commander_damage")]
#[case::lethal_high(json!({ "commander_damage": { "lethal": 101 } }), "commander_damage")]
#[case::deck_size_low(json!({ "deck": { "deck_size": 39 } }), "deck.deck_size")]
#[case::deck_size_high(json!({ "deck": { "deck_size": 251 } }), "deck.deck_size")]
#[case::copies_none(json!({ "deck": { "singleton": false, "max_copies": 0 } }), "deck.max_copies")]
#[case::copies_high(json!({ "deck": { "singleton": false, "max_copies": 100 } }), "deck.max_copies")]
#[case::copies_of_a_singleton(json!({ "deck": { "max_copies": 2 } }), "deck.max_copies")]
#[case::commanders_none(json!({ "commanders_per_seat": 0 }), "commanders_per_seat")]
#[case::commanders_three(json!({ "commanders_per_seat": 3 }), "commanders_per_seat")]
#[case::handicap_life(json!({ "handicaps": [{ "seat": 0, "starting_life": 0 }] }), "handicaps[0].starting_life")]
#[case::handicap_hand(json!({ "handicaps": [{ "seat": 0, "starting_life": 5 }, { "seat": 1, "opening_hand_size": 21 }] }), "handicaps[1].opening_hand_size")]
#[case::a_seat_the_table_lacks(json!({ "handicaps": [{ "seat": 2, "starting_life": 5 }] }), "handicaps")]
#[case::the_same_seat_twice(json!({ "handicaps": [{ "seat": 1, "starting_life": 5 }, { "seat": 1, "opening_hand_size": 5 }] }), "handicaps")]
#[case::a_handicap_that_sets_nothing(json!({ "handicaps": [{ "seat": 1 }] }), "handicaps")]
#[case::the_bad_one_among_good_ones(json!({ "starting_life": 30, "opening_hand_size": 99 }), "opening_hand_size")]
#[tokio::test]
async fn a_value_out_of_range_is_refused_naming_the_rule(
    #[case] rules: Value,
    #[case] field: &str,
) {
    let server = server().await;
    let reply = create(&server, rules).await;
    assert_eq!(reply.status, 400, "{}", reply.text());
    let body = reply.json();
    assert_eq!(body["error"], "invalid_request");
    assert_eq!(body["field"], field);
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .starts_with(&format!("house_rules.{field}: ")),
        "{body}"
    );
    assert_eq!(listed_rules(&server).await, [], "nothing was created");
}

#[rstest]
#[case::unknown_rule(json!({ "banana": true }))]
#[case::a_switch_that_is_not_one(json!({ "mana_burn": "yes" }))]
#[case::unknown_library_rule(json!({ "empty_library": "explode" }))]
#[case::unknown_attack_range(json!({ "attack_range": "right" }))]
#[case::unknown_mulligan_style(json!({ "mulligan_style": "Paris" }))]
#[case::open_hands_that_is_not_a_switch(json!({ "open_hands": "yes" }))]
#[case::smoothing_that_is_a_number(json!({ "opening_hand_smoothing": 1 }))]
#[case::a_handicap_with_an_unknown_value(json!({ "handicaps": [{ "seat": 0, "mana": 3 }] }))]
#[case::a_handicap_for_a_negative_seat(json!({ "handicaps": [{ "seat": -1, "starting_life": 3 }] }))]
#[case::a_handicap_without_a_seat(json!({ "handicaps": [{ "starting_life": 3 }] }))]
#[case::text_for_a_number(json!({ "starting_life": "forty" }))]
#[case::damage_as_a_bare_number(json!({ "commander_damage": 21 }))]
#[case::damage_with_an_unknown_name(json!({ "commander_damage": "none" }))]
#[case::a_deck_rule_that_is_unknown(json!({ "deck": { "banana": 1 } }))]
#[case::singleton_as_text(json!({ "deck": { "singleton": "off" } }))]
#[case::a_deck_that_is_a_number(json!({ "deck": 100 }))]
#[case::negative_tax(json!({ "commander_tax": -1 }))]
#[case::negative_count(json!({ "opening_hand_size": -1 }))]
#[case::a_fraction(json!({ "starting_life": 20.5 }))]
#[case::unknown_first_draw(json!({ "skip_first_draw": "sometimes" }))]
#[case::limit_without_a_number(json!({ "max_hand_size": { "cards": "seven" } }))]
#[case::not_an_object(json!("none"))]
#[tokio::test]
async fn a_malformed_rule_is_an_invalid_request(#[case] rules: Value) {
    let server = server().await;
    let reply = create(&server, rules).await;
    assert_eq!(reply.status, 400, "{}", reply.text());
    assert_eq!(reply.json()["error"], "invalid_request");
}

fn requests(seats: usize) -> Vec<SeatRequest> {
    (0..seats)
        .map(|n| SeatRequest {
            name: format!("Player {n}"),
            deck: &demo::all()[n % demo::all().len()],
        })
        .collect()
}

/// The first table every seat is shown once the game has begun: its rules, every player's life and the starting
/// player's hand.
async fn first_table(house_rules: HouseRules, seats: usize) -> (Option<HouseRules>, Vec<i32>) {
    let registry = Registry::new(&ServerConfig {
        game: common::roomy(),
        ..ServerConfig::default()
    });
    let options = GameOptions {
        visibility: Visibility::Invite,
        house_rules,
        ..GameOptions::default()
    };
    let created = registry
        .create_with(&requests(seats), options, None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    let (mut clients, _) = common::start_table(&handle, seats).await;
    let client: &mut Client = &mut clients[0];
    let table = client
        .until(|message| match message {
            ServerMessage::Events(batch) => Some((*batch.table).clone()),
            ServerMessage::Snapshot { state, .. } => Some(state.table.clone()),
            _ => None,
        })
        .await;
    (
        table.house_rules,
        table.players.iter().map(|p| p.life).collect(),
    )
}

#[rstest]
#[case::nothing_set(HouseRules::default(), None, 20)]
#[case::life_set(
    HouseRules { starting_life: Some(33), ..HouseRules::default() },
    Some(HouseRules { starting_life: Some(33), ..HouseRules::default() }),
    33
)]
#[case::a_value_equal_to_the_formats(
    HouseRules { starting_life: Some(20), skip_first_draw: Some(SkipFirstDraw::Never), ..HouseRules::default() },
    Some(HouseRules { starting_life: Some(20), skip_first_draw: Some(SkipFirstDraw::Never), ..HouseRules::default() }),
    20
)]
#[case::a_limit_alone(
    HouseRules { max_hand_size: Some(HandLimit::Unlimited), ..HouseRules::default() },
    Some(HouseRules { max_hand_size: Some(HandLimit::Unlimited), ..HouseRules::default() }),
    20
)]
#[tokio::test]
async fn the_table_every_seat_sees_carries_the_rules_that_are_set(
    #[case] house: HouseRules,
    #[case] shown: Option<HouseRules>,
    #[case] life: i32,
    #[values(2, 3)] seats: usize,
) {
    let (rules, lives) = first_table(house, seats).await;
    assert_eq!(rules, shown);
    assert_eq!(lives, vec![life; seats]);
}

#[rstest]
#[tokio::test]
async fn a_handicap_shows_in_the_table_as_the_life_of_its_seat_only(
    #[values(2, 3, 4)] seats: usize,
) {
    let house = HouseRules {
        starting_life: Some(30),
        handicaps: vec![Handicap {
            seat: PlayerId(1),
            starting_life: Some(7),
            opening_hand_size: Some(4),
        }],
        mana_burn: Some(true),
        turn_limit: Some(5),
        ..HouseRules::default()
    };
    let (rules, lives) = first_table(house.clone(), seats).await;
    assert_eq!(rules, Some(house));
    let want: Vec<i32> = (0..seats)
        .map(|seat| if seat == 1 { 7 } else { 30 })
        .collect();
    assert_eq!(lives, want);
}
