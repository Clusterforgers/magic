//! An ability as steps: what the game asks, checks and does for it, from a walk of the whole ability.

use super::phrase::{self, Ctx};
use super::{Step, effects, parts};
use crate::ability::{Ability, CostChange, Limit, LoyaltyCost, Timing};
use crate::refs::{Selector, ZoneKind};
use crate::value::{Amount, Condition};

/// What kind of ability it is, in a few words.
pub(super) fn label(ability: &Ability) -> String {
    let ctx = Ctx::none();
    match ability {
        Ability::Spell { .. } => "spell".to_owned(),
        Ability::Activated { .. } if ability.is_mana_ability() => "mana ability".to_owned(),
        Ability::Activated { .. } => "activated ability".to_owned(),
        Ability::Loyalty { .. } => "loyalty ability".to_owned(),
        Ability::Triggered { .. } => "triggered ability".to_owned(),
        Ability::Static { defining: true, .. } => "characteristic-defining ability".to_owned(),
        Ability::Static { .. } => "static ability".to_owned(),
        Ability::Rule { .. } => "rule of the game".to_owned(),
        Ability::Replacement { .. } => "replacement effect".to_owned(),
        Ability::CostModifier { .. } => "cost change".to_owned(),
        Ability::AdditionalCost { .. } => "additional cost".to_owned(),
        Ability::FreeCast { .. } => "alternative cost".to_owned(),
        Ability::CastOnly { .. } => "casting restriction".to_owned(),
        Ability::Keyword(keyword) => format!("keyword {}", parts::keyword(&ctx, keyword)),
        Ability::Manual { .. } => "played by hand".to_owned(),
    }
}

/// The steps of one ability.
pub(super) fn ability(ability: &Ability) -> Vec<Step> {
    let mut out = Vec::new();
    ability_into(ability, 0, &mut out);
    out
}

/// "choose: target creature; any target": what the game asks for as the ability is put on the stack.
pub(super) fn targets(ctx: &Ctx, depth: u8, out: &mut Vec<Step>) {
    if ctx.targets.is_empty() {
        return;
    }
    let chosen: Vec<String> = ctx
        .targets
        .iter()
        .map(|spec| phrase::spec_text(ctx, spec))
        .collect();
    out.push(Step::new(depth, format!("choose: {}", chosen.join("; "))));
}

/// The change a loyalty ability makes: `+2`, `-3`, `0`, `-X`.
fn loyalty(cost: LoyaltyCost) -> String {
    match cost {
        LoyaltyCost::Plus(by) => format!("+{by}"),
        LoyaltyCost::Minus(by) => format!("-{by}"),
        LoyaltyCost::Zero => "0".to_owned(),
        LoyaltyCost::MinusX => "-X".to_owned(),
    }
}

/// "while: ...": a condition that gates an ability.
fn gate(ctx: &Ctx, word: &str, condition: Option<&Condition>, depth: u8, out: &mut Vec<Step>) {
    if let Some(condition) = condition {
        out.push(Step::new(
            depth,
            format!("{word}: {}", phrase::condition(ctx, condition)),
        ));
    }
}

/// "limit: once each turn" and "from: a graveyard": where and how often.
fn bounds(limit: Option<Limit>, from: ZoneKind, depth: u8, out: &mut Vec<Step>) {
    if limit.is_some() {
        out.push(Step::new(depth, "limit: once each turn"));
    }
    if from != ZoneKind::Battlefield {
        out.push(Step::new(
            depth,
            format!("works from: {}", phrase::zone_text(from)),
        ));
    }
}

/// Adds the steps of an ability at a depth.
pub(super) fn ability_into(ability: &Ability, depth: u8, out: &mut Vec<Step>) {
    match ability {
        Ability::Spell {
            targets: specs,
            effects: list,
        } => {
            let ctx = Ctx::new(specs);
            targets(&ctx, depth, out);
            effects::list(&ctx, list, depth, out);
        }
        Ability::Activated {
            cost,
            timing,
            condition,
            limit,
            from,
            targets: specs,
            effects: list,
        } => {
            let ctx = Ctx::new(specs);
            out.push(Step::new(
                depth,
                format!("cost: {}", parts::cost(&ctx, cost)),
            ));
            if *timing == Timing::Sorcery {
                out.push(Step::new(depth, "only as a sorcery"));
            }
            gate(&ctx, "only if", condition.as_ref(), depth, out);
            bounds(*limit, *from, depth, out);
            targets(&ctx, depth, out);
            effects::list(&ctx, list, depth, out);
        }
        Ability::Loyalty {
            cost,
            targets: specs,
            effects: list,
        } => {
            let ctx = Ctx::new(specs);
            out.push(Step::new(depth, format!("loyalty: {}", loyalty(*cost))));
            targets(&ctx, depth, out);
            effects::list(&ctx, list, depth, out);
        }
        Ability::Triggered {
            when,
            intervening_if,
            limit,
            from,
            targets: specs,
            effects: list,
        } => {
            let ctx = Ctx::new(specs);
            out.push(Step::new(
                depth,
                format!("when: {}", parts::trigger(&ctx, when)),
            ));
            gate(&ctx, "only if", intervening_if.as_ref(), depth, out);
            bounds(*limit, *from, depth, out);
            targets(&ctx, depth, out);
            effects::list(&ctx, list, depth, out);
        }
        _ => other(ability, depth, out),
    }
}

/// The abilities that are not an action: static abilities, rules, replacements, costs and keywords.
fn other(ability: &Ability, depth: u8, out: &mut Vec<Step>) {
    let ctx = Ctx::none();
    match ability {
        Ability::Static {
            affected,
            mods,
            condition,
            defining,
            from,
        } => {
            out.push(Step::new(
                depth,
                format!("affects: {}", parts::affected(&ctx, affected)),
            ));
            if *defining {
                out.push(Step::new(
                    depth,
                    "defines its own characteristics, in every zone",
                ));
            }
            gate(&ctx, "while", condition.as_ref(), depth, out);
            bounds(None, *from, depth, out);
            parts::modifications(&ctx, mods, depth, out);
        }
        Ability::Rule {
            rule,
            condition,
            from,
        } => {
            out.push(Step::new(depth, parts::rule(&ctx, rule)));
            gate(&ctx, "while", condition.as_ref(), depth, out);
            bounds(None, *from, depth, out);
        }
        Ability::Replacement {
            when,
            by,
            condition,
            from,
        } => {
            out.push(Step::new(
                depth,
                format!("if {}:", parts::replace_when(&ctx, when)),
            ));
            parts::replace_by(&ctx, by, depth + 1, out);
            gate(&ctx, "while", condition.as_ref(), depth, out);
            bounds(None, *from, depth, out);
        }
        _ => costs(ability, depth, out),
    }
}

/// The abilities about the cost of casting a spell, and the keywords and clauses left to the table.
fn costs(ability: &Ability, depth: u8, out: &mut Vec<Step>) {
    let ctx = Ctx::none();
    match ability {
        Ability::CostModifier {
            applies_to,
            change,
            mana,
            per,
            condition,
        } => {
            let more = if *change == CostChange::Increase {
                "more"
            } else {
                "less"
            };
            let each = if *per == Amount::one() {
                String::new()
            } else {
                format!(", times {}", phrase::amount(&ctx, per))
            };
            let spells = if *applies_to == Selector::this() {
                "this spell costs".to_owned()
            } else {
                format!("{} cost", phrase::selectors(&ctx, applies_to))
            };
            out.push(Step::new(
                depth,
                format!("{spells} {mana} {more} to cast{each}"),
            ));
            gate(&ctx, "while", condition.as_ref(), depth, out);
        }
        Ability::AdditionalCost { cost, required } => {
            let how = if *required { "must pay" } else { "may pay" };
            out.push(Step::new(
                depth,
                format!(
                    "as an additional cost, you {how}: {}",
                    parts::cost(&ctx, cost)
                ),
            ));
        }
        Ability::FreeCast { condition, cost } => {
            let by = cost.as_ref().map_or_else(String::new, |cost| {
                format!(" by paying {}", parts::cost(&ctx, cost))
            });
            out.push(Step::new(
                depth,
                format!("may be cast without paying its mana cost{by}"),
            ));
            gate(&ctx, "if", condition.as_ref(), depth, out);
        }
        Ability::CastOnly { condition } => {
            out.push(Step::new(
                depth,
                format!(
                    "can be cast only if: {}",
                    phrase::condition(&ctx, condition)
                ),
            ));
        }
        Ability::Keyword(keyword) => out.push(Step::new(
            depth,
            format!("has {}", parts::keyword(&ctx, keyword)),
        )),
        Ability::Manual { reason, .. } => {
            out.push(Step::new(depth, format!("played by hand: {reason}")));
        }
        // The actions are worded by `ability_into`, the rules and replacements by `other`.
        Ability::Spell { .. }
        | Ability::Activated { .. }
        | Ability::Loyalty { .. }
        | Ability::Triggered { .. }
        | Ability::Static { .. }
        | Ability::Rule { .. }
        | Ability::Replacement { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every kind of ability becomes a non-empty list of steps that names what the game does, in the order the game does it.
    use rstest::rstest;

    use super::*;

    fn shown(json: &str) -> Vec<String> {
        let parsed: Ability = serde_json::from_str(json).unwrap();
        ability(&parsed)
            .iter()
            .map(|step| format!("{}{}", "  ".repeat(usize::from(step.depth)), step.text))
            .collect()
    }

    #[rstest]
    #[case::bolt(
        r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#,
        &["choose: any target", "deal 3 damage to any target"]
    )]
    #[case::trigger(
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{}}]}}"#,
        &["when: this permanent enters the battlefield", "draw a card"]
    )]
    #[case::intervening_if(
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":40}},"effects":[{"WinGame":{}}]}}"#,
        &["when: the beginning of your upkeep", "only if: your life total is at least 40", "win the game"]
    )]
    #[case::activated(
        r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"timing":"Sorcery","limit":"OncePerTurn","effects":[{"Draw":{}}]}}"#,
        &["cost: {T}, sacrifice this permanent", "only as a sorcery", "limit: once each turn", "draw a card"]
    )]
    #[case::x_cost(
        r#"{"Activated":{"cost":"{X}{R}","effects":[{"DealDamage":{"to":"This","amount":"X"}}]}}"#,
        &["cost: {X}{R}", "deal X damage to this permanent"]
    )]
    #[case::loyalty(r#"{"Loyalty":{"cost":{"Minus":2},"effects":[{"Draw":{}}]}}"#, &["loyalty: -2", "draw a card"])]
    #[case::keyword(r#"{"Keyword":"Flying"}"#, &["has flying"])]
    #[case::manual(r#"{"Manual":{"reason":"a clause the engine lacks","span":{"start":0,"end":4}}}"#, &["played by hand: a clause the engine lacks"])]
    fn an_ability_is_a_list_of_steps(#[case] json: &str, #[case] expected: &[&str]) {
        assert_eq!(shown(json), expected);
    }

    #[test]
    fn every_kind_of_ability_has_a_label_and_steps() {
        for json in [
            r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#,
            r#"{"Rule":{"rule":{"CantGainLife":"Opponents"}}}"#,
            r#"{"Replacement":{"when":{"Draw":{"who":"You"}},"by":"Prevent"}}"#,
            r#"{"CostModifier":{"applies_to":[{"Type":"Creature"}],"change":"Reduce","mana":"{1}"}}"#,
            r#"{"AdditionalCost":{"cost":[{"PayLife":2}],"required":true}}"#,
            r#"{"FreeCast":{}}"#,
            r#"{"CastOnly":{"condition":"YourTurn"}}"#,
        ] {
            let parsed: Ability = serde_json::from_str(json).unwrap();
            assert!(!label(&parsed).is_empty(), "{json}");
            let steps = ability(&parsed);
            assert!(
                !steps.is_empty() && steps.iter().all(|step| !step.text.trim().is_empty()),
                "{json}"
            );
        }
    }
}
