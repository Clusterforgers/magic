//! r47-10 wordings written from a twin another writer words: an enters trigger that works only for a card ("When this
//! enchantment enters, if it's not a token, each of your teammates creates a token that's a copy of this enchantment.",
//! Imperial Mask), and target players who each manifest cards from their hands, counted for the caster's draw (Kozilek,
//! the Broken Reality), counters removed from among creatures and twice that many put on the creature (Galloping Lizrog),
//! a card put onto the battlefield attacking the player it came from (Shark Shredder, Killer Clone), and energy counted
//! from the target opponent's lands and a second effect on the defending player's creatures (The Motherlode, Excavator).

use serde_json::{Value, json};

use crate::ability::Ability;

/// The intervening condition of a permanent that is not a token.
const R4710_NOT_TOKEN: &str =
    r#""if":{"Not":{"Matches":{"subject":"This","selector":["Token"]}}},"#;

/// Adds the r47-10 wordings of the ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    r4710_enters_unless_token(ability, texts);
    r4710_manifest_from_hands(ability, texts);
    r4710_among_doubled(ability, texts);
    r4710_attacking_that_player(ability, texts);
    r4710_energy_of_lands(ability, texts);
    r4710_that_player_cant_block(ability, texts);
}

/// The body of a triggered ability as JSON: its fields under `Triggered`.
fn r4710_body(ability: &Ability) -> Option<serde_json::Map<String, Value>> {
    let Ability::Triggered { .. } = ability else {
        return None;
    };
    serde_json::to_value(ability)
        .ok()?
        .get("Triggered")?
        .as_object()
        .cloned()
}

/// The head of a triggered ability ("When ~ enters, "), from the twin that draws a card with no targets.
fn r4710_head(ability: &Ability) -> Vec<String> {
    let Some(mut body) = r4710_body(ability) else {
        return Vec::new();
    };
    body.remove("targets");
    body.insert("effects".to_owned(), json!([{ "Draw": {} }]));
    let Ok(twin) = serde_json::from_value::<Ability>(json!({ "Triggered": body })) else {
        return Vec::new();
    };
    super::super::written_variants(&twin)
        .into_iter()
        .filter_map(|text| text.strip_suffix("draw a card.").map(str::to_owned))
        .collect()
}

/// Whether the triggered ability has exactly these targets (`[]` for none) and effects, as JSON.
fn r4710_steps_are(ability: &Ability, targets: &str, effects: &str) -> bool {
    let Some(body) = r4710_body(ability) else {
        return false;
    };
    let (Ok(targets), Ok(effects)) = (
        serde_json::from_str::<Value>(targets),
        serde_json::from_str::<Value>(effects),
    ) else {
        return false;
    };
    body.get("targets").cloned().unwrap_or_else(|| json!([])) == targets
        && body.get("effects") == Some(&effects)
}

/// "When ~ enters, you may remove any number of +1/+1 counters from among creatures you control. If you do, put twice
/// that many +1/+1 counters on ~."
fn r4710_among_doubled(ability: &Ability, texts: &mut Vec<String>) {
    const STEPS: &str = r#"[{"Pay":{"cost":[{"RemoveCountersAmong":{"counter":"+1/+1","count":"X","from":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"paid":[{"PutCounters":{"on":"This","counter":"+1/+1","count":{"Times":{"by":2,"of":"X"}}}}]}}]"#;
    if !r4710_steps_are(ability, "[]", STEPS) {
        return;
    }
    for head in r4710_head(ability) {
        texts.push(format!(
            "{head}you may remove any number of +1/+1 counters from among creatures you control. If you do, put twice that many +1/+1 counters on ~."
        ));
    }
}

/// "Whenever ~ deals combat damage to a player, put up to one target creature card from that player's graveyard onto the
/// battlefield under your control. It enters tapped and attacking that player."
fn r4710_attacking_that_player(ability: &Ability, texts: &mut Vec<String>) {
    const TARGETS: &str = r#"[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"Triggering"}]},"bounds":{"min":0,"max":1}}]"#;
    const STEPS: &str = r#"[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"tapped":true,"under":"You"}},"as":"returned"}},{"JoinAttack":{"what":{"Bound":"returned"},"against":"Triggering"}}]"#;
    if !r4710_steps_are(ability, TARGETS, STEPS) {
        return;
    }
    for head in r4710_head(ability) {
        texts.push(format!(
            "{head}put up to one target creature card from that player's graveyard onto the battlefield under your control. It enters tapped and attacking that player."
        ));
    }
}

/// "When ~ enters, choose target opponent. You get an amount of {E} equal to the number of nonbasic lands that player
/// controls."
fn r4710_energy_of_lands(ability: &Ability, texts: &mut Vec<String>) {
    const TARGETS: &str = r#"[{"what":"Opponent"}]"#;
    const STEPS: &str = r#"[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":{"Count":[{"Type":"Land"},{"Not":{"Supertype":"Basic"}},{"ControlledBy":{"Target":0}}]}}}]"#;
    if !r4710_steps_are(ability, TARGETS, STEPS) {
        return;
    }
    for head in r4710_head(ability) {
        texts.push(format!(
            "{head}choose target opponent. You get an amount of {{E}} equal to the number of nonbasic lands that player controls."
        ));
    }
}

/// "..., and creatures that player controls without flying can't block this turn": a second effect on the creatures of
/// the defending player the first step named, said after the twin without it and naming that player again.
fn r4710_that_player_cant_block(ability: &Ability, texts: &mut Vec<String>) {
    const CANT_BLOCK: &str = r#",{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Defending"},{"Not":{"HasKeyword":"Flying"}}]},"mods":["CantBlock"],"duration":"UntilEndOfTurn"}}"#;
    let Ok(json) = serde_json::to_string(ability) else {
        return;
    };
    if !json.contains(CANT_BLOCK) {
        return;
    }
    let Ok(twin) = serde_json::from_str::<Ability>(&json.replacen(CANT_BLOCK, "", 1)) else {
        return;
    };
    for text in super::super::written_variants(&twin) {
        if let Some(head) = text.strip_suffix(" defending player controls.") {
            texts.push(format!(
                "{head} defending player controls, and creatures that player controls without flying can't block this turn."
            ));
        }
    }
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
