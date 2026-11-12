//! Goal: every corpus card whose text investigates, discovers or manifests dread logs that action after its steps, a
//! Clue token made without investigating logs nothing, and the writers word the logged steps and the triggers on them
//! as the cards say them and nothing near them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::written_variants;
use mtg_cards::load::Library;
use rstest::rstest;

use crate::common::{cards_dir, printed};

/// The printed text of a card without its reminder text, in lower case.
fn rules_text(faces: &[mtg_cards::PrintedFace]) -> String {
    let mut text = String::new();
    for face in faces {
        let mut depth = 0_u32;
        for c in face.oracle_text.chars() {
            match c {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                _ if depth == 0 => text.push(c.to_ascii_lowercase()),
                _ => {}
            }
        }
        text.push('\n');
    }
    text
}

/// Whether the text performs the action `verb` names, and not only waits for it ("whenever you investigate").
fn performs(text: &str, verbs: &[&str]) -> bool {
    let waits = [
        "whenever you ",
        "whenever an opponent ",
        "whenever a player ",
    ];
    verbs.iter().any(|verb| {
        text.match_indices(verb)
            .any(|(at, _)| !waits.iter().any(|wait| text[..at].ends_with(wait)))
    })
}

#[rstest]
#[case::investigate(&["investigate"], "Investigated")]
#[case::discover(&["discover 1", "discover 2", "discover 3", "discover 4", "discover 5", "discover x"], "Discovered")]
#[case::manifest_dread(&["manifest dread", "manifests dread"], "ManifestedDread")]
fn a_card_that_performs_the_action_logs_it(#[case] verbs: &[&str], #[case] action: &str) {
    let library = Library::load_dir(&cards_dir()).unwrap();
    let logged = format!(r#""action":"{action}""#);
    let mut missing = Vec::new();
    for card in printed() {
        let Some(rules) = library.card(&card.oracle_id) else {
            continue;
        };
        let json = serde_json::to_string(rules).unwrap();
        let by_hand = json.contains(r#""Manual""#);
        if performs(&rules_text(&card.faces), verbs) && !by_hand && !json.contains(&logged) {
            missing.push(card.name);
        }
    }
    assert!(missing.is_empty(), "no {action} log: {missing:?}");
}

#[test]
fn a_clue_made_without_investigating_logs_nothing() {
    let library = Library::load_dir(&cards_dir()).unwrap();
    let wrong: Vec<String> = printed()
        .into_iter()
        .filter(|card| !rules_text(&card.faces).contains("investigat"))
        .filter(|card| {
            library.card(&card.oracle_id).is_some_and(|rules| {
                serde_json::to_string(rules)
                    .unwrap()
                    .contains(r#""action":"Investigated""#)
            })
        })
        .map(|card| card.name)
        .collect();
    assert!(wrong.is_empty(), "{wrong:?}");
}

fn says(json: &str, text: &str) -> bool {
    let ability: Ability = serde_json::from_str(json).unwrap();
    written_variants(&ability)
        .iter()
        .any(|written| written == text)
}

const INVESTIGATE: &str =
    r#"[{"CreateToken":{"token":{"Named":"clue"}}},{"Performed":{"action":"Investigated"}}]"#;

#[rstest]
#[case::erdwal(
    &format!(r#"{{"Triggered":{{"when":{{"Nth":{{"event":{{"Did":{{"action":"Investigated","who":"You"}}}},"nth":1}}}},"effects":{INVESTIGATE}}}}}"#),
    "Whenever you investigate for the first time each turn, investigate an additional time."
)]
#[case::every_investigation(
    &format!(r#"{{"Triggered":{{"when":{{"Did":{{"action":"Investigated","who":"You"}}}},"effects":{INVESTIGATE}}}}}"#),
    "Whenever you investigate, investigate an additional time."
)]
#[case::curator(
    r#"{"Triggered":{"when":{"Did":{"action":"Discovered","who":"You"}},"limit":"OncePerTurn","effects":[{"ExileUntil":{"filter":[{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":"Triggering"}}],"as":"found","exiled_as":"exiled"}},{"CastFromZone":{"what":{"Bound":"found"},"free":true,"otherwise":[{"Move":{"what":{"Bound":"found"},"to":"Hand"}}]}},{"Move":{"what":{"Bound":"exiled"},"to":{"Library":{"place":"Bottom","order":"Random"}}}},{"Performed":{"action":"Discovered","card":{"Bound":"found"},"value":"Triggering"}}]}}"#,
    "Whenever you discover, discover again for the same value. This ability triggers only once each turn."
)]
#[case::analyst(
    r#"{"Triggered":{"when":{"Did":{"action":"ManifestedDread","who":"You"}},"effects":[{"Move":{"what":"Triggering","to":"Hand"}}]}}"#,
    "Whenever you manifest dread, put a card you put into your graveyard this way into your hand."
)]
#[case::investigate_alone(&format!(r#"{{"Spell":{{"effects":{INVESTIGATE}}}}}"#), "Create a Clue token.")]
fn the_logged_steps_are_worded_as_the_card_says(#[case] json: &str, #[case] text: &str) {
    assert!(
        says(json, text),
        "{json}: {:?}",
        written_variants(&serde_json::from_str(json).unwrap())
    );
}

#[rstest]
#[case::curator_every_time(
    r#"{"Triggered":{"when":{"Did":{"action":"Discovered","who":"You"}},"effects":[{"ExileUntil":{"filter":[{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":"Triggering"}}],"as":"found","exiled_as":"exiled"}},{"CastFromZone":{"what":{"Bound":"found"},"free":true,"otherwise":[{"Move":{"what":{"Bound":"found"},"to":"Hand"}}]}},{"Move":{"what":{"Bound":"exiled"},"to":{"Library":{"place":"Bottom","order":"Random"}}}},{"Performed":{"action":"Discovered","card":{"Bound":"found"},"value":"Triggering"}}]}}"#
)]
#[case::analyst_returns_another(
    r#"{"Triggered":{"when":{"Did":{"action":"ManifestedDread","who":"You"}},"effects":[{"Move":{"what":"Triggering","to":"Graveyard"}}]}}"#
)]
#[case::discover_unlogged(
    r#"{"Spell":{"effects":[{"ExileUntil":{"filter":[{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":3}}],"as":"found","exiled_as":"exiled"}},{"CastFromZone":{"what":{"Bound":"found"},"free":true,"otherwise":[{"Move":{"what":{"Bound":"found"},"to":"Hand"}}]}},{"Move":{"what":{"Bound":"exiled"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#
)]
#[case::investigation_of_another_player(
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"clue"}}},{"Performed":{"action":"Investigated","who":"Opponents"}}]}}"#
)]
fn near_misses_are_not_worded_as_the_card(#[case] json: &str) {
    let ability: Ability = serde_json::from_str(json).unwrap();
    let written = written_variants(&ability);
    let cards = [
        "Whenever you discover, discover again for the same value. This ability triggers only once each turn.",
        "Whenever you manifest dread, put a card you put into your graveyard this way into your hand.",
        "Discover 3.",
        "Create a Clue token.",
        "Investigate.",
    ];
    assert!(
        !written.iter().any(|text| cards.contains(&text.as_str())),
        "{json}: {written:?}"
    );
}
