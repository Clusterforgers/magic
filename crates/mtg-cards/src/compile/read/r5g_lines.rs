//! Keyword lines a proposal writes as the abilities they stand for: Reinforce (CR 702.77) and Forecast (CR 702.57) are
//! activated abilities that work from the hand, Escalate (CR 702.120) is the cost a modal spell adds for each mode beyond
//! the first. The data is the expanded ability; this writes the keyword line the card prints, only for the ability the
//! keyword means.

use crate::ability::{Ability, Activators, Limit, Timing};
use crate::cost::{Cost, CostItem};
use crate::effect::Effect;
use crate::refs::{Pred, Selector, ZoneKind};
use crate::trigger::Step;
use crate::value::{Amount, Condition};
use serde_json::json;

/// "Reinforce 2—{1}{G}": the line of an ability that is exactly "{1}{G}, Discard this card: Put two +1/+1 counters on
/// target creature", activated from the hand at instant speed; `None` for any other ability.
pub fn reinforce_line(ability: &Ability) -> Option<String> {
    let Ability::Activated { cost, effects, .. } = ability else {
        return None;
    };
    let [CostItem::Mana(mana), CostItem::DiscardSelf] = cost.0.as_slice() else {
        return None;
    };
    let [Effect::PutCounters { count, .. }] = effects.as_slice() else {
        return None;
    };
    let number = match count {
        Amount::Fixed(number) if *number > 0 => number.to_string(),
        Amount::X if mana.x() > 0 => "X".to_owned(),
        _ => return None,
    };
    let count = serde_json::to_value(count).ok()?;
    let printed = serde_json::to_value(mana).ok()?;
    let printed = printed.as_str()?;
    let expected: Ability = serde_json::from_value(json!({"Activated": {
        "cost": [{"Mana": printed}, "DiscardSelf"],
        "effects": [{"PutCounters": {"count": count, "counter": "+1/+1", "on": {"Target": 0}}}],
        "from": "Hand",
        "targets": [{"what": {"Object": [{"Type": "Creature"}]}}]}}))
    .ok()?;
    (expected == *ability).then(|| format!("Reinforce {number}\u{2014}{printed}"))
}

/// "Forecast — {1}{W}, Reveal this card from your hand: <effect>": the lines of an ability activated from the hand only
/// during its controller's upkeep and once each turn, whose cost reveals the card itself; each is a wording the writers
/// give the same ability on the battlefield without the restriction. Empty for any other ability.
pub fn forecast_lines(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ability::Activated {
        cost,
        timing: Timing::Instant,
        condition: Some(condition),
        limit: Some(Limit::OncePerTurn),
        by: Activators::Controller,
        from: ZoneKind::Hand,
        targets,
        effects,
    } = ability
    else {
        return Vec::new();
    };
    let upkeep = Condition::All(vec![Condition::YourTurn, Condition::Step(Step::Upkeep)]);
    // The reveal of the card itself is the last part of a forecast cost; the parts before it are written as any cost.
    let Some((
        CostItem::RevealFromHand {
            filter,
            count: Amount::Fixed(1),
        },
        rest,
    )) = cost.0.split_last()
    else {
        return Vec::new();
    };
    if *condition != upkeep || *filter != Selector(vec![Pred::This]) || rest.is_empty() {
        return Vec::new();
    }
    let plain = Ability::Activated {
        cost: Cost(rest.to_vec()),
        timing: Timing::Instant,
        condition: None,
        limit: None,
        by: Activators::Controller,
        from: ZoneKind::Battlefield,
        targets: targets.clone(),
        effects: effects.clone(),
    };
    written(&plain)
        .into_iter()
        .filter_map(|text| {
            let (paid, done) = text.split_once(": ")?;
            Some(format!(
                "Forecast \u{2014} {paid}, Reveal this card from your hand: {done}"
            ))
        })
        .collect()
}

/// The cost Escalate adds for each mode beyond the first, of a spell that is one choice of several modes.
pub fn escalate_cost(ability: &Ability) -> Option<&Cost> {
    let Ability::Spell { effects, .. } = ability else {
        return None;
    };
    let [
        Effect::Modal {
            extra_mode_cost: Some(cost),
            ..
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    Some(cost)
}

/// Whether a keyword line is "Escalate {1}" or "Escalate—Discard a card." for exactly this cost.
pub fn escalate_reads(line: &str, cost: &Cost) -> bool {
    let Some(rest) = line.strip_prefix("Escalate") else {
        return false;
    };
    let rest = rest
        .strip_prefix('\u{2014}')
        .or_else(|| rest.strip_prefix(' '))
        .unwrap_or(rest)
        .trim()
        .trim_end_matches('.');
    super::cost_in_words(rest).is_some_and(|read| read == *cost)
}
