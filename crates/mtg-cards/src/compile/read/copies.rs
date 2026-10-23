//! Copies: "create a token that's a copy of target creature you control", "create two tokens that are copies of it,
//! except they have haste", "populate" and "You may have this creature enter as a copy of any creature on the
//! battlefield, except it's an Illusion in addition to its other types", read into `TokenRef::CopyOf` and
//! `ReplaceBy::AsCopy` and written back the same way. The exceptions are the changes a copy effect keeps in its
//! copiable values: a type or subtype, keywords, a quoted ability, colors, a fixed power and toughness.

use std::sync::Arc;

use mtg_core::card::{CardType, Subtype};

use super::abilities;
use super::amounts::{count_word, number_word};
use super::copy_spells;
use super::create::colors_text;
use super::render;
use super::selectors::{
    Form, card_type, color, granted_keywords, granted_text, phrase, phrase_text, subtype, type_name,
};
use super::tokens::Reader;
use super::triggers::article;
use crate::ability::Ability;
use crate::effect::{Effect, TokenRef};
use crate::modify::Modification;
use crate::refs::{Bounds, Pred, Ref, Selector};
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
            exceptions(r, if plural { Subject::They } else { Subject::It })
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
        token: TokenRef::CopyOf {
            of,
            except,
            no_mana_cost: false,
        },
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
                    Effect::Exile {
                        what: copy,
                        bind: None,
                        duration: None,
                    }
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
    copy_spells::permanent_target(r)
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
                no_mana_cost: false,
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
            exceptions(r, Subject::It)
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

/// Who the exceptions of a copy are about, which sets the words that name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Subject {
    /// A permanent that enters as a copy or one token: "it's", "it has".
    It,
    /// Several tokens: "they have".
    They,
    /// A copy of a spell: "the copy is", "the copy has".
    Copy,
}

impl Subject {
    /// The words before what the subject is.
    const fn is(self) -> &'static str {
        match self {
            Subject::Copy => "the copy is",
            Subject::It | Subject::They => "it's",
        }
    }

    /// The words before what the subject has.
    const fn has(self) -> &'static str {
        match self {
            Subject::It => "it has",
            Subject::They => "they have",
            Subject::Copy => "the copy has",
        }
    }
}

/// "except it's an Illusion in addition to its other types and it has flying", "except they have haste", "except the
/// copy is a 1/1 Spirit in addition to its other types": the parts joined by "and", each one change to the copiable
/// values. The words about what several tokens are ("they're") are not read.
pub(super) fn exceptions(r: &mut Reader<'_>, subject: Subject) -> Option<Vec<Modification>> {
    r.need("except")?;
    // The copy of a spell is worded with or without "that".
    if subject == Subject::Copy {
        r.word("that");
    }
    let mut mods = Vec::new();
    loop {
        if subject != Subject::They && r.words(subject.is()) {
            is(r, &mut mods)?;
        } else {
            r.need(subject.has())?;
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

/// "7/7": a fixed power and toughness.
fn power_toughness(r: &mut Reader<'_>) -> Option<Modification> {
    r.attempt(|r| {
        let power = r.number()?;
        r.punct('/').then_some(())?;
        let toughness = r.number()?;
        Some(Modification::SetPT {
            power: Amount::Fixed(i32::try_from(power).ok()?),
            toughness: Amount::Fixed(i32::try_from(toughness).ok()?),
        })
    })
}

/// What follows "it's": colors, a power and toughness, or types and subtypes in addition to the others ("an Illusion",
/// "a 1/1 Spirit", "a Golem artifact creature"), which are added in the order they are named.
fn is(r: &mut Reader<'_>, mods: &mut Vec<Modification>) -> Option<()> {
    if let Some(size) = power_toughness(r) {
        mods.push(size);
        return Some(());
    }
    if r.word("colorless") {
        mods.push(Modification::SetColors(Vec::new()));
        return Some(());
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
        mods.push(Modification::SetColors(colors));
        return Some(());
    }
    (r.word("a") || r.word("an")).then_some(())?;
    let size = power_toughness(r);
    let mut kinds = Vec::new();
    loop {
        if let Some(name) = subtype(r) {
            kinds.push(Modification::AddSubtype(Subtype(name)));
        } else if let Some(found) = r.peek_word().and_then(card_type) {
            r.skip();
            kinds.push(Modification::AddType(found));
        } else {
            break;
        }
    }
    (!kinds.is_empty()).then_some(())?;
    r.need("in addition to its other types")?;
    mods.extend(size);
    mods.extend(kinds);
    Some(())
}

/// The words of a run of kinds: "an Illusion", "a 1/1 Spirit", "a Golem artifact creature", then "in addition to its
/// other types"; a size alone is "7/7". `these` begins with the size, if any.
fn kinds_text(subject: Subject, these: &[Modification]) -> Option<String> {
    let (size, kinds) = match these {
        [
            Modification::SetPT {
                power: Amount::Fixed(power),
                toughness: Amount::Fixed(toughness),
            },
            kinds @ ..,
        ] if *power >= 0 && *toughness >= 0 => (Some(format!("{power}/{toughness}")), kinds),
        kinds => (None, kinds),
    };
    if kinds.is_empty() {
        return Some(format!("{} {}", subject.is(), size?));
    }
    let words = kinds
        .iter()
        .map(|kind| match kind {
            Modification::AddType(found) => Some(type_name(*found)),
            Modification::AddSubtype(Subtype(name)) => Some(name.as_str()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let names = words.join(" ");
    let article = article(size.as_deref().unwrap_or(&names));
    let size = size.map(|size| format!("{size} ")).unwrap_or_default();
    Some(format!(
        "{} {article} {size}{names} in addition to its other types",
        subject.is(),
    ))
}

/// The exceptions as a card words them, with "except" and without a comma before it; `None` for a change the
/// readers do not write. A copy of a spell is "except that the copy ...".
pub(super) fn exceptions_text(mods: &[Modification], subject: Subject) -> Option<String> {
    let is_kind = |found: &Modification| {
        matches!(
            found,
            Modification::AddType(_) | Modification::AddSubtype(_)
        )
    };
    let mut parts: Vec<String> = Vec::new();
    let mut rest = mods;
    while let Some(first) = rest.first() {
        // Keywords in a row are one phrase, and so are types and subtypes with the size before them.
        let run = match first {
            Modification::AddKeyword(_) => rest
                .iter()
                .take_while(|found| matches!(found, Modification::AddKeyword(_)))
                .count(),
            Modification::SetPT { .. } => 1 + rest[1..].iter().take_while(|m| is_kind(m)).count(),
            kind if is_kind(kind) => rest.iter().take_while(|m| is_kind(m)).count(),
            _ => 1,
        };
        let (these, after) = rest.split_at_checked(run)?;
        rest = after;
        parts.push(match first {
            Modification::AddKeyword(_) => format!("{} {}", subject.has(), granted_text(these)?),
            Modification::GrantAbility(ability) => {
                format!("{} \"{}\"", subject.has(), render::ability(ability)?)
            }
            _ if subject == Subject::They => return None,
            Modification::SetColors(colors) => {
                format!("{} {}", subject.is(), colors_text(colors))
            }
            Modification::SetPT { .. } | Modification::AddType(_) | Modification::AddSubtype(_) => {
                kinds_text(subject, these)?
            }
            _ => return None,
        });
    }
    let that = if subject == Subject::Copy {
        "that "
    } else {
        ""
    };
    (!parts.is_empty()).then(|| format!("except {that}{}", parts.join(" and ")))
}

/// A token copy as a sentence without its first capital and period; `of` says what is copied, and `delayed` that
/// the sentence that follows acts on the copy.
pub(super) fn token_text(effect: &Effect, of: &str, delayed: bool) -> Option<String> {
    let Effect::CreateToken {
        who: Ref::You,
        token:
            TokenRef::CopyOf {
                except,
                no_mana_cost: false,
                ..
            },
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
        format!(
            ", {}",
            exceptions_text(except, if plural { Subject::They } else { Subject::It })?
        )
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
        [
            Effect::Exile {
                what,
                bind: None,
                duration: None,
            },
        ] if *what == copy => "exile",
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
        format!(", {}", exceptions_text(except, Subject::It)?)
    };
    Some(format!(
        "you may have ~ enter as a copy of {}{tail}",
        choice_text(of)?
    ))
}
