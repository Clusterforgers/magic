//! The reflexive "When you do," (CR 603.12) read into the steps that exist: after "sacrifice it" the follow-up is an `If`
//! of `Performed`, after "surveil N" and "scry N" (done at once) it is the next steps, and after "you may pay {C}" or
//! "you may sacrifice a creature" it is the `paid` steps when they choose a target. "Put a +1/+1 counter on ~, then it
//! fights up to one target creature" is the two steps in one sentence. The writer adds the wordings to `variants`.

use super::effects::sentence;
use super::render::variants;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Condition;

/// What the step before "When you do" is: a sacrifice, which the follow-up depends on, or a keyword action that is always done.
enum W20bLink {
    /// The follow-up runs when the sacrifice took something away.
    Performed,
    /// The follow-up runs after the step.
    Sequence,
}

/// The kind of link the step before offers.
fn w20b_link_of(step: &Effect) -> Option<W20bLink> {
    match step {
        Effect::Sacrifice { .. } => Some(W20bLink::Performed),
        Effect::Surveil { .. } | Effect::Scry { .. } => Some(W20bLink::Sequence),
        _ => None,
    }
}

/// "When you do, S." after "sacrifice it", "surveil 4" or "scry 2": every sentence that follows is the reflexive trigger.
pub(super) fn w20b_when_you_do(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let link = w20b_link_of(effects.last()?)?;
    // A sacrifice also reads "If you do", which the reflexive wording replaces when the follow-up chooses a target.
    let said =
        r.words("when you do") || (matches!(link, W20bLink::Performed) && r.words("if you do"));
    said.then_some(())?;
    r.punct(',').then_some(())?;
    let mut then = Vec::new();
    loop {
        sentence(r, &mut then).then_some(())?;
        if r.end() {
            break;
        }
    }
    match link {
        W20bLink::Performed => effects.push(Effect::If {
            condition: Condition::Performed,
            then,
            otherwise: Vec::new(),
        }),
        W20bLink::Sequence => effects.extend(then),
    }
    Some(())
}

/// "If you do" or, for a follow-up that chooses a target, "When you do" after a payment: whether it was the reflexive one.
pub(super) fn w20b_payment_link(r: &mut Reader<'_>) -> Option<bool> {
    let reflexive = if r.words("when you do") {
        true
    } else {
        r.need("if you do")?;
        false
    };
    r.punct(',').then_some(())?;
    Some(reflexive)
}

/// Whether some step names a target of the ability.
pub(super) fn w20b_uses_target(effects: &[Effect]) -> bool {
    serde_json::to_string(effects).is_ok_and(|json| json.contains("{\"Target\":"))
}

/// "Put a +1/+1 counter on ~, then it fights up to one target creature defending player controls.": the counter and the
/// fight of the card that got it.
pub(super) fn w20b_counter_then_fight(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let mut effects = super::counters::put(r)?;
    matches!(
        effects.as_slice(),
        [Effect::PutCounters { on: Ref::This, .. }]
    )
    .then_some(())?;
    r.punct(',').then_some(())?;
    r.need("then")?;
    let before = r.it.replace(Ref::This);
    let fight = super::actions::fight(r);
    r.it = before;
    let fight = fight?;
    matches!(
        fight.as_slice(),
        [Effect::Fight {
            what: Ref::This,
            ..
        }]
    )
    .then_some(())?;
    effects.extend(fight);
    Some(effects)
}

/// The wordings the data of an ability also has when the reflexive link is spoken.
pub(super) fn w20b_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let effects = ability.effects();
    let mut more: Vec<String> = Vec::new();
    if w20b_has_fight_after_counter(effects) {
        for text in texts.iter() {
            for (written, said) in [
                (". ~ fights ", ", then it fights "),
                (", then ~ fights ", ", then it fights "),
            ] {
                if text.matches(written).count() == 1 {
                    more.push(text.replacen(written, said, 1));
                }
            }
        }
    }
    texts.extend(more);
    let mut linked: Vec<String> = w20b_sacrifice_texts(ability);
    for text in texts.iter() {
        if w20b_is_reflexive_payment(effects) {
            if text.matches(". If you do, ").count() == 1 {
                linked.push(text.replacen(". If you do, ", ". When you do, ", 1));
            }
        } else if w20b_has_keyword_action_link(effects) {
            linked.extend(w20b_after_action(text, "surveil "));
            linked.extend(w20b_after_action(text, "scry "));
        }
    }
    texts.extend(linked);
}

/// A counter on the card then a fight of the card, in the steps or in the steps of a payment.
fn w20b_has_fight_after_counter(effects: &[Effect]) -> bool {
    let steps = match effects {
        [Effect::Pay { paid, .. }] => paid.as_slice(),
        other => other,
    };
    matches!(
        steps,
        [
            Effect::PutCounters { on: Ref::This, .. },
            Effect::Fight {
                what: Ref::This,
                ..
            }
        ]
    )
}

/// One payment whose steps choose a target: the reflexive trigger of a cost.
fn w20b_is_reflexive_payment(effects: &[Effect]) -> bool {
    matches!(
        effects,
        [Effect::Pay { who: Ref::You, paid, unpaid, .. }]
            if unpaid.is_empty() && w20b_uses_target(paid)
    )
}

/// "S. When you do, T." and "S. If you do, T." for a sacrifice S and the steps T that depend on it: the steps after the
/// link are worded as a spell of their own, as the renderers write a search only as a step of an ability.
fn w20b_sacrifice_texts(ability: &Ability) -> Vec<String> {
    let [
        sacrifice @ Effect::Sacrifice { .. },
        Effect::If {
            condition: Condition::Performed,
            then,
            otherwise,
        },
    ] = ability.effects()
    else {
        return Vec::new();
    };
    let mut head = ability.clone();
    match &mut head {
        Ability::Triggered { effects, .. } if otherwise.is_empty() => {
            *effects = vec![sacrifice.clone()];
        }
        _ => return Vec::new(),
    }
    let tail = Ability::Spell {
        targets: Vec::new(),
        effects: then.clone(),
    };
    let mut texts = Vec::new();
    for first in variants(&head) {
        for body in variants(&tail) {
            for link in ["When you do, ", "If you do, "] {
                texts.push(format!("{first} {link}{}", w20b_lowered(&body)));
            }
        }
    }
    texts
}

/// The sentence with its first letter in lower case.
fn w20b_lowered(sentence: &str) -> String {
    let mut letters = sentence.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(letters).collect()
    })
}

/// A surveil or scry followed by steps.
fn w20b_has_keyword_action_link(effects: &[Effect]) -> bool {
    matches!(
        effects,
        [Effect::Surveil { .. } | Effect::Scry { .. }, _, ..]
    )
}

/// The text with "When you do, " after the sentence of the keyword action ("surveil 4. You may return ..."): the next
/// sentence starts in lower case.
fn w20b_after_action(text: &str, action: &str) -> Option<String> {
    let at = text.find(action)?;
    let rest = &text[at + action.len()..];
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    (digits > 0).then_some(())?;
    let tail = rest[digits..].strip_prefix(". ")?;
    let mut chars = tail.chars();
    let first = chars.next()?;
    let head = &text[..at + action.len() + digits];
    Some(format!(
        "{head}. When you do, {}{}",
        first.to_lowercase(),
        chars.as_str()
    ))
}
