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

/// The name a choice is bound to: its description in lower case, one word of letters.
fn name_of(part: &Params) -> Tr<String> {
    let name = part.get("SpellDescription").unwrap_or("").to_lowercase();
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase()) {
        return unmapped("key:Vote.Choices-name");
    }
    Ok(name)
}

fn votes(name: &str) -> Value {
    json!({"Players": {"Bound": name}})
}

/// `Vote` of every player on the named `Choices$`: the winner runs, or each choice runs by its votes.
pub fn vote(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
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
        _ => return unmapped("key:Vote.shape"),
    }
    Ok(effects)
}
