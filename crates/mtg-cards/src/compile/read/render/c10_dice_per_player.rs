//! A target for each opponent: the ability is written with the target of one opponent ("up to one target artifact an
//! opponent controls"), then that player is named in the phrase and "for each opponent," opens the clause: "For each
//! opponent, destroy up to one target artifact or enchantment that player controls."

use super::variants;
use crate::ability::Ability;
use crate::refs::{Ref, Targetable};

/// The phrases that name the opponent of the stand-in's target, and how the card names the player of the pass.
const C10_DICE_PLAYER: [(&str, &str); 2] = [
    ("an opponent controls", "that player controls"),
    ("an opponent's", "that player's"),
];

/// The wordings of an ability with one target specification for each opponent; `None` for any other ability.
pub(super) fn c10_dice_variants(ability: &Ability) -> Option<Vec<String>> {
    let mut stand = ability.clone();
    let (Ability::Spell { targets, .. }
    | Ability::Activated { targets, .. }
    | Ability::Triggered { targets, .. }
    | Ability::Loyalty { targets, .. }) = &mut stand
    else {
        return None;
    };
    let mut found = 0;
    for spec in targets.iter_mut() {
        if let Targetable::PerPlayer {
            players: players @ Ref::Opponents,
            what,
        } = &spec.what
        {
            spec.what = Targetable::Object(Targetable::c10_dice_per_player_objects(players, what));
            found += 1;
        }
    }
    (found == 1).then(|| {
        variants(&stand)
            .iter()
            .filter_map(|text| c10_dice_for_each(text))
            .collect()
    })
}

/// The wording with the opponent of the target named as the player of the pass and "for each opponent," before the
/// clause that asks for it.
fn c10_dice_for_each(text: &str) -> Option<String> {
    let target = text.find("up to one target ")?;
    let (head, tail) = text.split_at(target);
    let (from, to) = C10_DICE_PLAYER
        .iter()
        .find(|(from, _)| tail.contains(from))?;
    let tail = tail.replacen(from, to, 1);
    let start = [". ", ": ", ", ", "\u{2014} "]
        .iter()
        .filter_map(|mark| head.rfind(mark).map(|at| at + mark.len()))
        .max()
        .unwrap_or(0);
    let (before, clause) = head.split_at(start);
    let opens = start == 0
        || before.ends_with(". ")
        || before.ends_with(": ")
        || before.ends_with("\u{2014} ");
    let mut chars = clause.chars();
    let clause = match chars.next() {
        Some(first) if opens => format!("{}{}", first.to_lowercase(), chars.as_str()),
        _ => clause.to_owned(),
    };
    let lead = if opens {
        "For each opponent, "
    } else {
        "for each opponent, "
    };
    Some(format!("{before}{lead}{clause}{tail}"))
}

#[cfg(test)]
mod tests {
    //! Goal: the opponent of the stand-in's target becomes the player of the pass and the clause opens with "for each
    //! opponent", in a spell, after a trigger's comma and after an activation's colon.
    use rstest::rstest;

    use super::c10_dice_for_each;

    #[rstest]
    #[case::a_spell(
        "Destroy up to one target artifact or enchantment an opponent controls.",
        "For each opponent, destroy up to one target artifact or enchantment that player controls."
    )]
    #[case::a_trigger(
        "When ~ enters, return up to one target nonland permanent an opponent controls to its owner's hand.",
        "When ~ enters, for each opponent, return up to one target nonland permanent that player controls to its owner's hand."
    )]
    #[case::a_graveyard(
        "{T}: Exile up to one target card from an opponent's graveyard.",
        "{T}: For each opponent, exile up to one target card from that player's graveyard."
    )]
    fn c10_dice_the_pass_is_written(#[case] stand: &str, #[case] card: &str) {
        assert_eq!(c10_dice_for_each(stand).as_deref(), Some(card));
    }

    #[test]
    fn c10_dice_a_text_without_one_opponent_target_is_left_alone() {
        assert_eq!(
            c10_dice_for_each("Destroy target artifact an opponent controls."),
            None
        );
    }
}
