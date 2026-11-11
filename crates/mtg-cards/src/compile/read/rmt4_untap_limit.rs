//! The untap limit (CR 502.3): "Players can't untap more than one land during their untap steps.", "You can't untap
//! more than one land during your untap step." and "... more than two permanents ...".

use super::amounts::{count_word, number_word};
use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::modify::GameRule;
use crate::refs::Ref;

/// The players and their words: the subject and the untap step they name.
const WHO: [(&str, &str, Ref); 2] = [
    ("players", "during their untap steps", Ref::EachPlayer),
    ("you", "during your untap step", Ref::You),
];

/// Reads "<players> can't untap more than <N> <permanents> during <their> untap step(s)".
pub(super) fn read(r: &mut Reader<'_>) -> Option<GameRule> {
    let (step, who) = WHO
        .iter()
        .find_map(|(words, step, who)| r.words(words).then(|| (*step, who.clone())))?;
    r.need("can't untap more than")?;
    let count = number_word(r.peek_word()?).filter(|count| *count > 0)?;
    r.skip();
    let what = phrase(r, if count == 1 { Form::Subject } else { Form::All })?;
    r.need(step)?;
    Some(GameRule::UntapLimit { who, what, count })
}

/// The wordings of an untap limit, without the final period.
pub(super) fn texts(rule: &GameRule) -> Vec<String> {
    let GameRule::UntapLimit { who, what, count } = rule else {
        return Vec::new();
    };
    let Some((words, step, _)) = WHO.iter().find(|(_, _, named)| named == who) else {
        return Vec::new();
    };
    let form = if *count == 1 {
        Form::Subject
    } else {
        Form::All
    };
    let (Some(number), Some(kind)) = (count_word(*count), phrase_text(what, form)) else {
        return Vec::new();
    };
    let subject = super::render::capitalize(words);
    vec![format!(
        "{subject} can't untap more than {number} {kind} {step}"
    )]
}

/// The wordings of a rule about the untap step: untapping in other steps, or an untap limit.
pub(super) fn untap_rule_texts(rule: &GameRule) -> Vec<String> {
    let mut found = super::e10c_granted::e10c_untap_during_texts(rule);
    found.extend(texts(rule));
    found
}
