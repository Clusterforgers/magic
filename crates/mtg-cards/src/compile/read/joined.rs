//! Two sentences the card joins with "and": "Tap enchanted creature and you become the monarch." for the writer's "Tap
//! enchanted creature. You become the monarch.", and "you gain 2 life and draw a card" after a "then".

use super::effects::templates;
use super::tokens::Reader;
use crate::effect::Effect;

/// The wordings of `texts` in which a sentence of the controller follows the one before with "and".
pub(super) fn wordings(texts: &[String]) -> Vec<String> {
    let mut more = Vec::new();
    for text in texts.iter().filter(|text| !text.contains(". You may ")) {
        let parts: Vec<&str> = text.split(". You ").collect();
        let Some((first, rest)) = parts.split_first() else {
            continue;
        };
        if rest.is_empty() || rest.len() > 3 {
            continue;
        }
        let joined = |join: &str, later: &str| {
            let mut written = format!("{first}{join}{}", rest[0]);
            for part in &rest[1..] {
                written.push_str(later);
                written.push_str(part);
            }
            written
        };
        // The second sentence follows with "and you" or opens with "Then"; after "Then" a third continues the verb
        // phrase ("..., then you gain 2 life and draw a card") or is a sentence of its own.
        more.push(joined(" and you ", ". You "));
        more.push(joined(". Then you ", ". You "));
        if rest.len() > 1 {
            more.push(joined(". Then you ", " and "));
        }
    }
    more
}

/// "Put two +1/+1 counters on it, then you gain 2 life and draw a card.": a sentence, ", then you" and the sentence of
/// the controller, read as the two sentences in order.
pub(super) fn then_you(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let first = templates().find_map(|template| {
        r.attempt(|r| {
            let found = template(r)?;
            r.then_you_follows().then_some(found)
        })
    })?;
    let mut rest = Vec::new();
    if !super::effects::sentence(r, &mut rest) {
        return None;
    }
    effects.extend(first);
    effects.extend(rest);
    Some(())
}

/// "Tap enchanted creature and you become the monarch.": a sentence, "and" and a sentence about the controller, read as
/// the two sentences in order. The first sentence is the first shape that is followed by "and you".
pub(super) fn and_you(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let found = templates().find_map(|template| {
        r.attempt(|r| {
            let mut first = template(r)?;
            r.need("and")?;
            (r.peek_word() == Some("you")).then_some(())?;
            let second = templates().find_map(|template| {
                r.attempt(|r| {
                    let found = template(r)?;
                    r.sentence_end().then_some(found)
                })
            })?;
            first.extend(second);
            Some(first)
        })
    })?;
    effects.extend(found);
    Some(())
}
