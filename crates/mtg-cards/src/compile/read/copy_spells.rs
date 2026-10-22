//! Copies of spells and abilities: "Copy target instant or sorcery spell, except that the copy is red. You may choose
//! new targets for the copy.", "Copy target activated or triggered ability you control.", "Copy target permanent spell
//! you control." (`Effect::CopySpell`, `Effect::CopyAbility`) and a permanent that becomes a copy until end of turn
//! ("Choose a nonlegendary creature on the battlefield. Target creature becomes a copy of that creature until end of
//! turn.", `Effect::BecomeCopy`), read from the wording and written back the same way.

use mtg_core::card::CardType;

use super::copies;
use super::create::colors_text;
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
/// you control.", "Copy target permanent spell you control.": the object copied is the clause's one target.
pub fn copy(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("copy")?;
    let (what, ability) = if let Some(what) = permanent_spell(r) {
        (what, false)
    } else if let Some(what) = ability_target(r) {
        (what, true)
    } else {
        (spell_target(r)?, false)
    };
    let twice = r.word("twice");
    let count = if twice {
        Amount::Fixed(2)
    } else {
        Amount::one()
    };
    let except = if ability {
        Vec::new()
    } else {
        r.attempt(spell_exception).unwrap_or_default()
    };
    let new_targets = r
        .attempt(|r| {
            r.punct('.').then_some(())?;
            r.need(if twice {
                "you may choose new targets for the copies"
            } else {
                "you may choose new targets for the copy"
            })
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
            what,
            count,
            new_targets,
            except,
        }
    }])
}

/// ", except that the copy is red": the colors a copy of a spell has.
fn spell_exception(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    r.punct(',').then_some(())?;
    r.need("except that the copy is")?;
    match copies::is(r)? {
        colors @ Modification::SetColors(_) => Some(vec![colors]),
        _ => None,
    }
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
            what,
            count,
            new_targets,
            except,
        } => (what, count, *new_targets, except.as_slice(), true),
        Effect::CopyAbility {
            what,
            count,
            new_targets,
        } => (what, count, *new_targets, [].as_slice(), false),
        _ => return None,
    };
    let copied = target_text(spec)?;
    // A copy of a spell is of a spell, a copy of an ability of an ability.
    let target_is_spell = matches!(&spec.what, Targetable::Object(selector)
        if selector.0.first() == Some(&Pred::Spell));
    if *what != Ref::Target(0) || target_is_spell != is_spell {
        return None;
    }
    let colors = match except {
        [] => String::new(),
        [Modification::SetColors(colors)] => {
            format!(", except that the copy is {}", colors_text(colors))
        }
        _ => return None,
    };
    let twice = match count {
        Amount::Fixed(1) => "",
        Amount::Fixed(2) => " twice",
        _ => return None,
    };
    let again = if new_targets {
        if twice.is_empty() {
            ". You may choose new targets for the copy"
        } else {
            ". You may choose new targets for the copies"
        }
    } else {
        ""
    };
    Some(format!("copy {copied}{twice}{colors}{again}"))
}

/// "~ becomes a copy of target creature until end of turn": the card is a copy of one target permanent.
pub fn becomes_copy(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.source().then_some(())?;
    r.need("becomes a copy of")?;
    let of = permanent_target(r)?;
    r.need("until end of turn")?;
    Some(vec![Effect::BecomeCopy {
        what: Ref::This,
        of,
        except: Vec::new(),
        duration: Duration::UntilEndOfTurn,
    }])
}

/// One target permanent: not a spell, not a card in a zone.
fn permanent_target(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        let of = target(r)?;
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
fn chosen_noun(from: &Selector) -> Option<&'static str> {
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
        duration: Duration::UntilEndOfTurn,
    } = effect
    else {
        return None;
    };
    except
        .is_empty()
        .then(|| format!("~ becomes a copy of {target} until end of turn"))
}
