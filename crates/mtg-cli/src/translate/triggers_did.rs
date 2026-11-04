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
const MODES: [(&str, &str, Subject); 24] = [
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
    if mode == "AttackerBlockedByCreature" {
        return Some(blocked_by(ctx, p));
    }
    let (_, action, subject) = MODES.iter().find(|(name, ..)| *name == mode)?;
    Some(did(ctx, p, mode, action, *subject))
}

/// `AttackerBlockedByCreature`: an attacker becomes blocked by a creature, once for each such blocker.
fn blocked_by(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "AttackerBlockedByCreature",
        &["ValidCard", "ValidBlocker"],
    )?;
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
    Ok(json!({ "Did": node }))
}

/// Whether a mode of this table holds a player and an object among its subjects; none for any other mode.
pub fn holds(mode: &str) -> Option<(bool, bool)> {
    let (_, _, subject) = MODES.iter().find(|(name, ..)| *name == mode)?;
    Some(match subject {
        Player(_) => (true, false),
        Object(_) => (false, true),
        Both(..) => (true, true),
    })
}
