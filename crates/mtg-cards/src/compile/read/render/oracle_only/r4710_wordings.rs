//! r47-10 wordings written from a twin another writer words: an enters trigger that works only for a card ("When this
//! enchantment enters, if it's not a token, each of your teammates creates a token that's a copy of this enchantment.",
//! Imperial Mask), and target players who each manifest cards from their hands, counted for the caster's draw (Kozilek,
//! the Broken Reality).

use crate::ability::Ability;

/// The intervening condition of a permanent that is not a token.
const R4710_NOT_TOKEN: &str =
    r#""if":{"Not":{"Matches":{"subject":"This","selector":["Token"]}}},"#;

/// Adds the r47-10 wordings of the ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    r4710_enters_unless_token(ability, texts);
    r4710_manifest_from_hands(ability, texts);
}

/// The cast trigger of up to `players` target players who each manifest `cards` cards from their hands, and the caster's
/// draw of one card for each card manifested.
fn r4710_hands_manifested(players: u32, cards: u32) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":["This"],"by":"You"}}}},"from":"Stack","targets":[{{"what":"Player","bounds":{{"min":0,"max":{players}}}}}],"effects":[{{"Choose":{{"who":{{"Target":0}},"from":{{"Each":[{{"In":"Hand"}},{{"OwnedBy":"Chooser"}}]}},"count":{{"min":{cards},"max":{cards}}},"as":"chosen"}}}},{{"Manifest":{{"who":{{"Target":0}},"cards":{{"Bound":"chosen"}},"as":"manifested"}}}},{{"Draw":{{"count":{{"Count":[{{"Is":{{"Bound":"manifested"}}}}]}}}}}}]}}}}"#
    )
}

/// "When you cast this spell, up to two target players each manifest two cards from their hands. For each card
/// manifested this way, you draw a card."
fn r4710_manifest_from_hands(ability: &Ability, texts: &mut Vec<String>) {
    let words = ["one", "two", "three", "four"];
    for (players, many) in (1u32..).zip(words) {
        for (cards, count) in (1u32..).zip(words) {
            let Ok(shape) =
                serde_json::from_str::<Ability>(&r4710_hands_manifested(players, cards))
            else {
                continue;
            };
            if shape != *ability {
                continue;
            }
            let (target, them) = if players == 1 {
                ("target player", "manifests")
            } else {
                ("target players each", "manifest")
            };
            let (noun, hands) = match (cards, players) {
                (1, 1) => ("a card".to_owned(), "their hand"),
                (1, _) => ("a card".to_owned(), "their hands"),
                (_, 1) => (format!("{count} cards"), "their hand"),
                _ => (format!("{count} cards"), "their hands"),
            };
            texts.push(format!(
                "When you cast this spell, up to {many} {target} {them} {noun} from {hands}. For each card manifested this way, you draw a card."
            ));
        }
    }
}

/// "When ~ enters, if it's not a token, ...": the twin without the condition, the condition said after the head.
fn r4710_enters_unless_token(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { .. } = ability else {
        return;
    };
    let Ok(json) = serde_json::to_string(ability) else {
        return;
    };
    if !json.contains(R4710_NOT_TOKEN) {
        return;
    }
    let Ok(plain) = serde_json::from_str::<Ability>(&json.replacen(R4710_NOT_TOKEN, "", 1)) else {
        return;
    };
    for text in super::super::written_variants(&plain) {
        if let Some(rest) = text.strip_prefix("When ~ enters, ") {
            texts.push(format!("When ~ enters, if it's not a token, {rest}"));
        }
    }
}
