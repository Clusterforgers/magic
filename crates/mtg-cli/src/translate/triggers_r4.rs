//! Trigger modes for an ability activated (`AbilityCast`, `SpellAbilityCast`) and attackers declared at one player.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::triggers::{keys, player_ref};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The pattern of a mode this module reads; `None` for any other mode.
pub fn pattern(ctx: &Ctx, p: &Params, mode: &str) -> Option<Tr<Value>> {
    Some(match mode {
        "AbilityCast" => ability_cast(ctx, p),
        "SpellAbilityCast" => spell_ability_cast(ctx, p),
        "AttackersDeclaredOneTarget" => attackers_at_one_target(ctx, p, mode),
        _ => return None,
    })
}

/// What a `ValidSA` part asks of the activation: only the words that leave mana abilities out, because the engine logs
/// no activation for a mana ability (it uses no stack, CR 605.3b). `Activated` and `SpellAbility` alone count mana
/// abilities too and stay gaps.
fn activation_word(word: &str, mode: &str) -> Tr<()> {
    match word {
        "SpellAbility.!ManaAbility" | "Activated.!ManaAbility" => Ok(()),
        other => unmapped(format!("key:Trigger.{mode}.ValidSA={other}")),
    }
}

/// "Whenever you activate an ability of a creature": the source is the object, the player is who activates.
fn activation(ctx: &Ctx, p: &Params) -> Tr<Value> {
    let mut node = json!({ "action": "Activated" });
    let who = player_ref(p.get("ValidActivatingPlayer"))?;
    if who != "EachPlayer" {
        node["who"] = json!(who);
    }
    if let Some(valid) = p.get("ValidCard") {
        node["what"] = json!(selector_any(ctx, valid)?);
    }
    Ok(json!({ "Did": node }))
}

fn ability_cast(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "AbilityCast",
        &["ValidCard", "ValidActivatingPlayer", "ValidSA"],
    )?;
    let Some(word) = p.get("ValidSA") else {
        return unmapped("key:Trigger.AbilityCast.ValidSA-missing");
    };
    // A loyalty ability is an activated ability of a planeswalker (CR 606.1): the source is a planeswalker, and no mana
    // ability is one. `+OppCtrl` is an ability an opponent activates.
    let loyalty = match word {
        "Activated.Loyalty" => Some(false),
        "Activated.Loyalty+OppCtrl" => Some(true),
        _ => None,
    };
    let Some(opponent) = loyalty else {
        activation_word(word, "AbilityCast")?;
        return activation(ctx, p);
    };
    let mut found = activation(ctx, p)?;
    let node = &mut found["Did"];
    let walker = json!({"Type": "Planeswalker"});
    match node.get_mut("what").and_then(Value::as_array_mut) {
        Some(what) if !what.contains(&walker) => what.push(walker),
        Some(_) => {}
        None => node["what"] = json!([walker]),
    }
    if opponent {
        if node.get("who").is_some() {
            return unmapped(
                "key:Trigger.AbilityCast.ValidSA=Activated.Loyalty+OppCtrl-and-player",
            );
        }
        node["who"] = json!("Opponents");
    }
    Ok(found)
}

/// "Whenever you cast a spell or activate an ability": either event triggers it alone.
fn spell_ability_cast(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "SpellAbilityCast", &["ValidActivatingPlayer", "ValidSA"])?;
    let Some(words) = p.get("ValidSA") else {
        return unmapped("key:Trigger.SpellAbilityCast.ValidSA-missing");
    };
    let by = player_ref(p.get("ValidActivatingPlayer"))?;
    let mut spell: Option<Vec<Value>> = None;
    let mut activated = false;
    for word in words.split(',') {
        match word {
            "SpellAbility.!ManaAbility" => {
                activation_word(word, "SpellAbilityCast")?;
                activated = true;
                spell.get_or_insert_with(Vec::new);
            }
            "Activated.!ManaAbility" => {
                activation_word(word, "SpellAbilityCast")?;
                activated = true;
            }
            "Spell" => {
                spell.get_or_insert_with(Vec::new);
            }
            other => {
                let Some(rest) = other.strip_prefix("Spell.") else {
                    return unmapped(format!("key:Trigger.SpellAbilityCast.ValidSA={other}"));
                };
                // Two spell parts would be an either-or of selectors: one part is all this reads.
                if spell.is_some() {
                    return unmapped("key:Trigger.SpellAbilityCast.ValidSA-two-spell-parts");
                }
                spell = Some(selector_any(ctx, &format!("Card.{rest}"))?);
            }
        }
    }
    let mut parts = Vec::new();
    if let Some(what) = spell {
        let mut cast = json!({ "by": by });
        if !what.is_empty() {
            cast["what"] = json!(what);
        }
        parts.push(json!({ "SpellCast": cast }));
    }
    if activated {
        let mut node = json!({ "action": "Activated" });
        if by != "EachPlayer" {
            node["who"] = json!(by);
        }
        parts.push(json!({ "Did": node }));
    }
    match parts.len() {
        0 => unmapped("key:Trigger.SpellAbilityCast.ValidSA"),
        1 => Ok(parts.remove(0)),
        _ => Ok(json!({ "AnyOf": parts })),
    }
}

/// "Whenever one or more Dragons you control attack an opponent": the attackers of one player attacked, once for each.
pub(super) fn attackers_at_one_target(ctx: &Ctx, p: &Params, mode: &str) -> Tr<Value> {
    keys(
        p,
        mode,
        &[
            "AttackingPlayer",
            "ValidAttackers",
            "AttackedTarget",
            "ValidAttackersAmount",
        ],
    )?;
    // A planeswalker or battle attacked is no player attacked: the player word alone is read.
    let target = p.get("AttackedTarget");
    let Ok(at) = player_ref(target) else {
        return unmapped(format!(
            "key:Trigger.{mode}.AttackedTarget={}",
            target.unwrap_or_default()
        ));
    };
    let word = p.get("AttackingPlayer");
    let Ok(by) = player_ref(word) else {
        return unmapped(format!(
            "key:Trigger.{mode}.AttackingPlayer={}",
            word.unwrap_or_default()
        ));
    };
    let mut what = selector_any(ctx, p.get("ValidAttackers").unwrap_or("Creature"))?;
    // The attackers are the attacking player's own creatures.
    let own = json!({ "ControlledBy": by });
    if by != "EachPlayer" && !what.contains(&own) {
        what.push(own);
    }
    let mut node = json!({ "what": what, "at": at });
    match p.get("ValidAttackersAmount") {
        None => {}
        Some(amount) => match amount
            .strip_prefix("GE")
            .and_then(|n| n.parse::<u32>().ok())
        {
            Some(least) if least >= 2 => node["at_least"] = json!(least),
            _ => return unmapped(format!("key:Trigger.{mode}.ValidAttackersAmount")),
        },
    }
    Ok(json!({ "GroupAttacksAt": node }))
}
