//! `Vote` on named choices: each player votes, then the choices run by their votes.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, only, unmapped};
use super::effects::chain_at;
use crate::forge::script::Params;

/// A choice of the vote: the name its votes are bound to and the steps it runs.
struct Choice {
    name: String,
    part: Params,
}

/// The name a choice is bound to: its description in lower case, words of letters joined by `_` ("Redhorn Pass" is
/// `redhorn_pass`, which the vote shows as "redhorn pass"); at most three words, so a sentence stays a gap.
fn name_of(part: &Params) -> Tr<String> {
    let words: Vec<String> = part
        .get("SpellDescription")
        .unwrap_or("")
        .split(' ')
        .map(str::to_lowercase)
        .collect();
    let name = words.join("_");
    if words.len() > 3
        || words
            .iter()
            .any(|word| word.is_empty() || !word.chars().all(|c| c.is_ascii_lowercase()))
    {
        return unmapped("key:Vote.Choices-name");
    }
    Ok(name)
}

fn votes(name: &str) -> Value {
    json!({"Players": {"Bound": name}})
}

/// `Vote` of every player on the named `Choices$`: the winner runs, or each choice runs by its votes.
pub fn vote(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    if p.has("VoteCard") {
        return e14c_vote_card(ctx, p);
    }
    only(
        p,
        "Vote",
        &[
            "Defined",
            "Choices",
            "VoteTiedAbility",
            "StoreVoteNum",
            "EachVote",
            "Secretly",
        ],
    )?;
    if p.get("Defined") != Some("Player") {
        return unmapped("key:Vote.Defined");
    }
    let Some(list) = p.get("Choices") else {
        return unmapped("key:Vote.no-Choices");
    };
    let mut choices = Vec::new();
    let mut ids = Vec::new();
    for id in list.split(',') {
        let part = ctx.sub(id)?;
        choices.push(Choice {
            name: name_of(&part)?,
            part,
        });
        ids.push(id.to_owned());
    }
    let mut seen = std::collections::BTreeSet::new();
    if !choices
        .iter()
        .all(|choice| seen.insert(choice.name.clone()))
    {
        return unmapped("key:Vote.Choices-name");
    }
    let names: Vec<&str> = choices.iter().map(|c| c.name.as_str()).collect();
    let mut vote = json!({"options": {"Named": names}});
    if p.get("Secretly") == Some("True") {
        vote["secret"] = json!(true);
    }
    let mut effects = vec![json!({ "Vote": vote })];
    let store = p.get("StoreVoteNum") == Some("True");
    let each = p.get("EachVote") == Some("True");
    let tied = p.get("VoteTiedAbility");
    match (store, each, tied) {
        // A choice whose later lines run another choice (Truth or Consequences) would run that one under its own votes.
        _ if e14c_chains_into_choice(ctx, &choices, &ids) => {
            return unmapped("key:Vote.Choices-chained");
        }
        (true, false, None) => {
            for choice in &choices {
                ctx.vote_num = Some(choice.name.clone());
                let steps = chain_at(ctx, &choice.part, false);
                ctx.vote_num = None;
                let steps = steps?;
                // A choice that ignores its votes would run whether or not anyone chose it.
                if !json!(steps)
                    .to_string()
                    .contains(&votes(&choice.name).to_string())
                {
                    return unmapped("key:Vote.StoreVoteNum-unread");
                }
                effects.extend(steps);
            }
        }
        (false, true, None) => {
            for choice in &choices {
                let outer = ctx.remembered.replace(json!({"Bound": "voter"}));
                let steps = chain_at(ctx, &choice.part, false);
                ctx.remembered = outer;
                effects.push(json!({"RepeatEach": {
                    "over": {"Bound": choice.name}, "as": "voter", "effects": steps?,
                }}));
            }
        }
        (false, false, Some(tie)) if choices.len() == 2 => {
            let Some(at) = ids.iter().position(|id| id == tie) else {
                return unmapped("key:Vote.VoteTiedAbility");
            };
            let (other, tied) = (&choices[1 - at], &choices[at]);
            let more = chain_at(ctx, &other.part, false)?;
            let otherwise = chain_at(ctx, &tied.part, false)?;
            effects.push(json!({"If": {
                "condition": {"Compare": {
                    "left": votes(&other.name), "op": "Gt", "right": votes(&tied.name),
                }},
                "then": more,
                "otherwise": otherwise,
            }}));
        }
        // "Each choice with the most votes or tied for most votes": a choice runs when no other got more votes.
        (false, false, None) if choices.len() >= 2 => {
            effects.extend(e14c_most_votes(ctx, &choices)?);
        }
        _ => return unmapped("key:Vote.shape"),
    }
    Ok(effects)
}

/// Each choice runs when no other choice got more votes than it.
fn e14c_most_votes(ctx: &mut Ctx, choices: &[Choice]) -> Tr<Vec<Value>> {
    let mut effects = Vec::new();
    for (index, choice) in choices.iter().enumerate() {
        let most: Vec<Value> = choices
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(_, other)| {
                json!({"Compare": {
                    "left": votes(&choice.name), "op": "Ge", "right": votes(&other.name),
                }})
            })
            .collect();
        let steps = chain_at(ctx, &choice.part, false)?;
        effects.push(json!({"If": {"condition": {"All": most}, "then": steps}}));
    }
    Ok(effects)
}

/// Whether the `SubAbility$` lines after some choice name another choice of the vote; the walk stops after a few lines.
fn e14c_chains_into_choice(ctx: &mut Ctx, choices: &[Choice], ids: &[String]) -> bool {
    choices.iter().any(|choice| {
        let mut next = choice.part.get("SubAbility").map(str::to_owned);
        for _ in 0..16 {
            let Some(name) = next.take() else {
                return false;
            };
            if ids.contains(&name) {
                return true;
            }
            next = ctx
                .sub(&name)
                .ok()
                .and_then(|part| part.get("SubAbility").map(str::to_owned));
        }
        false
    })
}

/// The name the objects with the most votes are bound to.
const E14C_VOTED: &str = "voted";

/// `Vote` of every player on an object `VoteCard$` matches, on the battlefield or in the `Zone$` named: the objects with
/// the most votes, all of them when tied, are what `VoteSubAbility$` reads as `Remembered`. A vote for up to one, or one
/// whose tally is read per object, stays a gap.
fn e14c_vote_card(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Vote",
        &[
            "Defined",
            "VoteCard",
            "VoteMessage",
            "VoteSubAbility",
            "Zone",
            "Secretly",
        ],
    )?;
    if p.get("Defined") != Some("Player") {
        return unmapped("key:Vote.Defined");
    }
    let (Some(valid), Some(then)) = (p.get("VoteCard"), p.get("VoteSubAbility")) else {
        return unmapped("key:Vote.VoteCard-shape");
    };
    // The prompt shown while voting; the engine words its own.
    let _ = p.get("VoteMessage");
    let mut of = super::valid::selector_any(ctx, valid)?;
    match p.get("Zone") {
        None | Some("Battlefield") => of.push(json!({"In": "Battlefield"})),
        Some("Graveyard") => of.push(json!({"In": "Graveyard"})),
        Some(other) => return unmapped(format!("key:Vote.Zone={other}")),
    }
    let mut vote = json!({"options": {"Of": {"Each": of}}, "as": E14C_VOTED});
    if p.get("Secretly") == Some("True") {
        vote["secret"] = json!(true);
    }
    let part = ctx.sub(then)?;
    let outer = ctx.remembered.replace(json!({"Bound": E14C_VOTED}));
    let steps = chain_at(ctx, &part, false);
    ctx.remembered = outer;
    let mut effects = vec![json!({ "Vote": vote })];
    effects.extend(steps?);
    Ok(effects)
}
