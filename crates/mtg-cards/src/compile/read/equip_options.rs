//! Equip with options (CR 702.6): the activated abilities that attach an Equipment stand for one keyword line. The line
//! words each way to pay ("Equip {2} or {B}", "Equip\u{2014}Pay {3} or discard a card."), a limit ("Activate only once each
//! turn.") and a reduction ("This ability costs {1} less to activate for each other Equipment you control.").

use super::costs::text as cost_text;
use crate::ability::{Ability, CostChange, Limit, Timing};
use crate::cost::{Cost, CostItem};
use crate::refs::{Pred, Ref};
use crate::value::{Amount, Condition};
use serde_json::json;

/// How an equip ability pays: plain mana, or a cost with the words that follow "Equip\u{2014}".
enum Way {
    Mana(String),
    Other(String),
}

/// What the abilities that follow the equip ability share with it.
struct Shape {
    limit: Option<Limit>,
    reduction: Option<String>,
}

/// The way to pay of an ability that attaches the Equipment to a creature you control, with its limit and reduction.
fn way(ability: &Ability) -> Option<(Way, Shape)> {
    let Ability::Activated {
        cost,
        timing: Timing::Sorcery,
        condition: None,
        limit,
        by,
        from,
        targets,
        effects,
    } = ability
    else {
        return None;
    };
    let attach = json!([{"Attach": {"what": "This", "to": {"Target": 0}}}]);
    let creature = json!([{"what": {"Object": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}]);
    if serde_json::to_value(effects).ok()? != attach
        || serde_json::to_value(targets).ok()? != creature
        || !by.is_controller()
        || !from.is_battlefield()
        || !matches!(limit, None | Some(Limit::OncePerTurn))
    {
        return None;
    }
    let (paid, reduction) = split(cost)?;
    let way = match paid.0.as_slice() {
        [CostItem::Mana(mana)] => Way::Mana(mana.to_string()),
        _ => Way::Other(cost_text(&paid)?),
    };
    Some((
        way,
        Shape {
            limit: *limit,
            reduction,
        },
    ))
}

/// The cost without its reduction, and the sentence that words the reduction.
fn split(cost: &Cost) -> Option<(Cost, Option<String>)> {
    let mut paid = Vec::new();
    let mut reduction = None;
    for item in &cost.0 {
        match item {
            CostItem::Changed {
                change: CostChange::Reduce,
                mana,
                per,
                condition: None,
            } if reduction.is_none() => reduction = Some(reduce(&mana.to_string(), per)?),
            CostItem::Changed { .. } => return None,
            other => paid.push(other.clone()),
        }
    }
    Some((Cost(paid), reduction))
}

/// "This ability costs {1} less to activate for each other Equipment you control."
fn reduce(mana: &str, per: &Amount) -> Option<String> {
    let head = "This ability costs";
    match per {
        Amount::Count(selector) => {
            let [
                Pred::Subtype(kind),
                Pred::ControlledBy(Ref::You),
                Pred::Other,
            ] = selector.0.as_slice()
            else {
                return None;
            };
            Some(format!(
                "{head} {mana} less to activate for each other {} you control.",
                kind.0
            ))
        }
        Amount::When {
            condition,
            then,
            otherwise,
        } if **otherwise == Amount::Fixed(0) => {
            let (Amount::Fixed(times), "{1}") = (&**then, mana) else {
                return None;
            };
            let Condition::Compare { left, op, right } = &**condition else {
                return None;
            };
            let hand = serde_json::to_value(left).ok()?
                == json!({"Count": [{"OwnedBy": "You"}, {"In": "Hand"}]});
            let few = serde_json::to_value(op).ok()? == json!("Le")
                && serde_json::to_value(right).ok()? == json!(1);
            (hand && few).then(|| {
                format!(
                    "{head} {{{times}}} less to activate if you have one or fewer cards in hand."
                )
            })
        }
        _ => None,
    }
}

/// "Pay {3} or discard a card": the first word of an alternative in lower case.
fn lowered(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The keyword line the abilities from the first one on write, and how many of them it takes: the equip abilities, one for
/// each way to pay, when one has an option; `None` for a plain equip, which is the keyword.
pub fn equip_line(abilities: &[Ability]) -> Option<(String, usize)> {
    let mut ways = Vec::new();
    let mut shape: Option<Shape> = None;
    for ability in abilities {
        let Some((next, found)) = way(ability) else {
            break;
        };
        match &shape {
            Some(first) if first.limit != found.limit || first.reduction != found.reduction => {
                break;
            }
            Some(_) => {}
            None => shape = Some(found),
        }
        ways.push(next);
    }
    let shape = shape?;
    let count = ways.len();
    if count > 2 || (count == 2 && (shape.limit.is_some() || shape.reduction.is_some())) {
        return None;
    }
    let mut line = match ways.as_slice() {
        [Way::Mana(only)] => format!("Equip {only}"),
        [Way::Mana(first), Way::Mana(second)] => format!("Equip {first} or {second}"),
        [Way::Mana(first), Way::Other(second)] => {
            format!("Equip\u{2014}Pay {first} or {}.", lowered(second))
        }
        [Way::Other(only)] => format!("Equip\u{2014}{only}."),
        _ => return None,
    };
    if shape.limit.is_none() && shape.reduction.is_none() && count == 1 {
        return None;
    }
    for sentence in [
        shape
            .limit
            .map(|_| "Activate only once each turn.".to_owned()),
        shape.reduction,
    ]
    .into_iter()
    .flatten()
    {
        if !line.ends_with('.') {
            line.push('.');
        }
        line.push(' ');
        line.push_str(&sentence);
    }
    Some((line, count))
}
