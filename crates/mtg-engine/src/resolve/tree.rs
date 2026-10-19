//! Positions in the effect tree.
//!
//! A position is a path of indices. `[2]` is the third effect of the list. An effect that holds lists of
//! effects (`If`, `May`, `Pay`, `Modal`) is entered by a branch: `[2, 1, 0]` is the first effect of the second
//! list of the third effect (the `otherwise` of an `If`, the `unpaid` of a `Pay`, the second mode of a `Modal`).

use mtg_cards::effect::Effect;

use super::Next;

/// The lists of effects inside an effect, in branch order.
fn branches(effect: &Effect) -> Vec<&[Effect]> {
    match effect {
        Effect::If {
            then, otherwise, ..
        } => vec![then, otherwise],
        Effect::May { effects, .. } => vec![effects],
        Effect::Pay { paid, unpaid, .. } => vec![paid, unpaid],
        Effect::Modal { modes, .. } => modes.iter().map(|mode| mode.effects.as_slice()).collect(),
        _ => Vec::new(),
    }
}

/// The effect at `position`, if there is one.
pub(crate) fn effect_at<'a>(root: &'a [Effect], position: &[usize]) -> Option<&'a Effect> {
    let mut list = root;
    let mut path = position;
    loop {
        let (&index, rest) = path.split_first()?;
        let effect = list.get(index)?;
        if rest.is_empty() {
            return Some(effect);
        }
        let (&branch, deeper) = rest.split_first()?;
        list = branches(effect).get(branch).copied()?;
        path = deeper;
    }
}

/// Where the effect after the one at `position` runs: inside the branch it enters, else the next sibling, else
/// the effect after the one that holds the list. Past the last effect of the root it is the end, one beyond.
pub(crate) fn after(root: &[Effect], position: &[usize], next: Next) -> Vec<usize> {
    if let Next::Enter(branch) = next {
        let mut inside = position.to_vec();
        inside.extend([branch, 0]);
        if effect_at(root, &inside).is_some() {
            return inside;
        }
    }
    sibling(root, position)
}

fn sibling(root: &[Effect], position: &[usize]) -> Vec<usize> {
    let mut next = position.to_vec();
    if let Some(last) = next.last_mut() {
        *last += 1;
    }
    if next.len() == 1 || effect_at(root, &next).is_some() {
        return next;
    }
    next.truncate(position.len().saturating_sub(2));
    sibling(root, &next)
}

#[cfg(test)]
mod tests {
    //! Goal: positions walk the effect tree in the order the effects happen, entering the branch an effect chooses and leaving a list after its last effect, for any nesting.
    use super::*;
    use rstest::rstest;

    fn effects(json: &str) -> Vec<Effect> {
        serde_json::from_str(json).unwrap()
    }

    fn tree() -> Vec<Effect> {
        effects(
            r#"[
                {"Draw":{}},
                {"If":{"condition":"YourTurn","then":[{"GainLife":{"amount":1}},{"LoseLife":{"amount":1}}],"otherwise":[{"Draw":{}}]}},
                {"Pay":{"cost":"{1}","paid":[],"unpaid":[{"May":{"effects":[{"Draw":{}}]}}]}},
                {"GainLife":{"amount":2}}
            ]"#,
        )
    }

    fn name(effect: Option<&Effect>) -> String {
        match effect {
            None => "end".to_owned(),
            Some(effect) => serde_json::to_value(effect)
                .unwrap()
                .as_object()
                .and_then(|map| map.keys().next().cloned())
                .unwrap_or_default(),
        }
    }

    #[rstest]
    #[case::the_first_effect(&[0], "Draw")]
    #[case::a_branch_effect(&[1, 0, 1], "LoseLife")]
    #[case::the_other_branch(&[1, 1, 0], "Draw")]
    #[case::nested_two_deep(&[2, 1, 0, 0, 0], "Draw")]
    #[case::the_last_effect(&[3], "GainLife")]
    #[case::past_the_end(&[4], "end")]
    #[case::past_the_end_of_a_branch(&[1, 0, 2], "end")]
    #[case::a_branch_that_does_not_exist(&[0, 0, 0], "end")]
    #[case::a_branch_with_no_effect_index(&[1, 0], "end")]
    #[case::nothing(&[], "end")]
    fn a_position_names_an_effect_or_the_end(#[case] position: &[usize], #[case] expected: &str) {
        assert_eq!(name(effect_at(&tree(), position)), expected);
    }

    #[rstest]
    #[case::to_the_next_sibling(&[0], Next::Sibling, &[1])]
    #[case::into_then(&[1], Next::Enter(0), &[1, 0, 0])]
    #[case::into_otherwise(&[1], Next::Enter(1), &[1, 1, 0])]
    #[case::within_a_branch(&[1, 0, 0], Next::Sibling, &[1, 0, 1])]
    #[case::out_of_a_branch(&[1, 0, 1], Next::Sibling, &[2])]
    #[case::out_of_the_short_branch(&[1, 1, 0], Next::Sibling, &[2])]
    #[case::into_an_empty_branch_is_past_it(&[2], Next::Enter(0), &[3])]
    #[case::into_a_branch_that_is_not_there(&[2], Next::Enter(5), &[3])]
    #[case::out_of_two_levels(&[2, 1, 0, 0, 0], Next::Sibling, &[3])]
    #[case::into_two_levels(&[2], Next::Enter(1), &[2, 1, 0])]
    #[case::to_the_end(&[3], Next::Sibling, &[4])]
    fn the_next_position_follows_the_branch_or_the_list(
        #[case] position: &[usize],
        #[case] next: Next,
        #[case] expected: &[usize],
    ) {
        assert_eq!(after(&tree(), position, next), expected);
    }

    #[test]
    fn a_walk_that_always_goes_on_visits_every_effect_once_in_order() {
        let tree = tree();
        let mut position = vec![0];
        let mut seen = Vec::new();
        while let Some(effect) = effect_at(&tree, &position) {
            seen.push(name(Some(effect)));
            position = after(&tree, &position, Next::Sibling);
        }
        assert_eq!(seen, ["Draw", "If", "Pay", "GainLife"]);
        assert_eq!(position, [4]);
    }

    #[test]
    fn a_walk_that_enters_every_branch_visits_each_nested_effect_before_the_next_sibling() {
        let tree = tree();
        let mut position = vec![0];
        let mut seen = Vec::new();
        while let Some(effect) = effect_at(&tree, &position) {
            seen.push(name(Some(effect)));
            position = after(&tree, &position, Next::Enter(0));
        }
        // `then` of the If, then the Pay (its `paid` list is empty), then the last effect.
        assert_eq!(
            seen,
            ["Draw", "If", "GainLife", "LoseLife", "Pay", "GainLife"]
        );
    }
}
