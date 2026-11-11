//! Trigger modes that are an action of a player or object (`TriggerPattern::Did`): one table row per mode.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr};
use super::triggers::{keys, player_ref};
use super::valid::selector_any;
use crate::forge::script::Params;

/// Who the action belongs to: the player the mode names, the object it is done to or by, or both.
#[derive(Clone, Copy)]
enum Subject {
    /// The player under the key.
    Player(&'static str),
    /// The object under the key.
    Object(&'static str),
    /// A player under one key and an object under another.
    Both(&'static str, &'static str),
}

use Subject::{Both, Object, Player};

/// The Forge mode, the action it is and the subject keys it reads; any other key makes the line a gap.
const MODES: [(&str, &str, Subject); 25] = [
    ("Scry", "Scried", Player("ValidPlayer")),
    ("Surveil", "Surveilled", Player("ValidPlayer")),
    ("SearchedLibrary", "SearchedLibrary", Player("ValidPlayer")),
    ("Shuffled", "Shuffled", Player("ValidPlayer")),
    ("LosesGame", "LostGame", Player("ValidPlayer")),
    ("Foretell", "Foretold", Player("ValidPlayer")),
    ("Proliferate", "Proliferated", Player("ValidPlayer")),
    ("Learn", "Learned", Player("ValidPlayer")),
    ("RolledDie", "Rolled", Player("ValidPlayer")),
    (
        "CollectEvidence",
        "CollectedEvidence",
        Player("ValidPlayer"),
    ),
    (
        "FullyUnlock",
        "FullyUnlocked",
        Both("ValidPlayer", "ValidCard"),
    ),
    (
        "TokenCreated",
        "TokenCreated",
        Both("ValidPlayer", "ValidToken"),
    ),
    (
        "LandPlayed",
        "PlayedLand",
        Both("ValidActivatingPlayer", "ValidCard"),
    ),
    ("Explores", "Explored", Object("ValidCard")),
    ("Connives", "Connived", Object("ValidCard")),
    ("BecomeMonstrous", "BecameMonstrous", Object("ValidCard")),
    ("BecomeRenowned", "BecameRenowned", Object("ValidCard")),
    ("Exerted", "Exerted", Object("ValidCard")),
    ("Mutates", "Mutated", Object("ValidCard")),
    ("Transformed", "Transformed", Object("ValidCard")),
    ("Destroyed", "Destroyed", Object("ValidCard")),
    ("Exiled", "Exiled", Object("ValidCard")),
    (
        "ChangesController",
        "ChangedController",
        Object("ValidCard"),
    ),
    ("Regenerated", "Regenerated", Object("ValidCard")),
    ("Countered", "Countered", Object("ValidCard")),
];

/// The pattern of a mode this module reads; `None` for any other mode.
pub fn pattern(ctx: &Ctx, p: &Params, mode: &str) -> Option<Tr<Value>> {
    if let Some(found) = super::triggers_r4::pattern(ctx, p, mode) {
        return Some(found);
    }
    if mode == "AttackerBlockedByCreature" {
        return Some(blocked_by(ctx, p));
    }
    // "Whenever you surveil for the first time each turn": the surveil the engine counts as the player's first.
    if mode == "Surveil" && p.get("FirstTime").is_some() {
        return Some(keys(p, mode, &["ValidPlayer", "FirstTime"]).and_then(|()| {
            let who = player_ref(p.get("ValidPlayer"))?;
            super::triggers::first_time(p, json!({"Surveilled": {"who": who}}))
        }));
    }
    // "Whenever an opponent searches their library": a search of the player's own library only.
    if mode == "SearchedLibrary" && p.get("SearchOwnLibrary") == Some("True") {
        let line = p.without(&["SearchOwnLibrary"]);
        return Some(did(
            ctx,
            &line,
            mode,
            "SearchedOwnLibrary",
            Player("ValidPlayer"),
        ));
    }
    let (_, action, subject) = MODES.iter().find(|(name, ..)| *name == mode)?;
    if let Some(origin) = p.get("Origin") {
        let line = p.without(&["Origin"]);
        return Some(
            super::triggers_g13c::g13c_origin(mode, origin).and_then(|from| {
                let mut found = did(ctx, &line, mode, action, *subject)?;
                if let Some(node) = found.get_mut("Did") {
                    let mut what = node.get("what").cloned().unwrap_or_else(|| json!([]));
                    if let Some(list) = what.as_array_mut() {
                        list.push(from);
                    }
                    node["what"] = what;
                }
                Ok(found)
            }),
        );
    }
    Some(did(ctx, p, mode, action, *subject))
}

/// `AttackerBlockedByCreature`: an attacker becomes blocked by a creature, once for each such blocker.
fn blocked_by(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "AttackerBlockedByCreature",
        &["ValidCard", "ValidBlocker", "ValidBlocked"],
    )?;
    // A stray `ValidBlocked$ Creature` (Cave Tiger) says "by a creature", which every blocker is.
    if p.get("ValidBlocked").is_some_and(|word| word != "Creature") {
        return super::ctx::unmapped("key:Trigger.AttackerBlockedByCreature.ValidBlocked");
    }
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    let mut node = json!({ "what": what });
    if let Some(valid) = p.get("ValidBlocker") {
        node["by"] = json!(selector_any(ctx, valid)?);
    }
    Ok(json!({ "BecomesBlocked": node }))
}

fn did(ctx: &Ctx, p: &Params, mode: &str, action: &str, subject: Subject) -> Tr<Value> {
    let (player_key, object_key) = match subject {
        Player(key) => (Some(key), None),
        Object(key) => (None, Some(key)),
        Both(player, object) => (Some(player), Some(object)),
    };
    let read: Vec<&str> = player_key.into_iter().chain(object_key).collect();
    keys(p, mode, &read)?;
    let mut node = json!({ "action": action });
    if let Some(key) = player_key {
        let who = player_ref(p.get(key))?;
        if who != "EachPlayer" {
            node["who"] = json!(who);
        }
    }
    if let Some(valid) = object_key.and_then(|key| p.get(key)) {
        node["what"] = json!(selector_any(ctx, valid)?);
    }
    // Becoming monstrous is its own pattern: it carries the X the monstrosity was paid with.
    if action == "BecameMonstrous" {
        let what = node.get("what").cloned().unwrap_or_else(|| json!([]));
        return Ok(json!({ "BecomesMonstrous": { "what": what } }));
    }
    Ok(json!({ "Did": node }))
}

/// Whether a mode of this table holds a player and an object among its subjects; none for any other mode.
pub fn holds(mode: &str) -> Option<(bool, bool)> {
    if mode == "AbilityCast" {
        return Some((true, true));
    }
    let (_, _, subject) = MODES.iter().find(|(name, ..)| *name == mode)?;
    Some(match subject {
        Player(_) => (true, false),
        Object(_) => (false, true),
        Both(..) => (true, true),
    })
}
