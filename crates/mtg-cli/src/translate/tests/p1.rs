//! Goal: piece P1 (selectors): the world holds the names of the checkout's cards and tokens.
use super::world;

#[test]
fn world_names_hold_tokens() {
    assert!(world().names.contains("Soldier Token"));
}
