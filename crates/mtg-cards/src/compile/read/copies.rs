//! Copies: "create a token that's a copy of target creature you control", "create two tokens that are copies of it,
//! except they have haste", "populate" and "You may have this creature enter as a copy of any creature on the
//! battlefield, except it's an Illusion in addition to its other types", read into `TokenRef::CopyOf` and
//! `ReplaceBy::AsCopy` and written back the same way. The exceptions are the changes a copy effect keeps in its
//! copiable values: a type or subtype, keywords, a quoted ability, colors, a fixed power and toughness.

use std::sync::Arc;

use mtg_core::card::{CardType, Subtype};

use super::abilities;
use super::amounts::{count_word, number_word};
use super::create::colors_text;
use super::render;
use super::selectors::{
    Form, card_type, color, granted_keywords, granted_text, phrase, phrase_text, subtype, target,
    type_name,
};
use super::tokens::Reader;
use super::triggers::article;
use crate::ability::Ability;
use crate::effect::{Effect, TokenRef};
use crate::modify::Modification;
use crate::refs::{Bounds, Pred, Ref, Selector, Targetable};
use crate::trigger::{ReplaceBy, ReplaceWhen, Step, TriggerPattern};
use crate::value::Amount;

/// The name the token of a copy that a delayed effect acts on is bound to.
pub(super) const COPY: &str = "copy";

/// The name the token that populate copies is chosen as.
const CHOSEN: &str = "token";

/// "create a token that's a copy of target creature you control", "create two tokens that are copies of it, except
/// they have haste", also with the sentence that exiles or sacrifices the copy at the next end step.
pub fn token_copy(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("create")?;
    let (count, plural) = if r.words("a token that's a copy of") {
        (Amount::one(), false)
    } else {
        let count = if r.x_free && r.word("x") {
            Amount::X
        } else {
            let number = number_word(r.take_word()?).filter(|number| *number > 1)?;
            Amount::Fixed(i32::try_from(number).ok()?)
        };
        r.need("tokens that are copies of")?;
        (count, true)
    };
    let of = copied(r)?;
    let except = r
        .attempt(|r| {
            r.punct(',').then_some(())?;
            exceptions(r, plural)
        })
        .unwrap_or_default();
    let delay = r.attempt(|r| {
        r.punct('.').then_some(())?;
        let verb = ["exile", "sacrifice"]
            .into_iter()
            .find(|verb| r.word(verb))?;
        r.word(if plural { "them" } else { "it" }).then_some(())?;
        r.need("at the beginning of the next end step")?;
        Some(verb)
    });
    let create = Effect::CreateToken {
        who: Ref::You,
        token: TokenRef::CopyOf { of, except },
        count,
        tapped: false,
        attacking: false,
        bind: delay.map(|_| COPY.to_owned()),
    };
    let copy = Ref::Bound(COPY.to_owned());
    Some(match delay {
        None => vec![create],
        Some(verb) => vec![
            create,
            Effect::DelayedTrigger {
                when: TriggerPattern::StepBegins {
                    step: Step::End,
                    whose: Ref::EachPlayer,
                },
                once: true,
                effects: vec![if verb == "exile" {
                    Effect::Exile { what: copy }
                } else {
                    Effect::Sacrifice {
                        who: Ref::You,
                        filter: Selector(vec![Pred::Is(copy)]),
                        count: Amount::one(),
                    }
                }],
            },
        ],
    })
}

/// What a token is a copy of: the card itself, what it enchants or equips, what the sentence before acted on, or one
/// target permanent.
fn copied(r: &mut Reader<'_>) -> Option<Ref> {
    if r.source() {
        return Some(Ref::This);
    }
    if r.word("it") || r.words("that creature") || r.words("that permanent") {
        return r.it.clone();
    }
    if r.attempt(|r| {
        ((r.word("enchanted") || r.word("equipped")) && r.word("creature")).then_some(())
    })
    .is_some()
    {
        return Some(Ref::Attached);
    }
    r.attempt(|r| {
        let of = target(r)?;
        let spec = r.targets.first()?;
        let permanent = spec.bounds.is_one()
            && matches!(&spec.what, Targetable::Object(selector)
                if !selector.0.iter().any(|pred| matches!(pred, Pred::In(_) | Pred::Spell)));
        permanent.then_some(of)
    })
}

/// "populate": a creature token you control is chosen and a token that is a copy of it is created.
pub fn populate(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.word("populate").then_some(populate_steps())
}

/// ", then populate" after the sentence before it.
pub fn then_populate(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.punct(',').then_some(())?;
    r.need("then")?;
    populate(r)
}

/// The two steps of populate.
fn populate_steps() -> Vec<Effect> {
    vec![
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(populated()),
            count: Bounds::default(),
            bind: CHOSEN.to_owned(),
            rest_as: None,
        },
        Effect::CreateToken {
            who: Ref::You,
            token: TokenRef::CopyOf {
                of: Ref::Bound(CHOSEN.to_owned()),
                except: Vec::new(),
            },
            count: Amount::one(),
            tapped: false,
            attacking: false,
            bind: None,
        },
    ]
}

/// The tokens populate chooses from.
fn populated() -> Selector {
    Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::Token,
        Pred::ControlledBy(Ref::You),
    ])
}

/// Whether the effects begin with the two steps of populate.
pub(super) fn is_populate(effects: &[Effect]) -> bool {
    effects.starts_with(&populate_steps())
}

/// "You may have this creature enter as a copy of any creature on the battlefield, except it's an Illusion in
/// addition to its other types."
pub fn enters_as_copy(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("you may have")?;
    r.source().then_some(())?;
    r.need("enter as a copy of")?;
    (r.word("a") || r.word("an") || r.word("any")).then_some(())?;
    let of = phrase(r, Form::Target)?;
    r.words("on the battlefield");
    let except = r
        .attempt(|r| {
            r.punct(',').then_some(())?;
            exceptions(r, false)
        })
        .unwrap_or_default();
    Some(Ability::Replacement {
        when: ReplaceWhen::Enters {
            what: Selector::this(),
        },
        by: ReplaceBy::AsCopy { of, except },
        condition: None,
        from: crate::refs::ZoneKind::Battlefield,
    })
}

/// "except it's an Illusion in addition to its other types and it has flying", "except they have haste": the parts
/// joined by "and", each one change to the copiable values. The words about what the copies are ("they're") are not
/// read.
fn exceptions(r: &mut Reader<'_>, plural: bool) -> Option<Vec<Modification>> {
    r.need("except")?;
    let mut mods = Vec::new();
    loop {
        if plural {
            r.need("they have")?;
            has(r, &mut mods)?;
        } else if r.word("it's") {
            mods.push(is(r)?);
        } else {
            r.need("it has")?;
            has(r, &mut mods)?;
        }
        if !r.word("and") {
            return Some(mods);
        }
    }
}

/// What follows "it has": keywords, or an ability in quotes.
fn has(r: &mut Reader<'_>, mods: &mut Vec<Modification>) -> Option<()> {
    if let Some(ability) = r.quoted(abilities::triggered) {
        mods.push(Modification::GrantAbility(Arc::new(ability)));
        return Some(());
    }
    mods.extend(
        granted_keywords(r)?
            .into_iter()
            .map(Modification::AddKeyword),
    );
    while let Some(more) = r.attempt(|r| {
        r.word("and").then_some(())?;
        granted_keywords(r)
    }) {
        mods.extend(more.into_iter().map(Modification::AddKeyword));
    }
    Some(())
}

/// What follows "it's": a type or subtype in addition to the others, colors, or a power and toughness.
fn is(r: &mut Reader<'_>) -> Option<Modification> {
    if let Some(number) = r.number() {
        r.punct('/').then_some(())?;
        let toughness = r.number()?;
        return Some(Modification::SetPT {
            power: Amount::Fixed(i32::try_from(number).ok()?),
            toughness: Amount::Fixed(i32::try_from(toughness).ok()?),
        });
    }
    if r.word("colorless") {
        return Some(Modification::SetColors(Vec::new()));
    }
    if let Some(first) = r.peek_word().and_then(color) {
        r.skip();
        let mut colors = vec![first];
        while let Some(more) = r.attempt(|r| {
            r.word("and").then_some(())?;
            let found = r.peek_word().and_then(color)?;
            r.skip();
            Some(found)
        }) {
            colors.push(more);
        }
        return Some(Modification::SetColors(colors));
    }
    (r.word("a") || r.word("an")).then_some(())?;
    let modification = if let Some(name) = subtype(r) {
        Modification::AddSubtype(Subtype(name))
    } else {
        let found = r.peek_word().and_then(card_type)?;
        r.skip();
        Modification::AddType(found)
    };
    r.need("in addition to its other types")?;
    Some(modification)
}

/// The exceptions as a card words them, with "except" and without a comma before it; `None` for a change the
/// readers do not write.
pub(super) fn exceptions_text(mods: &[Modification], plural: bool) -> Option<String> {
    let has = if plural { "they have" } else { "it has" };
    let mut parts: Vec<String> = Vec::new();
    let mut rest = mods;
    while let Some(first) = rest.first() {
        // Keywords in a row are one phrase.
        let run = rest
            .iter()
            .take_while(|found| matches!(found, Modification::AddKeyword(_)))
            .count()
            .max(1);
        let (these, after) = rest.split_at_checked(run)?;
        rest = after;
        parts.push(match first {
            Modification::AddKeyword(_) => format!("{has} {}", granted_text(these)?),
            Modification::GrantAbility(ability) => {
                format!("{has} \"{}\"", render::ability(ability)?)
            }
            _ if plural => return None,
            Modification::AddType(found) => {
                let name = type_name(*found);
                format!(
                    "it's {} {name} in addition to its other types",
                    article(name)
                )
            }
            Modification::AddSubtype(Subtype(name)) => {
                format!(
                    "it's {} {name} in addition to its other types",
                    article(name)
                )
            }
            Modification::SetColors(colors) => format!("it's {}", colors_text(colors)),
            Modification::SetPT {
                power: Amount::Fixed(power),
                toughness: Amount::Fixed(toughness),
            } if *power >= 0 && *toughness >= 0 => format!("it's {power}/{toughness}"),
            _ => return None,
        });
    }
    (!parts.is_empty()).then(|| format!("except {}", parts.join(" and ")))
}

/// A token copy as a sentence without its first capital and period; `of` says what is copied, and `delayed` that
/// the sentence that follows acts on the copy.
pub(super) fn token_text(effect: &Effect, of: &str, delayed: bool) -> Option<String> {
    let Effect::CreateToken {
        who: Ref::You,
        token: TokenRef::CopyOf { except, .. },
        count,
        tapped: false,
        attacking: false,
        bind,
    } = effect
    else {
        return None;
    };
    if bind.is_some() != delayed || bind.as_deref().is_some_and(|name| name != COPY) {
        return None;
    }
    let (head, plural) = match count {
        Amount::Fixed(1) => ("a token that's a copy of".to_owned(), false),
        Amount::Fixed(number) if *number > 1 => (
            format!(
                "{} tokens that are copies of",
                count_word(u32::try_from(*number).ok()?)?
            ),
            true,
        ),
        Amount::X => ("X tokens that are copies of".to_owned(), true),
        _ => return None,
    };
    let tail = if except.is_empty() {
        String::new()
    } else {
        format!(", {}", exceptions_text(except, plural)?)
    };
    Some(format!("create {head} {of}{tail}"))
}

/// The sentence that acts on the copy at the next end step: "exile it at the beginning of the next end step".
pub(super) fn delay_text(created: &Effect, delayed: &Effect) -> Option<String> {
    let Effect::CreateToken { count, .. } = created else {
        return None;
    };
    let Effect::DelayedTrigger {
        when:
            TriggerPattern::StepBegins {
                step: Step::End,
                whose: Ref::EachPlayer,
            },
        once: true,
        effects,
    } = delayed
    else {
        return None;
    };
    let copy = Ref::Bound(COPY.to_owned());
    let verb = match effects.as_slice() {
        [Effect::Exile { what }] if *what == copy => "exile",
        [
            Effect::Sacrifice {
                who: Ref::You,
                filter,
                count,
            },
        ] if filter.0 == [Pred::Is(copy)] && count.is_one() => "sacrifice",
        _ => return None,
    };
    let pronoun = if *count == Amount::one() {
        "it"
    } else {
        "them"
    };
    Some(format!(
        "{verb} {pronoun} at the beginning of the next end step"
    ))
}

/// "a creature you control" or "any creature on the battlefield": the permanents a permanent may enter as a copy of.
fn choice_text(of: &Selector) -> Option<String> {
    let text = phrase_text(of, Form::Target)?;
    if of.0.contains(&Pred::ControlledBy(Ref::You)) {
        Some(format!("{} {text}", article(&text)))
    } else {
        Some(format!("any {text} on the battlefield"))
    }
}

/// How a permanent that enters as a copy is written, without its first capital and period.
pub(super) fn enters_text(of: &Selector, except: &[Modification]) -> Option<String> {
    let tail = if except.is_empty() {
        String::new()
    } else {
        format!(", {}", exceptions_text(except, false)?)
    };
    Some(format!(
        "you may have ~ enter as a copy of {}{tail}",
        choice_text(of)?
    ))
}
