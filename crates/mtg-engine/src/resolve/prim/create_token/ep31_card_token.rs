//! Goal: a token copy of a named card ("Create a Tarmogoyf token") is made from the card the game can conjure by that
//! name, as many as asked, tapped when it says so, under the player named; a name the game cannot resolve makes nothing.
use rstest::rstest;

use super::*;
use crate::fixtures::{creature, setup};
use crate::resolve::{Resolving, Scope};
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

fn game() -> GameState {
    let table = setup(
        vec![Vec::new(), Vec::new()],
        vec![vec![creature("Source", 1, 1)], Vec::new()],
    )
    .with_conjurable([("Tarmogoyf".to_owned(), creature("Tarmogoyf", 0, 1))]);
    GameState::from_setup(&table).unwrap()
}

fn made(game: &GameState, json: &str) -> Vec<(String, bool)> {
    let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
    let resolving = Resolving::begin(source.id());
    let cx = Cx {
        scope: Scope::announcing(game, ME, source.reference()),
        resolving: &resolving,
    };
    let effect: Effect = serde_json::from_str(json).unwrap();
    let Ok(Run::Done { events, .. }) = run(&cx, &effect) else {
        panic!("a token maker asks nothing here");
    };
    events
        .into_iter()
        .filter_map(|event| match event {
            Event::TokenCreated(token) if token.controller == ME => {
                Some((token.name, token.tapped))
            }
            _ => None,
        })
        .collect()
}

#[rstest]
#[case::one(r#"{"CreateToken":{"token":{"Card":"Tarmogoyf"}}}"#, 1, false)]
#[case::two_tapped(
    r#"{"CreateToken":{"token":{"Card":"Tarmogoyf"},"count":2,"tapped":true}}"#,
    2,
    true
)]
#[case::an_unknown_name(r#"{"CreateToken":{"token":{"Card":"Black Lotus"}}}"#, 0, false)]
fn ep31_a_token_copy_of_a_named_card(
    #[case] json: &str,
    #[case] count: usize,
    #[case] tapped: bool,
) {
    let found = made(&game(), json);
    assert_eq!(found, vec![("Tarmogoyf".to_owned(), tapped); count]);
}
