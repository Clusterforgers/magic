//! Goal: every game is made with a join code that finds that game and no other, however the visitor types it; wrong and malformed codes find nothing and cost the address one of its five misses a minute, the sixth try is turned away even with the right code until the minute is over, a hit costs nothing, addresses have budgets of their own, and no code is ever printed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use mtg_server::join_code::{ALPHABET, CODE_CHARS, JoinCode};
use mtg_server::registry::SeatRequest;
use mtg_server::{GameId, Registry, ServerConfig, demo};
use rstest::rstest;

fn requests() -> Vec<SeatRequest> {
    ["mono-green", "mono-red"]
        .iter()
        .map(|id| SeatRequest {
            name: (*id).to_owned(),
            deck: demo::find(id).unwrap(),
        })
        .collect()
}

fn ip(text: &str) -> IpAddr {
    text.parse().unwrap()
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

/// A game and the code it was made with.
async fn game(registry: &Registry) -> (GameId, JoinCode) {
    let created = registry.create(&requests()).await.unwrap();
    (created.id, created.code)
}

#[tokio::test]
async fn a_new_game_has_a_code_of_the_right_shape_that_finds_it() {
    let registry = Registry::new(&ServerConfig::default());
    let (id, code) = game(&registry).await;
    let text = code.expose();
    assert_eq!(text.len(), CODE_CHARS);
    assert!(text.bytes().all(|b| ALPHABET.contains(&b)));

    let now = Instant::now();
    assert_eq!(
        registry.find_by_code(text, ip("10.0.0.1"), now),
        Ok(Some(id))
    );
}

#[tokio::test]
async fn every_game_is_found_by_its_own_code_only() {
    let registry = Registry::new(&ServerConfig::default());
    let mut games = Vec::new();
    for _ in 0..60 {
        games.push(game(&registry).await);
    }
    let distinct: HashSet<&str> = games.iter().map(|(_, code)| code.expose()).collect();
    assert_eq!(
        distinct.len(),
        games.len(),
        "no two live games share a code"
    );
    let now = Instant::now();
    for (n, (id, code)) in games.iter().enumerate() {
        // A fresh address per lookup keeps the throttle out of the way.
        let from = IpAddr::from([10, 1, 0, u8::try_from(n).unwrap()]);
        assert_eq!(
            registry.find_by_code(code.expose(), from, now),
            Ok(Some(*id))
        );
    }
}

#[rstest]
#[case::as_shown(|code: &str| code.to_owned())]
#[case::lower_case(|code: &str| code.to_lowercase())]
#[case::hyphenated(|code: &str| format!("{}-{}", &code[..4], &code[4..]))]
#[case::spaced_and_padded(|code: &str| format!("  {} {}\n", &code[..4], &code[4..]))]
#[tokio::test]
async fn a_code_is_found_however_it_is_typed(#[case] typed: fn(&str) -> String) {
    let registry = Registry::new(&ServerConfig::default());
    let (id, code) = game(&registry).await;
    let found = registry.find_by_code(&typed(code.expose()), ip("10.0.0.1"), Instant::now());
    assert_eq!(found, Ok(Some(id)));
}

#[rstest]
#[case::empty("")]
#[case::another_code("ABCD2345")]
#[case::too_short("ABCD234")]
#[case::too_long("ABCD23456")]
#[case::look_alikes("ABCDO1I0")]
#[case::punctuation("ABCD.345")]
#[case::non_ascii("ÄBCD2345")]
#[case::a_game_id("6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55")]
#[tokio::test]
async fn a_wrong_or_malformed_code_finds_nothing(#[case] typed: &str) {
    let registry = Registry::new(&ServerConfig::default());
    let (_, code) = game(&registry).await;
    // The case that happens to be the game's own code would find it.
    assert_ne!(code.expose(), typed);
    let found = registry.find_by_code(typed, ip("10.0.0.1"), Instant::now());
    assert_eq!(found, Ok(None));
}

#[tokio::test]
async fn five_misses_a_minute_per_address_then_even_the_right_code_is_turned_away() {
    let registry = Registry::new(&ServerConfig::default());
    let (id, code) = game(&registry).await;
    let me = ip("192.0.2.7");
    let t0 = Instant::now();

    for n in 0..5 {
        let miss = registry.find_by_code("ABCD2345", me, t0 + secs(n));
        assert_eq!(miss, Ok(None), "miss {n}");
    }
    // The window began at the first miss and lasts a minute.
    let blocked = registry.find_by_code(code.expose(), me, t0 + secs(10));
    assert_eq!(blocked.unwrap_err().retry_after, secs(50));
    let blocked = registry.find_by_code("ABCD2345", me, t0 + secs(59));
    assert_eq!(blocked.unwrap_err().seconds(), 1);
    // Turned-away tries cost nothing and do not push the end out.
    assert_eq!(
        registry.find_by_code(code.expose(), me, t0 + secs(60)),
        Ok(Some(id))
    );

    // Another address has a budget of its own.
    assert_eq!(
        registry.find_by_code(code.expose(), ip("192.0.2.8"), t0 + secs(10)),
        Ok(Some(id))
    );
}

#[tokio::test]
async fn a_code_that_finds_its_game_does_not_use_up_the_budget() {
    let registry = Registry::new(&ServerConfig::default());
    let (id, code) = game(&registry).await;
    let me = ip("192.0.2.7");
    let t0 = Instant::now();
    for _ in 0..50 {
        assert_eq!(registry.find_by_code(code.expose(), me, t0), Ok(Some(id)));
    }
    for _ in 0..5 {
        assert_eq!(registry.find_by_code("ABCD2345", me, t0), Ok(None));
    }
    assert!(registry.find_by_code(code.expose(), me, t0).is_err());
}

#[rstest]
#[case::one_network("2001:db8:1:2::1", "2001:db8:1:2:ffff::9", true)]
#[case::another_network("2001:db8:1:2::1", "2001:db8:1:3::1", false)]
#[tokio::test]
async fn misses_are_counted_per_network_not_per_address(
    #[case] first: &str,
    #[case] second: &str,
    #[case] shared: bool,
) {
    let registry = Registry::new(&ServerConfig::default());
    let t0 = Instant::now();
    for _ in 0..5 {
        assert_eq!(registry.find_by_code("ABCD2345", ip(first), t0), Ok(None));
    }
    assert_eq!(
        registry.find_by_code("ABCD2345", ip(second), t0).is_err(),
        shared
    );
}

#[tokio::test]
async fn the_limits_come_from_the_configuration() {
    let mut config = ServerConfig::default();
    config.auth.limits.code_misses_per_ip = 2;
    config.auth.limits.code_window = secs(5);
    let registry = Registry::new(&config);
    let (id, code) = game(&registry).await;
    let me = ip("192.0.2.7");
    let t0 = Instant::now();
    assert_eq!(registry.find_by_code("ABCD2345", me, t0), Ok(None));
    assert_eq!(registry.find_by_code("ABCD2345", me, t0), Ok(None));
    assert!(
        registry
            .find_by_code(code.expose(), me, t0 + secs(4))
            .is_err()
    );
    assert_eq!(
        registry.find_by_code(code.expose(), me, t0 + secs(5)),
        Ok(Some(id))
    );
}

#[tokio::test]
async fn a_game_that_is_gone_is_found_no_more() {
    let registry = Registry::new(&ServerConfig::default());
    let (_, code) = game(&registry).await;
    let later = Instant::now() + secs(3_601);
    assert_eq!(registry.sweep(later).await, 1);
    assert_eq!(
        registry.find_by_code(code.expose(), ip("10.0.0.1"), Instant::now()),
        Ok(None)
    );
}

#[tokio::test]
async fn no_code_is_printed() {
    let registry = Registry::new(&ServerConfig::default());
    let created = registry.create(&requests()).await.unwrap();
    let code = created.code.expose().to_owned();
    let shown = format!("{created:?} {created:#?}");
    assert!(!shown.contains(&code), "{shown}");
    let public = registry.describe(created.id).await.unwrap();
    assert!(!format!("{public:?}").contains(&code));
}
