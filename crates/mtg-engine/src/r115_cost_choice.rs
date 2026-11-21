//! A cost of an activated ability with alternatives chosen as it is announced (CR 602.2b, 601.2b): "{3}, {T}, Sacrifice
//! an artifact or discard a nonland card: ...". Each `CostItem::OneOf` of the cost is one choice; a combination of the
//! choices is a variant, numbered as `required.rs` numbers those of a spell (the last choice varies fastest), kept in
//! `CostTaken::variant`. Every reader of the cost reads the ability with its chosen variant in place of the choices: the
//! offer of the ability (`priority::activation`, ready when one variant is), the announcement (`announce::rules_of`) and
//! the payment (`cast.rs`).

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::cost::{Cost, CostItem};

/// The most variants an activation is offered; a cost with more is left as it is (and never paid).
const MOST: usize = 64;

/// The variants of `cost`, each the cost with every choice replaced by the parts of one of its alternatives; `None` when
/// the cost has no choice.
pub(crate) fn r115_variants(cost: &Cost) -> Option<Vec<Cost>> {
    if !cost.0.iter().any(|item| matches!(item, CostItem::OneOf(_))) {
        return None;
    }
    let mut found: Vec<Vec<CostItem>> = vec![Vec::new()];
    for item in &cost.0 {
        match item {
            CostItem::OneOf(costs) => {
                let mut next = Vec::with_capacity(found.len() * costs.len());
                for before in &found {
                    for option in costs {
                        let mut items = before.clone();
                        items.extend(option.0.iter().cloned());
                        next.push(items);
                    }
                }
                found = next;
            }
            other => found.iter_mut().for_each(|items| items.push(other.clone())),
        }
        if found.len() > MOST {
            return None;
        }
    }
    Some(found.into_iter().map(Cost).collect())
}

/// The parts of the alternatives `variant` takes, in the order of the choices: what the player is shown for it.
pub(crate) fn r115_chosen_parts(cost: &Cost, variant: usize) -> Cost {
    let choices: Vec<&Vec<Cost>> = cost
        .0
        .iter()
        .filter_map(|item| match item {
            CostItem::OneOf(costs) => Some(costs),
            _ => None,
        })
        .collect();
    let mut rest = variant;
    let mut picked = Vec::with_capacity(choices.len());
    for costs in choices.iter().rev() {
        let count = costs.len().max(1);
        picked.push(costs.get(rest % count));
        rest /= count;
    }
    picked.reverse();
    Cost(
        picked
            .into_iter()
            .flatten()
            .flat_map(|cost| cost.0.iter().cloned())
            .collect(),
    )
}

/// `rules` as the variants of its cost: one activated ability for each, `None` for an ability whose cost has no choice.
pub(crate) fn r115_ability_variants(rules: &Ability) -> Option<Vec<Ability>> {
    let Ability::Activated { cost, .. } = rules else {
        return None;
    };
    let variants = r115_variants(cost)?;
    Some(
        variants
            .into_iter()
            .map(|chosen| with_cost(rules, chosen))
            .collect(),
    )
}

/// `rules` with the variant `variant` of its cost in place of the choices; `rules` itself when its cost has none.
pub(crate) fn r115_as_chosen(rules: Arc<Ability>, variant: u8) -> Arc<Ability> {
    let Ability::Activated { cost, .. } = &*rules else {
        return rules;
    };
    match r115_variants(cost).and_then(|all| all.into_iter().nth(usize::from(variant))) {
        Some(chosen) => Arc::new(with_cost(&rules, chosen)),
        None => rules,
    }
}

/// The activated ability `rules` with `cost` as its cost.
fn with_cost(rules: &Ability, cost: Cost) -> Ability {
    let mut changed = rules.clone();
    if let Ability::Activated { cost: slot, .. } = &mut changed {
        *slot = cost;
    }
    changed
}

#[cfg(test)]
mod tests {
    //! Goal: a cost with choices has one variant per combination of its alternatives, in the order `required.rs` numbers
    //! those of a spell, and the chosen one replaces the choices for every reader.
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;

    fn cost(json: &str) -> Cost {
        serde_json::from_str(json).unwrap()
    }

    #[rstest]
    #[case::no_choice(r#"["Tap"]"#, None)]
    #[case::one_choice(
        r#"["Tap",{"OneOf":[[{"Sacrifice":{"filter":[{"Type":"Artifact"}]}}],[{"PayLife":2}]]}]"#,
        Some(vec![r#"["Tap",{"Sacrifice":{"filter":[{"Type":"Artifact"}]}}]"#, r#"["Tap",{"PayLife":2}]"#])
    )]
    #[case::two_choices(
        r#"[{"OneOf":[[{"PayLife":1}],[{"PayLife":2}]]},{"OneOf":[[{"PayLife":3}],["Tap"]]}]"#,
        Some(vec![
            r#"[{"PayLife":1},{"PayLife":3}]"#,
            r#"[{"PayLife":1},"Tap"]"#,
            r#"[{"PayLife":2},{"PayLife":3}]"#,
            r#"[{"PayLife":2},"Tap"]"#,
        ])
    )]
    fn r115_variants_are_the_combinations_of_the_alternatives(
        #[case] printed: &str,
        #[case] expected: Option<Vec<&str>>,
    ) {
        let printed = cost(printed);
        let expected = expected.map(|all| all.into_iter().map(cost).collect::<Vec<_>>());
        assert_eq!(r115_variants(&printed), expected);
        if let Some(all) = r115_variants(&printed) {
            for (variant, chosen) in all.iter().enumerate() {
                let parts = r115_chosen_parts(&printed, variant);
                assert!(parts.0.iter().all(|item| chosen.0.contains(item)));
                assert_ne!(parts, Cost(Vec::new()));
            }
        }
    }
}
