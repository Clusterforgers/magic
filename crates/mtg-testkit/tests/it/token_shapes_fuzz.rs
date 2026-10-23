//! Goal: whole bot games at 2 to 4 seats with creatures that make a list of kinds, the predefined Vibranium, Map and Mutagen tokens, a legendary named token, a token with an ability of its own, tokens for each opponent and a bound result keep every invariant (zones with tokens that cease to exist, life, the journal, the replay of the log equal to the live state), and across the seeds each shape of token occurs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::format::Normal;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::{mountain, plains};
use mtg_testkit::play_game;
use rstest::rstest;

/// A creature that makes what `effects` say when it enters.
fn maker(name: &str, effects: &str) -> Arc<CardDefinition> {
    let trigger = format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":{effects}}}}}"#
    );
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse("{1}").unwrap())
            .with_rules(serde_json::from_str::<Ability>(&trigger).unwrap()),
    )
}

const SNAKE: &str = r#"{"Custom":{"name":"Snake","colors":["Green"],"types":["Creature"],"subtypes":["Snake"],"power":1,"toughness":1}}"#;
const WOLF: &str = r#"{"Custom":{"name":"Wolf","colors":["Green"],"types":["Creature"],"subtypes":["Wolf"],"power":2,"toughness":2}}"#;
const TINY: &str = r#"{"Custom":{"name":"Tiny","colors":["Green"],"supertypes":["Legendary"],"types":["Creature"],"subtypes":["Dog","Detective"],"power":2,"toughness":2,"abilities":[{"Keyword":"Trample"}]}}"#;
const SPAWN: &str = r#"{"Custom":{"name":"Eldrazi Spawn","types":["Creature"],"subtypes":["Eldrazi","Spawn"],"power":0,"toughness":1,"abilities":[{"Activated":{"cost":["SacrificeSelf"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}]}}"#;

fn makers() -> Vec<Arc<CardDefinition>> {
    vec![
        maker(
            "Pack",
            &format!(
                r#"[{{"CreateToken":{{"token":{SNAKE}}}}},{{"CreateToken":{{"token":{WOLF}}}}}]"#
            ),
        ),
        maker(
            "Vault",
            r#"[{"CreateToken":{"token":{"Named":"vibranium"},"tapped":true}},{"CreateToken":{"token":{"Named":"map"}}},{"CreateToken":{"token":{"Named":"mutagen"}}}]"#,
        ),
        maker(
            "Legend",
            &format!(r#"[{{"CreateToken":{{"token":{TINY}}}}}]"#),
        ),
        maker(
            "Hatchery",
            &format!(r#"[{{"CreateToken":{{"token":{SPAWN},"count":2}}}}]"#),
        ),
        maker(
            "Gift",
            &format!(r#"[{{"CreateToken":{{"who":"Opponents","token":{WOLF}}}}}]"#),
        ),
        maker(
            "Sizer",
            r#"[{"CreateToken":{"token":{"Sized":{"size":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"token":{"name":"Horror","colors":["Black"],"types":["Creature"],"subtypes":["Horror"],"abilities":[{"Static":{"affected":"This","mods":["CantBlock"]}}]}}}}}]"#,
        ),
        maker(
            "Mourner",
            r#"[{"CreateToken":{"token":{"Sized":{"size":{"Tallied":{"who":"EachPlayer","what":"CreaturesDied"}},"token":{"name":"Ghoul","types":["Creature"],"subtypes":["Ghoul"]}}}}}]"#,
        ),
        maker(
            "Grower",
            &format!(
                r#"[{{"CreateToken":{{"token":{SNAKE},"as":"made"}}}},{{"PutCounters":{{"on":{{"Bound":"made"}},"counter":"+1/+1","count":2}}}}]"#
            ),
        ),
    ]
}

fn deck() -> Vec<Arc<CardDefinition>> {
    let mut deck: Vec<Arc<CardDefinition>> = Vec::new();
    deck.extend((0..18).map(|_| Arc::new(mountain().as_ref().clone())));
    deck.extend((0..18).map(|_| Arc::new(plains().as_ref().clone())));
    for card in makers() {
        deck.extend((0..4).map(|_| Arc::clone(&card)));
    }
    deck
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_every_token_shape_finish_and_keep_every_invariant(#[case] seats: usize) {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    // Tokens an opponent of the maker controls: the Wolves of "Gift" are the only ones made for another player.
    let mut for_others = 0;
    for seed in 0..fuzz_games(20) {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(); seats]);
        let outcome =
            play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        let mut controllers = BTreeMap::new();
        for event in &outcome.events {
            if let Event::TokenCreated(token) = event {
                *seen.entry(token.name.clone()).or_default() += 1;
                if token.name == "Wolf" {
                    controllers.insert(token.object, token.controller);
                }
            }
        }
        for_others += usize::from(
            controllers
                .values()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 1,
        );
    }
    for name in [
        "Snake",
        "Wolf",
        "Vibranium",
        "Map",
        "Mutagen",
        "Tiny",
        "Eldrazi Spawn",
        "Horror",
        "Ghoul",
    ] {
        assert!(seen.contains_key(name), "{name} was made: {seen:?}");
    }
    assert!(for_others > 0, "Wolves were made for more than one player");
}
