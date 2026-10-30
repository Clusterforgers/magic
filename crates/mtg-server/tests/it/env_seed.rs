//! Goal: `MTG_SEED` pins every game's shuffles, so one seed gives a game the same first events each time and another seed gives other ones.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{self, Client};

use mtg_protocol::ServerMessage;
use mtg_server::registry::SeatRequest;
use mtg_server::{Registry, ServerConfig, demo};
use rstest::rstest;

fn pair() -> Vec<SeatRequest> {
    ["mono-green", "mono-red"]
        .iter()
        .enumerate()
        .map(|(i, id)| SeatRequest {
            name: format!("Player {i}"),
            deck: demo::find(id).unwrap(),
        })
        .collect()
}

/// A registry whose configuration came from the environment variable `MTG_SEED=seed`.
fn registry_from_env(seed: u64) -> Registry {
    let config =
        ServerConfig::from_lookup(|key| (key == "MTG_SEED").then(|| seed.to_string())).unwrap();
    Registry::new(&config)
}

/// The event batches the first seat to decide hears before its first decision.
async fn first_events(registry: &Registry) -> Vec<ServerMessage> {
    let created = registry.create(&pair()).await.unwrap();
    let handle = registry.handle(created.id).unwrap();
    let (mut clients, first) = common::start_table(&handle, 2).await;
    let client: &mut Client = &mut clients[first];
    let mut heard = Vec::new();
    loop {
        match client.next().await {
            ServerMessage::Decision { .. } => return heard,
            batch @ ServerMessage::Events(_) => heard.push(batch),
            _ => {}
        }
    }
}

#[rstest]
#[case(3, 4)]
#[case(1_000_000_007, 1_000_000_008)]
#[tokio::test]
async fn one_seed_gives_the_same_first_events_and_another_gives_others(
    #[case] seed: u64,
    #[case] other: u64,
) {
    let registry = registry_from_env(seed);
    let first = first_events(&registry).await;
    assert!(!first.is_empty(), "the opening is dealt before a decision");
    assert_eq!(first, first_events(&registry).await);
    assert_eq!(first, first_events(&registry_from_env(seed)).await);
    assert_ne!(first, first_events(&registry_from_env(other)).await);
}
