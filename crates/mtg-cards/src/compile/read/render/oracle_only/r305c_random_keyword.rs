//! Wordings of a triggered ability that gives its source one of two keywords chosen at random: "At the beginning of
//! combat on your turn, choose flying or indestructible at random. ~ gains that ability until end of turn.", written from
//! the two twins that give one keyword each.

use crate::ability::Ability;
use crate::effect::{Effect, Mode};

/// Adds the wording of a trigger whose only effect is a random choice between two modes that each give one keyword.
pub(super) fn r305c_add_random_keyword(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    let [
        Effect::Modal {
            at_random: true,
            modes,
            ..
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let [first, second] = modes.as_slice() else {
        return;
    };
    if !r305c_plain(first) || !r305c_plain(second) {
        return;
    }
    let twin = |mode: &Mode| {
        let mut twin = ability.clone();
        if let Ability::Triggered { effects, .. } = &mut twin {
            effects.clone_from(&mode.effects);
        }
        super::super::variants(&twin)
    };
    for one in twin(first) {
        for other in twin(second) {
            texts.extend(r305c_joined(&one, &other));
        }
    }
}

/// A mode with no label or target and one effect that gives a keyword until end of turn.
fn r305c_plain(mode: &Mode) -> bool {
    mode.label.is_none()
        && mode.targets.is_empty()
        && matches!(mode.effects.as_slice(), [Effect::Continuous { mods, .. }] if mods.len() == 1)
}

/// "<event>, choose <one> or <other> at random. ~ gains that ability until end of turn." from "<event>, ~ gains <one>
/// until end of turn." and the same with `other`.
fn r305c_joined(one: &str, other: &str) -> Option<String> {
    let prefix = one
        .char_indices()
        .zip(other.chars())
        .take_while(|((_, a), b)| a == b)
        .last()
        .map_or(0, |((at, a), _)| at + a.len_utf8());
    let head = one.get(..prefix)?;
    let (event, subject) = head.strip_suffix("gains ")?.rsplit_once(", ")?;
    let tail = " until end of turn.";
    let first = one.get(prefix..)?.strip_suffix(tail)?;
    let second = other.get(prefix..)?.strip_suffix(tail)?;
    if first.is_empty() || second.is_empty() || first == second {
        return None;
    }
    Some(format!(
        "{event}, choose {first} or {second} at random. {subject}gains that ability until end of turn."
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: a random choice of one of two keywords reads as printed; a chosen (not random) mode or three modes do not.
    use super::*;
    use rstest::rstest;

    fn mode(keyword: &str) -> String {
        format!(
            r#"{{"effects":[{{"Continuous":{{"affected":"This","mods":[{{"AddKeyword":"{keyword}"}}],"duration":"UntilEndOfTurn"}}}}]}}"#
        )
    }

    fn written(at_random: bool, keywords: &[&str]) -> Vec<String> {
        let modes: Vec<String> = keywords.iter().map(|k| mode(k)).collect();
        let json = format!(
            r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"BeginCombat","whose":"You"}}}},"effects":[{{"Modal":{{"at_random":{at_random},"modes":[{}]}}}}]}}}}"#,
            modes.join(",")
        );
        let ability: Ability = serde_json::from_str(&json).unwrap();
        let mut texts = Vec::new();
        r305c_add_random_keyword(&ability, &mut texts);
        texts
    }

    const BLITZWING: &str = "At the beginning of combat on your turn, choose flying or indestructible at random. ~ gains that ability until end of turn.";

    #[rstest]
    #[case::random_pair(true, &["Flying", "Indestructible"], true)]
    #[case::chosen_not_random(false, &["Flying", "Indestructible"], false)]
    #[case::three_keywords(true, &["Flying", "Indestructible", "Haste"], false)]
    #[case::other_order(true, &["Indestructible", "Flying"], false)]
    fn r305c_random_keyword_reads_back(
        #[case] at_random: bool,
        #[case] keywords: &[&str],
        #[case] expected: bool,
    ) {
        let texts = written(at_random, keywords);
        assert_eq!(texts.iter().any(|t| t == BLITZWING), expected, "{texts:#?}");
    }
}
