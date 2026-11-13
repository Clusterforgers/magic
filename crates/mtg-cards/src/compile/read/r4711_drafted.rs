//! r47-11 wordings of what a booster draft notes for cards of a name (`DraftNote`): "the highest number you noted for
//! cards named ~" as an amount, "a card with a name you noted for cards named ~" and "other creatures you control of a
//! type you noted ..." as an object phrase, "if you guessed correctly for a card named ~" as a condition. Each is
//! written and read through a twin with a sentinel in its place: a count of a made-up subtype, the subtype itself and a
//! life total, which the other writers and readers word.

use crate::ability::Ability;
use crate::r4711_noted::DraftNote;

/// The made-up subtype the twins use.
const R4711_SUBTYPE: &str = "Brushwagg";
/// The amount a noted number is twinned with, as JSON.
const R4711_AMOUNT: &str = r#"{"Count":[{"Subtype":"Brushwagg"},{"ControlledBy":"You"}]}"#;
/// The twin amount's words.
const R4711_AMOUNT_WORDS: &str = "the number of Brushwaggs you control";
/// The condition a guess is twinned with, as JSON.
const R4711_CONDITION: &str = r#"{"Compare":{"left":{"Life":"You"},"op":"Ge","right":77}}"#;
/// The twin condition's words.
const R4711_CONDITION_WORDS: &str = "you have 77 or more life";

/// How a note stands in a sentence.
#[derive(Clone, Copy, PartialEq, Eq)]
enum R4711Role {
    /// A number.
    Amount,
    /// A phrase after the noun of an object phrase.
    Object,
    /// A condition.
    Condition,
}

/// The role of a note and, for an object phrase, the words after its noun.
fn r4711_role(note: DraftNote) -> (R4711Role, String) {
    match note {
        DraftNote::HighestNumber | DraftNote::CardsRemoved => {
            (R4711Role::Amount, note.words().to_owned())
        }
        DraftNote::GuessedRight | DraftNote::NotedPlayer => {
            (R4711Role::Condition, note.words().to_owned())
        }
        DraftNote::NotedName | DraftNote::NameNotedAsDrafted => {
            (R4711Role::Object, format!("with {}", note.words()))
        }
        DraftNote::NotedType => (R4711Role::Object, format!("of {}", note.words())),
        DraftNote::ExiledWith => (R4711Role::Object, note.words().to_owned()),
        DraftNote::ChosenColors => (R4711Role::Object, format!("that's {}", note.words())),
    }
}

/// The JSON of a note as the rules file holds it.
fn r4711_json(note: DraftNote) -> Option<String> {
    let value = serde_json::to_value(note).ok()?;
    Some(format!(r#"{{"Drafted":{value}}}"#))
}

/// The JSON of the twin's sentinel for a note.
fn r4711_sentinel(note: DraftNote) -> String {
    match r4711_role(note).0 {
        R4711Role::Amount => R4711_AMOUNT.to_owned(),
        R4711Role::Condition => R4711_CONDITION.to_owned(),
        R4711Role::Object => format!(r#"{{"Subtype":"{R4711_SUBTYPE}"}}"#),
    }
}

/// The ability with its one note put in place of its sentinel, and the note; `None` without exactly one note.
fn r4711_twin(ability: &Ability) -> Option<(Ability, DraftNote)> {
    let text = serde_json::to_string(ability).ok()?;
    let mut found = None;
    for note in DraftNote::ALL {
        let json = r4711_json(note)?;
        if text.contains(&json) {
            if found.is_some() || text.matches(&json).count() != 1 {
                return None;
            }
            found = Some((json, note));
        }
    }
    let (json, note) = found?;
    // A noted player is worded only whole (`r4711_wholes`).
    if note == DraftNote::NotedPlayer {
        return None;
    }
    let twin = serde_json::from_str(&text.replacen(&json, &r4711_sentinel(note), 1)).ok()?;
    Some((twin, note))
}

/// The twin's text with the note's words in place of its sentinel.
fn r4711_noted_text(text: &str, note: DraftNote) -> Option<String> {
    let (role, words) = r4711_role(note);
    match role {
        R4711Role::Amount => (text.matches(R4711_AMOUNT_WORDS).count() == 1)
            .then(|| text.replacen(R4711_AMOUNT_WORDS, &words, 1)),
        R4711Role::Condition => (text.matches(R4711_CONDITION_WORDS).count() == 1)
            .then(|| text.replacen(R4711_CONDITION_WORDS, &words, 1)),
        R4711Role::Object => {
            let marker = format!("{R4711_SUBTYPE} ");
            let (head, rest) = text.split_once(&marker)?;
            if rest.contains(R4711_SUBTYPE) {
                return None;
            }
            let noun_end = rest.find([' ', '.', ',']).unwrap_or(rest.len());
            let (noun, tail) = rest.split_at(noun_end);
            let (control, tail) = match tail.strip_prefix(" you control") {
                Some(after) => (" you control", after),
                None => ("", tail),
            };
            Some(format!("{head}{noun}{control} {words}{tail}"))
        }
    }
}

/// The twin text of a text with a note's words, with the sentinel's words in their place.
fn r4711_twin_text(text: &str, note: DraftNote) -> Option<String> {
    let (role, words) = r4711_role(note);
    if text.matches(words.as_str()).count() != 1 {
        return None;
    }
    match role {
        R4711Role::Amount => Some(text.replacen(&words, R4711_AMOUNT_WORDS, 1)),
        R4711Role::Condition => Some(text.replacen(&words, R4711_CONDITION_WORDS, 1)),
        R4711Role::Object => {
            let (head, tail) = text.split_once(&format!(" {words}"))?;
            let head_noun = head.strip_suffix(" you control").unwrap_or(head);
            let control = &head[head_noun.len()..];
            let start = head_noun.rfind(' ').map_or(0, |space| space + 1);
            let (before, noun) = head_noun.split_at(start);
            Some(format!("{before}{R4711_SUBTYPE} {noun}{control}{tail}"))
        }
    }
}

/// The wordings of an ability with a note: the twin's wordings with the note's words in place of its sentinel.
pub(super) fn r4711_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some((twin, note)) = r4711_twin(ability) else {
        return;
    };
    let found: Vec<String> = written(&twin)
        .iter()
        .filter_map(|text| r4711_noted_text(text, note))
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// Reads a clause with a note's words: the twin with the sentinel's words read by `read`, the note put back in place
/// of the sentinel.
pub(super) fn r4711_read(text: &str, read: impl Fn(&str) -> Option<Ability>) -> Option<Ability> {
    DraftNote::ALL
        .into_iter()
        .filter(|note| *note != DraftNote::NotedPlayer)
        .find_map(|note| {
            let twin_text = r4711_twin_text(text, note)?;
            let twin = read(&twin_text)?;
            let json = serde_json::to_string(&twin).ok()?;
            let sentinel = serde_json::to_string(
                &serde_json::from_str::<serde_json::Value>(&r4711_sentinel(note)).ok()?,
            )
            .ok()?;
            if json.matches(&sentinel).count() != 1 {
                return None;
            }
            serde_json::from_str(&json.replacen(&sentinel, &r4711_json(note)?, 1)).ok()
        })
}

#[cfg(test)]
mod tests {
    //! Goal: a note's words and the sentinel's words turn into each other in every role, and only one note is read.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::amount(
        DraftNote::HighestNumber,
        "~ deals damage to target creature equal to the highest number you noted for cards named ~.",
        "~ deals damage to target creature equal to the number of Brushwaggs you control."
    )]
    #[case::name(
        DraftNote::NotedName,
        "search your library for a card with a name you noted for cards named ~, reveal it",
        "search your library for a Brushwagg card, reveal it"
    )]
    #[case::type_you_control(
        DraftNote::NotedType,
        "Other creatures you control of a type you noted for cards named ~ get +1/+1.",
        "Other Brushwagg creatures you control get +1/+1."
    )]
    #[case::colors(
        DraftNote::ChosenColors,
        "Destroy target creature that's one or more of the colors chosen as you drafted cards named ~.",
        "Destroy target Brushwagg creature."
    )]
    #[case::guess(
        DraftNote::GuessedRight,
        "When ~ enters, if you guessed correctly for a card named ~, draw a card.",
        "When ~ enters, if you have 77 or more life, draw a card."
    )]
    fn r4711_note_words_and_twin_words_turn_into_each_other(
        #[case] note: DraftNote,
        #[case] noted: &str,
        #[case] twin: &str,
    ) {
        assert_eq!(r4711_twin_text(noted, note).as_deref(), Some(twin));
        assert_eq!(r4711_noted_text(twin, note).as_deref(), Some(noted));
    }

    #[rstest]
    #[case::other_note(
        DraftNote::CardsRemoved,
        "the highest number you noted for cards named ~"
    )]
    #[case::twice(
        DraftNote::HighestNumber,
        "the highest number you noted for cards named ~ and the highest number you noted for cards named ~"
    )]
    fn r4711_a_text_without_one_note_has_no_twin(#[case] note: DraftNote, #[case] text: &str) {
        assert_eq!(r4711_twin_text(text, note), None);
    }
}
