//! Positions in the effect tree.
//!
//! A position is a path of indices. `[2]` is the third effect of the list. An effect that holds lists of
//! effects (`If`, `May`, `Pay`, `Modal`) is entered by a branch: `[2, 1, 0]` is the first effect of the second
//! list of the third effect (the `otherwise` of an `If`, the `unpaid` of a `Pay`). The branches of a `Modal` are the
//! modes chosen, in the order they happen: `[0, 1, 0]` is the first effect of the second mode chosen, which is the
//! same mode as the first one when it was chosen twice.

use mtg_cards::effect::Effect;

use super::Next;

/// The lists of effects inside an effect, in branch order; `chosen` are the modes chosen for a `Modal`.
fn branches<'a>(effect: &'a Effect, chosen: &[usize]) -> Vec<&'a [Effect]> {
    match effect {
        Effect::If {
            then, otherwise, ..
        } => vec![then, otherwise],
        Effect::May { effects, .. } => vec![effects],
        Effect::Pay { paid, unpaid, .. } => vec![paid, unpaid],
        Effect::CastFromZone { otherwise, .. } => vec![otherwise],
        Effect::Modal { modes, .. } => chosen
            .iter()
            .map(|index| modes.get(*index).map_or(&[][..], |m| m.effects.as_slice()))
            .collect(),
        _ => Vec::new(),
    }
}

/// The effect at `position`, if there is one.
pub(crate) fn effect_at<'a>(
    root: &'a [Effect],
    chosen: &[usize],
    position: &[usize],
) -> Option<&'a Effect> {
    let mut list = root;
    let mut path = position;
    loop {
        let (&index, rest) = path.split_first()?;
        let effect = list.get(index)?;
        if rest.is_empty() {
            return Some(effect);
        }
        let (&branch, deeper) = rest.split_first()?;
        list = branches(effect, chosen).get(branch).copied()?;
        path = deeper;
    }
}

/// The place among the modes chosen of the mode the position is inside, if it is inside a `Modal`.
pub(crate) fn mode_at(root: &[Effect], chosen: &[usize], position: &[usize]) -> Option<usize> {
    let mut list = root;
    let mut path = position;
    loop {
        let (&index, rest) = path.split_first()?;
        let effect = list.get(index)?;
        let (&branch, deeper) = rest.split_first()?;
        if matches!(effect, Effect::Modal { .. }) {
            return Some(branch);
        }
        list = branches(effect, chosen).get(branch).copied()?;
        path = deeper;
    }
}

/// Where the effect after the one at `position` runs: inside the branch it enters, else the next sibling, else
/// the effect after the one that holds the list. Past the last effect of the root it is the end, one beyond.
pub(crate) fn after(
    root: &[Effect],
    chosen: &[usize],
    position: &[usize],
    next: Next,
) -> Vec<usize> {
    if let Next::Enter(branch) = next
        && let Some(inside) = enter(root, chosen, position, branch)
    {
        return inside;
    }
    sibling(root, chosen, position)
}

/// The first effect inside the effect at `position`, from `branch` on. The branch of an `If`, `May` or `Pay` is the
/// one it chose and no other; a `Modal` goes on to the next mode chosen that has an effect to run.
fn enter(
    root: &[Effect],
    chosen: &[usize],
    position: &[usize],
    branch: usize,
) -> Option<Vec<usize>> {
    let last = match effect_at(root, chosen, position) {
        Some(Effect::Modal { .. }) => chosen.len(),
        _ => branch + 1,
    };
    (branch..last)
        .map(|at| [position, &[at, 0]].concat())
        .find(|inside| effect_at(root, chosen, inside).is_some())
}

fn sibling(root: &[Effect], chosen: &[usize], position: &[usize]) -> Vec<usize> {
    let mut next = position.to_vec();
    if let Some(last) = next.last_mut() {
        *last += 1;
    }
    if next.len() == 1 || effect_at(root, chosen, &next).is_some() {
        return next;
    }
    // The list is done: on to the next mode chosen if the effect that holds it is a `Modal`, else past that effect.
    let holder = &position[..position.len().saturating_sub(2)];
    if let Some(&branch) = position.get(holder.len())
        && matches!(effect_at(root, chosen, holder), Some(Effect::Modal { .. }))
        && let Some(inside) = enter(root, chosen, holder, branch + 1)
    {
        return inside;
    }
    sibling(root, chosen, holder)
}

#[cfg(test)]
mod tests {
    //! Goal: positions walk the effect tree in the order the effects happen, entering the branch an effect chooses and leaving a list after its last effect, for any nesting, and a modal effect walks the modes chosen in the order chosen, a mode chosen twice twice and a mode with nothing to do not at all.
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

    /// A modal ability: mode 0 gains life then loses it, mode 1 draws, mode 2 has nothing to do, mode 3 shuffles.
    fn modal() -> Vec<Effect> {
        effects(
            r#"[{"Modal":{"modes":[
                {"effects":[{"GainLife":{"amount":1}},{"LoseLife":{"amount":1}}]},
                {"effects":[{"Draw":{}}]},
                {"effects":[]},
                {"effects":[{"Shuffle":{}}]}
            ]}}]"#,
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
        assert_eq!(name(effect_at(&tree(), &[], position)), expected);
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
        assert_eq!(after(&tree(), &[], position, next), expected);
    }

    #[test]
    fn a_walk_that_always_goes_on_visits_every_effect_once_in_order() {
        let tree = tree();
        let mut position = vec![0];
        let mut seen = Vec::new();
        while let Some(effect) = effect_at(&tree, &[], &position) {
            seen.push(name(Some(effect)));
            position = after(&tree, &[], &position, Next::Sibling);
        }
        assert_eq!(seen, ["Draw", "If", "Pay", "GainLife"]);
        assert_eq!(position, [4]);
    }

    #[test]
    fn a_walk_that_enters_every_branch_visits_each_nested_effect_before_the_next_sibling() {
        let tree = tree();
        let mut position = vec![0];
        let mut seen = Vec::new();
        while let Some(effect) = effect_at(&tree, &[], &position) {
            seen.push(name(Some(effect)));
            position = after(&tree, &[], &position, Next::Enter(0));
        }
        // `then` of the If, then the Pay (its `paid` list is empty), then the last effect.
        assert_eq!(
            seen,
            ["Draw", "If", "GainLife", "LoseLife", "Pay", "GainLife"]
        );
    }

    /// The effects a walk of the modal ability visits when `chosen` modes were chosen, entering the first at once.
    fn walk(chosen: &[usize]) -> Vec<String> {
        let tree = modal();
        let mut position = vec![0];
        let mut seen = Vec::new();
        while let Some(effect) = effect_at(&tree, chosen, &position) {
            seen.push(name(Some(effect)));
            position = after(&tree, chosen, &position, Next::Enter(0));
        }
        assert_eq!(position, [1], "the walk ends past the modal effect");
        seen
    }

    #[rstest]
    #[case::no_mode(&[], &["Modal"])]
    #[case::one_mode(&[1], &["Modal", "Draw"])]
    #[case::a_mode_of_two_effects(&[0], &["Modal", "GainLife", "LoseLife"])]
    #[case::two_modes_in_order(&[0, 3], &["Modal", "GainLife", "LoseLife", "Shuffle"])]
    #[case::a_mode_twice(&[1, 1], &["Modal", "Draw", "Draw"])]
    #[case::a_repeat_between_others(&[0, 1, 1, 3], &["Modal", "GainLife", "LoseLife", "Draw", "Draw", "Shuffle"])]
    #[case::a_mode_with_nothing_to_do_is_skipped(&[2, 1], &["Modal", "Draw"])]
    #[case::only_empty_modes(&[2, 2], &["Modal"])]
    #[case::an_empty_mode_in_the_middle(&[0, 2, 3], &["Modal", "GainLife", "LoseLife", "Shuffle"])]
    fn a_modal_effect_walks_the_modes_chosen_in_order(
        #[case] chosen: &[usize],
        #[case] expected: &[&str],
    ) {
        assert_eq!(walk(chosen), expected);
    }

    #[rstest]
    #[case::the_modal_effect_itself(true, &[0], &[1, 0], None)]
    #[case::the_first_mode(true, &[0, 0, 1], &[1, 0], Some(0))]
    #[case::the_second_mode_chosen(true, &[0, 1, 0], &[1, 0], Some(1))]
    #[case::the_third_mode_chosen(true, &[0, 2, 0], &[0, 0, 3], Some(2))]
    #[case::no_modal_effect(false, &[1, 0, 0], &[], None)]
    fn the_mode_a_position_is_inside_is_its_place_among_those_chosen(
        #[case] modal_tree: bool,
        #[case] position: &[usize],
        #[case] chosen: &[usize],
        #[case] expected: Option<usize>,
    ) {
        let tree = if modal_tree { modal() } else { tree() };
        assert_eq!(mode_at(&tree, chosen, position), expected);
    }
}
