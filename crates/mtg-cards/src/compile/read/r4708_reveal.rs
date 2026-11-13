//! r47-08 reveals of the first card drawn each turn: "Reveal the first card you draw each turn.", "Reveal the first card
//! you draw on each of your turns." and "You may reveal the first card you draw each turn as you draw it."
//! (`GameRule::RevealFirstDraw`), and the head "Whenever you reveal a creature card this way"
//! (`TriggerPattern::FirstDrawRevealed`), read and written back.

use mtg_core::card::{CardType, Supertype};

use super::tokens::{Reader, same_text};
use crate::ability::Ability;
use crate::modify::GameRule;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// The sentences of the reveal, with whether it is optional and whether it works only on your turns.
const SENTENCES: [(&str, bool, bool); 3] = [
    ("Reveal the first card you draw each turn.", false, false),
    (
        "Reveal the first card you draw on each of your turns.",
        false,
        true,
    ),
    (
        "You may reveal the first card you draw each turn as you draw it.",
        true,
        false,
    ),
];

/// The reveal rule of the permanent.
const fn reveal(may: bool, your_turns: bool) -> Ability {
    Ability::Rule {
        rule: GameRule::RevealFirstDraw { may, your_turns },
        condition: None,
        from: ZoneKind::Battlefield,
    }
}

/// The reveal rule the sentence words.
pub(super) fn r4708_reveal_read(text: &str) -> Option<Ability> {
    SENTENCES
        .iter()
        .find(|(sentence, ..)| same_text(text, sentence))
        .map(|(_, may, your_turns)| reveal(*may, *your_turns))
}

/// The sentence of a reveal rule.
pub(super) fn r4708_reveal_wordings(ability: &Ability, texts: &mut Vec<String>) {
    for (sentence, may, your_turns) in SENTENCES {
        if *ability == reveal(may, your_turns) && !texts.iter().any(|text| text == sentence) {
            texts.push(sentence.to_owned());
        }
    }
}

/// The kinds of card a reveal trigger names, each with its words before "card".
fn kinds() -> [(&'static str, Selector); 5] {
    let of = |kind| Pred::Type(kind);
    [
        ("a creature", Selector(vec![of(CardType::Creature)])),
        (
            "a basic land",
            Selector(vec![Pred::Supertype(Supertype::Basic), of(CardType::Land)]),
        ),
        ("a land", Selector(vec![of(CardType::Land)])),
        (
            "a nonland",
            Selector(vec![Pred::Not(Box::new(of(CardType::Land)))]),
        ),
        (
            "an instant or sorcery",
            Selector(vec![Pred::AnyOf(vec![
                of(CardType::Instant),
                of(CardType::Sorcery),
            ])]),
        ),
    ]
}

/// "reveal a creature card this way", after the player who reveals (only you).
pub(super) fn r4708_revealed(r: &mut Reader<'_>, by: &Ref) -> Option<TriggerPattern> {
    if *by != Ref::You {
        return None;
    }
    let (_, what) = kinds().into_iter().find(|(words, _)| r.words(words))?;
    r.need("card this way")?;
    Some(TriggerPattern::FirstDrawRevealed { what })
}

/// "Whenever you reveal a creature card this way".
pub(super) fn r4708_revealed_text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::FirstDrawRevealed { what } = pattern else {
        return None;
    };
    let (words, _) = kinds().into_iter().find(|(_, kind)| kind == what)?;
    Some(format!("Whenever you reveal {words} card this way"))
}

/// The sentences of a line that begins with the reveal and goes on with its triggers ("Whenever you reveal ..."): the
/// reveal first, then each trigger with its period.
fn sentences(text: &str) -> Option<(&str, Vec<&str>)> {
    let cut = text.find(". Whenever ")?;
    let (head, rest) = text.split_at(cut + 1);
    let rest = rest.trim_start();
    let starts: Vec<usize> = rest.match_indices("Whenever ").map(|(at, _)| at).collect();
    let mut triggers = Vec::new();
    for (index, start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(rest.len());
        triggers.push(rest.get(*start..end)?.trim_end());
    }
    Some((head, triggers))
}

/// The reveal and its triggers, read from one line: the reveal sentence, then each "Whenever you reveal ... this way"
/// sentence read as a triggered clause by `trigger`.
pub(super) fn r4708_reveal_group_read(
    text: &str,
    trigger: impl Fn(&str) -> Option<Ability>,
) -> Option<Vec<Ability>> {
    let (head, triggers) = sentences(text)?;
    let mut found = vec![r4708_reveal_read(head)?];
    for sentence in triggers {
        let ability = trigger(sentence)?;
        matches!(
            &ability,
            Ability::Triggered {
                when: TriggerPattern::FirstDrawRevealed { .. },
                ..
            }
        )
        .then_some(())?;
        found.push(ability);
    }
    (found.len() > 1).then_some(found)
}

/// Every wording of a reveal and the triggers on it as one line, each sentence as its writer words it; empty unless
/// `group` is a reveal rule followed only by triggers on its reveal.
pub fn r4708_reveal_group_texts(group: &[Ability]) -> Vec<String> {
    let Some((rule, triggers)) = group.split_first() else {
        return Vec::new();
    };
    let mut texts = Vec::new();
    r4708_reveal_wordings(rule, &mut texts);
    if texts.is_empty() || triggers.is_empty() {
        return Vec::new();
    }
    for trigger in triggers {
        if !matches!(
            trigger,
            Ability::Triggered {
                when: TriggerPattern::FirstDrawRevealed { .. },
                ..
            }
        ) {
            return Vec::new();
        }
        let written = super::written_variants(trigger);
        texts = texts
            .iter()
            .flat_map(|before| written.iter().map(move |after| format!("{before} {after}")))
            .collect();
        if texts.is_empty() {
            return Vec::new();
        }
    }
    texts
}
