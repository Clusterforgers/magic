//! Copies of spells and abilities: "Copy target instant or sorcery spell, except that the copy is red. You may choose
//! new targets for the copy.", "Copy target activated or triggered ability you control.", "Copy target permanent spell
//! you control." (`Effect::CopySpell`, `Effect::CopyAbility`) and a permanent that becomes a copy until end of turn
//! ("Choose a nonlegendary creature on the battlefield. Target creature becomes a copy of that creature until end of
//! turn.", `Effect::BecomeCopy`), read from the wording and written back the same way.

use mtg_core::card::CardType;

use super::amounts::{count_word, for_each, for_each_text, number_word};
use super::copies::{self, Subject};
use super::selectors::{Form, card_type, phrase, phrase_text, spec_text, target, type_name};
use super::tokens::Reader;
use super::triggers::article;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{AbilityKind, Bounds, Pred, Ref, Selector, TargetSpec, Targetable};
use crate::value::Amount;

/// The name the permanent a copy effect chooses is bound to.
pub(super) const COPIED: &str = "copied";

/// "Copy target instant or sorcery spell. You may choose new targets for the copy.", "Copy target triggered ability
/// you control.", "Copy target permanent spell you control.": the object copied is the clause's one target. After a
/// spell is cast, "copy it" and "copy that spell" are the spell cast ("copy it twice, except the copy isn't red. You may
/// choose new targets for the copies.").
pub fn copy(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("copy")?;
    let (what, ability) = if let Some(what) = cast_spell(r) {
        (what, false)
    } else if let Some(what) = permanent_spell(r) {
        (what, false)
    } else if let Some(what) = ability_target(r) {
        (what, true)
    } else {
        (spell_target(r)?, false)
    };
    let (count, except) = if ability {
        (Amount::one(), Vec::new())
    } else {
        (
            r.attempt(times)
                .or_else(|| r.attempt(for_each))
                .unwrap_or_else(Amount::one),
            r.attempt(spell_exceptions).unwrap_or_default(),
        )
    };
    let copies = if count.is_one() { "copy" } else { "copies" };
    let new_targets = r
        .attempt(|r| {
            r.punct('.').then_some(())?;
            r.need(&format!("you may choose new targets for the {copies}"))
        })
        .is_some();
    Some(vec![if ability {
        Effect::CopyAbility {
            what,
            count,
            new_targets,
        }
    } else {
        Effect::CopySpell {
            who: Ref::You,
            what,
            count,
            new_targets,
            except,
        }
    }])
}

/// "it" or "that spell" after a spell is cast: the spell the trigger is about.
fn cast_spell(r: &mut Reader<'_>) -> Option<Ref> {
    r.cast_spell.then_some(())?;
    (r.word("it") || r.words("that spell")).then_some(if r.cast_itself {
        Ref::This
    } else {
        Ref::Triggering
    })
}

/// "twice", "three times", "X times": how many copies there are.
fn times(r: &mut Reader<'_>) -> Option<Amount> {
    if r.word("twice") {
        return Some(Amount::Fixed(2));
    }
    let count = if r.x_free && r.word("x") {
        Amount::X
    } else {
        let number = number_word(r.take_word()?).filter(|number| *number > 2)?;
        Amount::Fixed(i32::try_from(number).ok()?)
    };
    r.word("times").then_some(count)
}

/// ", except that the copy is red", ", except the copy is an artifact in addition to its other types": what a copy of
/// a spell is besides the spell.
fn spell_exceptions(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    r.punct(',').then_some(())?;
    copies::exceptions(r, Subject::Copy)
}

/// The one target the clause asks for, when it is a single object.
fn one_target(r: &mut Reader<'_>, what: Targetable) -> Option<Ref> {
    r.targets.is_empty().then_some(())?;
    r.targets.push(TargetSpec {
        what,
        bounds: Bounds::default(),
        divide: None,
    });
    Some(Ref::Target(0))
}

/// The instants and sorceries, which a permanent spell is none of.
fn instant_or_sorcery() -> Pred {
    Pred::AnyOf(vec![
        Pred::Type(CardType::Instant),
        Pred::Type(CardType::Sorcery),
    ])
}

/// "target permanent spell you control": a spell that is neither an instant nor a sorcery.
fn permanent_spell(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        r.need("target permanent spell")?;
        let mut preds = vec![Pred::Spell, Pred::Not(Box::new(instant_or_sorcery()))];
        if r.words("you control") {
            preds.push(Pred::ControlledBy(Ref::You));
        }
        one_target(r, Targetable::Object(Selector(preds)))
    })
}

/// "target activated ability", "target triggered ability you control", "target activated or triggered ability".
fn ability_target(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        r.need("target")?;
        let kind = if r.words("activated or triggered") {
            Pred::AnyOf(vec![
                Pred::Ability(AbilityKind::Activated),
                Pred::Ability(AbilityKind::Triggered),
            ])
        } else if r.word("activated") {
            Pred::Ability(AbilityKind::Activated)
        } else if r.word("triggered") {
            Pred::Ability(AbilityKind::Triggered)
        } else {
            return None;
        };
        r.need("ability")?;
        let mut preds = vec![kind];
        if r.words("you control") {
            preds.push(Pred::ControlledBy(Ref::You));
        }
        one_target(r, Targetable::Object(Selector(preds)))
    })
}

/// "target instant or sorcery spell", "target spell you control": a spell on the stack.
fn spell_target(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        let what = target(r)?;
        let spec = r.targets.first()?;
        let spell = spec.bounds.is_one()
            && matches!(&spec.what, Targetable::Object(selector)
                if selector.0.first() == Some(&Pred::Spell)
                    && !selector.0.iter().any(|pred| matches!(pred, Pred::In(_))));
        spell.then_some(what)
    })
}

/// The target of a copy as the card words it: the spells and abilities `spell_target`, `permanent_spell` and
/// `ability_target` read.
fn target_text(spec: &TargetSpec) -> Option<String> {
    if !spec.bounds.is_one() || spec.divide.is_some() {
        return None;
    }
    let Targetable::Object(selector) = &spec.what else {
        return None;
    };
    let (kind, rest) = match selector.0.as_slice() {
        [Pred::Spell, not, rest @ ..] if *not == Pred::Not(Box::new(instant_or_sorcery())) => {
            ("permanent spell", rest)
        }
        [Pred::Spell, ..] => return spec_text(spec),
        [Pred::Ability(AbilityKind::Activated), rest @ ..] => ("activated ability", rest),
        [Pred::Ability(AbilityKind::Triggered), rest @ ..] => ("triggered ability", rest),
        [Pred::AnyOf(kinds), rest @ ..]
            if kinds
                == &[
                    Pred::Ability(AbilityKind::Activated),
                    Pred::Ability(AbilityKind::Triggered),
                ] =>
        {
            ("activated or triggered ability", rest)
        }
        _ => return None,
    };
    match rest {
        [] => Some(format!("target {kind}")),
        [Pred::ControlledBy(Ref::You)] => Some(format!("target {kind} you control")),
        _ => None,
    }
}

/// A copy of a spell or an ability as a sentence without its first capital and period, with the sentence about the
/// new targets after it; `spec` is the clause's target.
pub(super) fn copy_text(effect: &Effect, spec: &TargetSpec) -> Option<String> {
    let (what, count, new_targets, except, is_spell) = match effect {
        Effect::CopySpell {
            who: Ref::You,
            what,
            count,
            new_targets,
            except,
        } => (what, count, *new_targets, except.as_slice(), true),
        Effect::CopyAbility {
            what,
            count,
            new_targets,
        } if count.is_one() => (what, count, *new_targets, [].as_slice(), false),
        _ => return None,
    };
    let copied = target_text(spec)?;
    // A copy of a spell is of a spell, a copy of an ability of an ability.
    let target_is_spell = matches!(&spec.what, Targetable::Object(selector)
        if selector.0.first() == Some(&Pred::Spell));
    if *what != Ref::Target(0) || target_is_spell != is_spell {
        return None;
    }
    Some(format!(
        "copy {copied}{}",
        tail_text(count, new_targets, except)?
    ))
}

/// A copy of the spell a trigger is about as a sentence without its first capital and period: "copy it", "copy it
/// twice, except that the copy is red. You may choose new targets for the copies".
pub(super) fn cast_text(effect: &Effect) -> Option<String> {
    let Effect::CopySpell {
        what: Ref::Triggering | Ref::This,
        who: Ref::You,
        count,
        new_targets,
        except,
    } = effect
    else {
        return None;
    };
    Some(format!(
        "copy it{}",
        tail_text(count, *new_targets, except)?
    ))
}

/// How many copies there are, what they are besides the spell and the sentence about new targets, after the words
/// that name what is copied.
fn tail_text(count: &Amount, new_targets: bool, except: &[Modification]) -> Option<String> {
    let times = match count {
        Amount::Fixed(1) => String::new(),
        Amount::Fixed(2) => " twice".to_owned(),
        Amount::Fixed(number) if *number > 2 => {
            format!(" {} times", count_word(u32::try_from(*number).ok()?)?)
        }
        Amount::X => " X times".to_owned(),
        counted @ Amount::CommanderCasts(_) => format!(" for each {}", for_each_text(counted)?),
        _ => return None,
    };
    let except = if except.is_empty() {
        String::new()
    } else {
        format!(", {}", copies::exceptions_text(except, Subject::Copy)?)
    };
    let again = match (new_targets, *count == Amount::one()) {
        (false, _) => "",
        (true, true) => ". You may choose new targets for the copy",
        (true, false) => ". You may choose new targets for the copies",
    };
    Some(format!("{times}{except}{again}"))
}

/// "~ becomes a copy of target creature until end of turn", "it becomes a copy of another target creature until end
/// of turn" after an attack: the card is a copy of one target permanent.
pub fn becomes_copy(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    if let Some(effects) = r.attempt(c10_xp_target_becomes_copy) {
        return Some(effects);
    }
    (r.source() || (r.attacker && r.word("it"))).then_some(())?;
    r.need("becomes a copy of")?;
    let of = permanent_target(r)?;
    let duration = c10_xp_copy_duration(r);
    let except = r
        .attempt(|r| {
            r.punct(',').then_some(())?;
            copies::exceptions(r, Subject::It)
        })
        .unwrap_or_default();
    Some(vec![Effect::BecomeCopy {
        what: Ref::This,
        of,
        except,
        duration,
    }])
}

/// "until end of turn", "until your next turn" or nothing: how long a copy lasts.
fn c10_xp_copy_duration(r: &mut Reader<'_>) -> Duration {
    if r.words("until end of turn") {
        Duration::UntilEndOfTurn
    } else if r.words("until your next turn") {
        Duration::UntilYourNextTurn
    } else {
        Duration::Forever
    }
}

/// "Target Shapeshifter becomes a copy of target creature until your next turn.": the first target permanent becomes
/// a copy of the second.
fn c10_xp_target_becomes_copy(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let what = permanent_target(r)?;
    r.need("becomes a copy of")?;
    let of = super::selectors::second_target(r)?;
    (of == Ref::Target(1)).then_some(())?;
    let spec = r.targets.get(1)?;
    (spec.bounds.is_one()
        && matches!(&spec.what, Targetable::Object(selector)
            if !selector.0.iter().any(|pred| matches!(pred, Pred::In(_) | Pred::Spell))))
    .then_some(())?;
    let duration = c10_xp_copy_duration(r);
    Some(vec![Effect::BecomeCopy {
        what,
        of,
        except: Vec::new(),
        duration,
    }])
}

/// The words of how long a copy lasts, after the copied object; `None` for a duration no card says.
pub(super) fn c10_xp_copy_duration_text(duration: Duration) -> Option<&'static str> {
    match duration {
        Duration::UntilEndOfTurn => Some(" until end of turn"),
        Duration::UntilYourNextTurn => Some(" until your next turn"),
        Duration::Forever => Some(""),
        _ => None,
    }
}

/// One target permanent, "another target creature" also: not a spell, not a card in a zone.
pub(super) fn permanent_target(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        let another = r.word("another");
        let of = target(r)?;
        if another
            && let Some(TargetSpec {
                what: Targetable::Object(selector),
                ..
            }) = r.targets.first_mut()
        {
            selector.0.push(Pred::Other);
        }
        let spec = r.targets.first()?;
        let permanent = spec.bounds.is_one()
            && matches!(&spec.what, Targetable::Object(selector)
                if !selector.0.iter().any(|pred| matches!(pred, Pred::In(_) | Pred::Spell)));
        permanent.then_some(of)
    })
}

/// "Choose a nonlegendary creature on the battlefield. Target creature becomes a copy of that creature until end of
/// turn.": the permanent is chosen first, and the target becomes a copy of it.
pub fn choose_and_become(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("choose")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let from = phrase(r, Form::Target)?;
    r.need("on the battlefield")?;
    r.punct('.').then_some(())?;
    let what = permanent_target(r)?;
    r.need("becomes a copy of")?;
    let noun = chosen_noun(&from)?;
    r.need(&format!("that {noun}")).or_else(|| r.need("it"))?;
    r.need("until end of turn")?;
    Some(vec![
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(from),
            count: Bounds::default(),
            bind: COPIED.to_owned(),
            rest_as: None,
        },
        Effect::BecomeCopy {
            what,
            of: Ref::Bound(COPIED.to_owned()),
            except: Vec::new(),
            duration: Duration::UntilEndOfTurn,
        },
    ])
}

/// The noun the second sentence calls the chosen permanent: its one card type, else "permanent".
pub(super) fn chosen_noun(from: &Selector) -> Option<&'static str> {
    let mut types = from.0.iter().filter_map(|pred| match pred {
        Pred::Type(found) => Some(*found),
        _ => None,
    });
    let first = types.next();
    if types.next().is_some() || from.0.iter().any(|pred| matches!(pred, Pred::AnyOf(_))) {
        return None;
    }
    let noun = first.map_or("permanent", type_name);
    // The word must read back as the type it names.
    (first.is_none() || card_type(noun) == first).then_some(noun)
}

/// The two sentences of a choice of a permanent and the target that becomes a copy of it, without their first
/// capitals and periods; `target` names the clause's target.
pub(super) fn become_texts(choose: &Effect, copy: &Effect, target: &str) -> Option<[String; 2]> {
    let (
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(from),
            count,
            bind,
            rest_as: None,
        },
        Effect::BecomeCopy {
            what: Ref::Target(0),
            of: Ref::Bound(name),
            except,
            duration: Duration::UntilEndOfTurn,
        },
    ) = (choose, copy)
    else {
        return None;
    };
    if bind != COPIED || name != COPIED || !count.is_one() || !except.is_empty() {
        return None;
    }
    let chosen = phrase_text(from, Form::Target)?;
    Some([
        format!("choose {} {chosen} on the battlefield", article(&chosen)),
        format!(
            "{target} becomes a copy of that {} until end of turn",
            chosen_noun(from)?
        ),
    ])
}

/// "~ becomes a copy of target creature until end of turn", with `target` naming the clause's target.
pub(super) fn becomes_text(effect: &Effect, target: &str) -> Option<String> {
    let Effect::BecomeCopy {
        what: Ref::This,
        of: Ref::Target(0),
        except,
        duration,
    } = effect
    else {
        return None;
    };
    let until = c10_xp_copy_duration_text(duration.clone())?;
    let except = if except.is_empty() {
        String::new()
    } else {
        format!(", {}", copies::exceptions_text(except, Subject::It)?)
    };
    Some(format!("~ becomes a copy of {target}{until}{except}"))
}
