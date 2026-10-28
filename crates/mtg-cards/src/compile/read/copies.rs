//! Copies: "create a token that's a copy of target creature you control", "create two tokens that are copies of it,
//! except they have haste", "populate" and "You may have this creature enter as a copy of any creature on the
//! battlefield, except it's an Illusion in addition to its other types", read into `TokenRef::CopyOf` and
//! `ReplaceBy::AsCopy` and written back the same way. The exceptions are the changes a copy effect keeps in its
//! copiable values: a type or subtype, keywords, a quoted ability, colors, a fixed power and toughness.

use std::sync::Arc;

use mtg_core::card::{CardType, Subtype, Supertype};

use super::abilities;
use super::amounts::{count_word, number_word};
use super::copy_spells;
use super::create::colors_text;
use super::defining;
use super::render;
use super::selectors::{
    Form, card_type, color, granted_keywords, granted_text, phrase, phrase_in, phrase_text,
    subtype, target, type_name,
};
use super::token_binds::TOKENS;
use super::tokens::Reader;
use super::triggers::article;
use super::where_x;
use crate::ability::Ability;
use crate::effect::{Effect, TokenRef};
use crate::modify::{Duration, Modification};
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen, Step, TriggerPattern};
use crate::value::Amount;

/// The name the token of a copy that a delayed effect acts on is bound to.
pub(super) const COPY: &str = "copy";

/// The name the token that populate copies is chosen as.
const CHOSEN: &str = "token";

/// How many tokens a copy makes and how they enter.
struct Made {
    /// The number of tokens.
    count: Amount,
    /// Whether the sentences after the creation say "they" and "those tokens".
    plural: bool,
    /// Whether they enter tapped.
    tapped: bool,
    /// Whether they enter attacking.
    attacking: bool,
}

/// "a token that's a copy of", "a tapped and attacking token that's a copy of", "X tokens that are copies of": the
/// words that open a token copy after "create".
fn made(r: &mut Reader<'_>) -> Option<Made> {
    let (mut tapped, mut attacking) = (false, false);
    let (count, plural) = if r.words("a token that's a copy of") {
        (Amount::one(), false)
    } else if r.words("a tapped token that's a copy of") {
        tapped = true;
        (Amount::one(), false)
    } else if r.words("a tapped and attacking token that's a copy of") {
        (tapped, attacking) = (true, true);
        (Amount::one(), false)
    } else {
        let count = if r.x() {
            Amount::X
        } else {
            let number = number_word(r.take_word()?).filter(|number| *number > 1)?;
            Amount::Fixed(i32::try_from(number).ok()?)
        };
        r.need("tokens that are copies of")?;
        (count, true)
    };
    Some(Made {
        count,
        plural,
        tapped,
        attacking,
    })
}

/// ", where X is the number of creatures defending player controls" after X copies: the count, read here because the
/// sentences that act on the copies follow it.
fn where_count(r: &mut Reader<'_>, count: Amount) -> Option<Amount> {
    if count != Amount::X || !r.where_x {
        return Some(count);
    }
    let found = r.attempt(where_x::amount)?;
    // The sentence's where clause is read: none is left for `where_x::bind` to find.
    r.where_x = false;
    Some(found)
}

/// "create a token that's a copy of target creature you control", "create two tokens that are copies of it, except
/// they have haste", also with the sentence that exiles or sacrifices the copy at the next end step.
pub fn token_copy(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    if let Some(each) = r.attempt(each_copy) {
        return Some(each);
    }
    let chosen = r.attempt(chosen_first).is_some();
    if chosen {
        r.it = Some(Ref::Target(0));
    }
    // "for each card you've discarded this turn, create a token that's a copy of ~": a token for each.
    let each = r.attempt(each_counted);
    r.need("create")?;
    let Made {
        mut count,
        plural,
        mut tapped,
        mut attacking,
    } = made(r)?;
    if let Some(each) = each {
        (count.is_one() && !plural && !tapped).then_some(())?;
        count = each;
    }
    // One token for each object the sentence before chose.
    (!chosen || !(plural || tapped)).then_some(())?;
    let of = copied(r)?;
    // "... and that's tapped and attacking": the token enters attacking a player or planeswalker its controller picks.
    let late = if plural {
        "and that are tapped and attacking"
    } else {
        "and that's tapped and attacking"
    };
    if !tapped && !chosen && r.words(late) {
        (tapped, attacking) = (true, true);
    }
    let except = r
        .attempt(|r| {
            r.punct(',').then_some(())?;
            exceptions(r, if plural { Subject::They } else { Subject::It })
        })
        .unwrap_or_default();
    count = where_count(r, count)?;
    // A token for each of several targets is several tokens to the sentences after it.
    let several = plural
        || !count.is_one()
        || (chosen && r.targets.first().is_some_and(|spec| !spec.bounds.is_one()));
    // ". The tokens enter tapped and attacking."
    if !tapped && r.attempt(|r| copy_enters(r, several)).is_some() {
        (tapped, attacking) = (true, true);
    }
    let gains = r.attempt(|r| {
        r.punct('.').then_some(())?;
        copy_gains(r, several)
    });
    let delay = r.attempt(|r| {
        r.punct('.').then_some(())?;
        copy_delay(r, several)
    });
    let create = Effect::CreateToken {
        who: Ref::You,
        token: TokenRef::CopyOf {
            of,
            except,
            no_mana_cost: false,
        },
        count,
        tapped,
        attacking,
        bind: (gains.is_some() || delay.is_some()).then(|| COPY.to_owned()),
    };
    let copy = Ref::Bound(COPY.to_owned());
    let mut effects = vec![create];
    effects.extend(gains);
    if let Some((verb, step)) = delay {
        effects.push(Effect::DelayedTrigger {
            when: TriggerPattern::StepBegins {
                step,
                whose: Ref::EachPlayer,
            },
            once: true,
            this_turn: false,
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
        });
    }
    Some(effects)
}

/// "Choose target artifact creature you control. For each creature chosen this way,", "Choose up to two target
/// creatures you don't control. For each of those creatures,": the sentence that chooses the targets and the words
/// that make a token for each of them.
fn chosen_first(r: &mut Reader<'_>) -> Option<()> {
    r.need("choose")?;
    target(r)?;
    r.punct('.').then_some(())?;
    r.need("for each")?;
    let spec = r.targets.first()?;
    let Targetable::Object(selector) = &spec.what else {
        return None;
    };
    let noun = chosen_kind(selector)?;
    if spec.bounds.is_one() {
        r.need(noun)?;
        r.need("chosen this way")?;
    } else if !r.words("of them") {
        r.need("of those")?;
        r.need(&format!("{noun}s"))?;
    }
    r.punct(',').then_some(())
}

/// Whether the effect makes one token copy of the clause's target, which a card may word after a sentence that chooses it.
pub(super) fn is_target_copy(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::CreateToken {
            token: TokenRef::CopyOf {
                of: Ref::Target(0), ..
            },
            count,
            tapped: false,
            ..
        } if count.is_one()
    )
}

/// The noun the sentence after a choice calls what was chosen: "creature" for an artifact creature, else the one card
/// type or "permanent".
pub(super) fn chosen_kind(from: &Selector) -> Option<&'static str> {
    if from.0.contains(&Pred::Type(CardType::Creature))
        && !from.0.iter().any(|pred| matches!(pred, Pred::AnyOf(_)))
    {
        return Some("creature");
    }
    copy_spells::chosen_noun(from)
}

/// The words that name the copy in the sentences after its creation: one token or several.
const fn copy_words(plural: bool) -> [&'static str; 3] {
    if plural {
        ["they", "those tokens", "the tokens"]
    } else {
        ["it", "that token", "the token"]
    }
}

/// "It gains haste until end of turn.", "That token gains haste.", "Those tokens gain haste.": the keywords the copy gets
/// (for good when no end is named, since the sentence after it takes the copy away).
fn copy_gains(r: &mut Reader<'_>, plural: bool) -> Option<Effect> {
    copy_words(plural)
        .into_iter()
        .find(|subject| r.words(subject))?;
    r.word(if plural { "gain" } else { "gains" })
        .then_some(())?;
    let mods = gained_keywords(r)?;
    let duration = if r.words("until end of turn") {
        Duration::UntilEndOfTurn
    } else {
        Duration::Forever
    };
    Some(Effect::Continuous {
        affected: Ref::Bound(COPY.to_owned()),
        mods,
        duration,
    })
}

/// "for each card you've discarded this turn," before "create a token that's a copy of ...": what the tokens are counted by.
fn each_counted(r: &mut Reader<'_>) -> Option<Amount> {
    r.need("for each")?;
    let count = defining::died_each(r)?;
    r.punct(',').then_some(())?;
    Some(count)
}

/// ". The token enters tapped and attacking", ". The tokens enter tapped and attacking": the copies enter attacking a
/// player or planeswalker their controller picks.
fn copy_enters(r: &mut Reader<'_>, several: bool) -> Option<()> {
    r.punct('.').then_some(())?;
    r.need(if several {
        "the tokens enter tapped and attacking"
    } else {
        "the token enters tapped and attacking"
    })
}

/// The keywords after "gains": "haste", "flying and haste".
fn gained_keywords(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    let mut mods: Vec<Modification> = granted_keywords(r)?
        .into_iter()
        .map(Modification::AddKeyword)
        .collect();
    while let Some(more) = r.attempt(|r| {
        r.word("and").then_some(())?;
        granted_keywords(r)
    }) {
        mods.extend(more.into_iter().map(Modification::AddKeyword));
    }
    Some(mods)
}

/// "Exile it at the beginning of the next end step.", "Sacrifice those tokens at end of combat.": what is done to the
/// copy and the step it waits for.
fn copy_delay(r: &mut Reader<'_>, plural: bool) -> Option<(&'static str, Step)> {
    let verb = ["exile", "sacrifice"]
        .into_iter()
        .find(|verb| r.word(verb))?;
    copy_objects(plural)
        .into_iter()
        .find(|object| r.words(object))?;
    let step = if r.words("at the beginning of the next end step") {
        Step::End
    } else {
        r.need("at end of combat")?;
        Step::EndCombat
    };
    Some((verb, step))
}

/// "For each token you control, create a token that's a copy of that permanent.": one token for each permanent the
/// group holds; the noun after "that" is the group's one card type, else "permanent".
fn each_copy(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("for each")?;
    let mut from = phrase(r, Form::Count)?;
    // "For each creature target player controls": the group of the player the spell targets.
    if r.targets.is_empty() && r.words("target player controls") {
        r.targets.push(TargetSpec {
            what: Targetable::Player,
            bounds: Bounds::default(),
            divide: None,
        });
        from.0.push(Pred::ControlledBy(Ref::Target(0)));
    }
    from.0
        .iter()
        .any(|pred| matches!(pred, Pred::ControlledBy(_)))
        .then_some(())?;
    r.punct(',').then_some(())?;
    r.need("create a token that's a copy of that")?;
    r.need(copy_spells::chosen_noun(&from)?)?;
    Some(vec![Effect::CreateToken {
        who: Ref::You,
        token: TokenRef::CopyOf {
            of: Ref::Each(from),
            except: Vec::new(),
            no_mana_cost: false,
        },
        count: Amount::one(),
        tapped: false,
        attacking: false,
        bind: None,
    }])
}

/// The sentence `each_copy` reads, without its first capital and period.
pub(super) fn each_text(effect: &Effect, from: &Selector) -> Option<String> {
    let Effect::CreateToken {
        who: Ref::You,
        token:
            TokenRef::CopyOf {
                of: Ref::Each(each),
                except,
                no_mana_cost: false,
            },
        count,
        tapped: false,
        attacking: false,
        bind: None,
    } = effect
    else {
        return None;
    };
    if each != from || !except.is_empty() || !count.is_one() {
        return None;
    }
    Some(format!(
        "for each {}, create a token that's a copy of that {}",
        phrase_text(from, Form::Count)?,
        copy_spells::chosen_noun(from)?
    ))
}

/// What a token is a copy of: the card itself, what it enchants or equips, what the sentence before acted on, or one
/// target permanent.
fn copied(r: &mut Reader<'_>) -> Option<Ref> {
    if r.source() {
        return Some(Ref::This);
    }
    if r.word("it") || r.words("that creature") || r.words("that permanent") {
        // In a trigger of the card's own event, "it" is the card.
        return r
            .it
            .clone()
            .or_else(|| (r.acts_itself || r.attacker).then_some(Ref::This));
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

/// "populate": a creature token you control is chosen and a token that is a copy of it is created. The sentences after
/// it may make the token enter tapped and attacking ("The token enters tapped and attacking.") or give it keywords
/// and sacrifice it at the next end step ("The token created this way gains haste. Sacrifice it at the beginning of
/// the next end step.").
pub fn populate(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.word("populate").then_some(())?;
    let mut steps = populate_steps();
    if r.attempt(populate_enters).is_some() {
        if let Some(Effect::CreateToken {
            tapped, attacking, ..
        }) = steps.get_mut(1)
        {
            (*tapped, *attacking) = (true, true);
        }
    } else if let Some(later) = r.attempt(populate_later) {
        if let Some(Effect::CreateToken { bind, .. }) = steps.get_mut(1) {
            *bind = Some(TOKENS.to_owned());
        }
        steps.extend(later);
    }
    Some(steps)
}

/// ". The token enters tapped and attacking".
fn populate_enters(r: &mut Reader<'_>) -> Option<()> {
    r.punct('.').then_some(())?;
    r.need("the token enters tapped and attacking")
}

/// ". The token created this way gains haste. Sacrifice it at the beginning of the next end step": the keywords the
/// token has for good and its sacrifice.
fn populate_later(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.punct('.').then_some(())?;
    r.need("the token created this way gains")?;
    let mods = gained_keywords(r)?;
    r.punct('.').then_some(())?;
    r.need("sacrifice it at the beginning of the next end step")?;
    let token = Ref::Bound(TOKENS.to_owned());
    Some(vec![
        Effect::Continuous {
            affected: token.clone(),
            mods,
            duration: Duration::Forever,
        },
        Effect::DelayedTrigger {
            when: TriggerPattern::StepBegins {
                step: Step::End,
                whose: Ref::EachPlayer,
            },
            once: true,
            this_turn: false,
            effects: vec![Effect::Sacrifice {
                who: Ref::You,
                filter: Selector(vec![Pred::Is(token)]),
                count: Amount::one(),
            }],
        },
    ])
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
    // "As this artifact enters, you may have it become a copy of any creature on the battlefield until end of turn".
    let until_end_of_turn = r
        .attempt(|r| {
            r.need("as")?;
            r.source().then_some(())?;
            r.need("enters")?;
            r.punct(',').then_some(())
        })
        .is_some();
    r.need("you may have")?;
    let mut tapped = false;
    if until_end_of_turn {
        r.word("it").then_some(())?;
        r.need("become a copy of")?;
    } else {
        r.source().then_some(())?;
        tapped = r.words("enter tapped as a copy of");
        if !tapped {
            r.need("enter as a copy of")?;
        }
    }
    (r.word("a") || r.word("an") || r.word("any")).then_some(())?;
    let mut of = phrase_in(r, Form::Target, true)?;
    // "any creature card in a graveyard".
    if r.words("in a graveyard") {
        of.0.push(Pred::In(ZoneKind::Graveyard));
    } else {
        r.words("on the battlefield");
    }
    if until_end_of_turn {
        r.need("until end of turn")?;
    }
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
        by: ReplaceBy::AsCopy {
            of,
            except,
            tapped,
            until_end_of_turn,
        },
        condition: None,
        from: ZoneKind::Battlefield,
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

    /// The words before what the subject is not.
    const fn isnt(self) -> Option<&'static str> {
        match self {
            Subject::It => Some("it isn't"),
            Subject::Copy => Some("the copy isn't"),
            Subject::They => None,
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
    part(r, subject, &mut mods)?;
    loop {
        if r.word("and") {
            part(r, subject, &mut mods)?;
        } else if let Some(more) = r.attempt(|r| listed(r, subject)) {
            mods.extend(more);
        } else {
            return Some(mods);
        }
    }
}

/// One part of the exceptions, with the words that name the subject before it.
fn part(r: &mut Reader<'_>, subject: Subject, mods: &mut Vec<Modification>) -> Option<()> {
    if subject != Subject::They && r.words(subject.is()) {
        is(r, mods)
    } else if let Some(gone) = r.attempt(|r| not_legendary(r, subject)) {
        mods.push(gone);
        Some(())
    } else {
        r.need(subject.has())?;
        has(r, mods)
    }
}

/// A part of a list that names its subject once: ", is an artifact in addition to its other types, and has myriad".
fn listed(r: &mut Reader<'_>, subject: Subject) -> Option<Vec<Modification>> {
    r.punct(',').then_some(())?;
    r.word("and");
    let mut mods = Vec::new();
    if subject != Subject::They && r.word("is") {
        is(r, &mut mods)?;
    } else if subject != Subject::They && r.word("isn't") {
        r.need("legendary")?;
        mods.push(Modification::RemoveSupertype(Supertype::Legendary));
    } else {
        r.need(if subject == Subject::They {
            "have"
        } else {
            "has"
        })?;
        has(r, &mut mods)?;
    }
    Some(mods)
}

/// "it isn't legendary", "the token isn't legendary", "the copy isn't legendary": the copy has no supertype Legendary.
fn not_legendary(r: &mut Reader<'_>, subject: Subject) -> Option<Modification> {
    (r.words(subject.isnt()?) || (subject == Subject::It && r.words("the token isn't")))
        .then_some(())?;
    r.need("legendary")?;
    Some(Modification::RemoveSupertype(Supertype::Legendary))
}

/// What follows "it has": keywords, or an ability in quotes.
fn has(r: &mut Reader<'_>, mods: &mut Vec<Modification>) -> Option<()> {
    if let Some(ability) = r
        .quoted(abilities::triggered)
        .or_else(|| r.quoted(abilities::activated))
    {
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
    // "it's a Vehicle artifact with crew 3 and it loses all other card types": the keywords come with the kinds.
    let with = r
        .attempt(|r| {
            r.word("with").then_some(())?;
            granted_keywords(r)
        })
        .unwrap_or_default();
    let types: Vec<CardType> = kinds
        .iter()
        .filter_map(|kind| match kind {
            Modification::AddType(found) => Some(*found),
            _ => None,
        })
        .collect();
    // "it's an artifact and it loses all other card types": these are the types, and no others.
    if size.is_none()
        && !types.is_empty()
        && r.attempt(|r| r.need("and it loses all other card types"))
            .is_some()
    {
        mods.push(Modification::SetTypes(types));
        mods.extend(
            kinds
                .into_iter()
                .filter(|kind| matches!(kind, Modification::AddSubtype(_))),
        );
        mods.extend(with.into_iter().map(Modification::AddKeyword));
        return Some(());
    }
    if !with.is_empty() {
        return None;
    }
    // "it's an artifact": the types, and no others, as the same words with "loses all other card types" mean.
    if size.is_none()
        && types.len() == kinds.len()
        && r.lookahead(|r| r.need("in addition")).is_none()
    {
        mods.push(Modification::SetTypes(types));
        return Some(());
    }
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
    Some(joined(&exception_parts(mods, subject, false)?, subject))
}

/// The words "except" and the parts joined by "and".
fn joined(parts: &[String], subject: Subject) -> String {
    let that = if subject == Subject::Copy {
        "that "
    } else {
        ""
    };
    format!("except {that}{}", parts.join(" and "))
}

/// Every wording of the exceptions: the parts joined by "and", a list that names its subject once when there are
/// three or more ("it isn't legendary, is an artifact in addition to its other types, and has myriad"), and a type
/// alone without "and it loses all other card types" ("it's an artifact and it has ...").
pub(super) fn exceptions_texts(mods: &[Modification], subject: Subject) -> Vec<String> {
    let mut texts: Vec<String> = Vec::new();
    for terse in [false, true] {
        let Some(parts) = exception_parts(mods, subject, terse) else {
            continue;
        };
        for text in [Some(joined(&parts, subject)), listed_text(&parts, subject)]
            .into_iter()
            .flatten()
        {
            if !texts.contains(&text) {
                texts.push(text);
            }
        }
    }
    texts
}

/// The parts of three or more as a list that names its subject once.
fn listed_text(parts: &[String], subject: Subject) -> Option<String> {
    let (first, rest) = parts.split_first().filter(|_| parts.len() > 2)?;
    let heads: Vec<(&str, &str)> = [
        Some((subject.is(), "is")),
        subject.isnt().map(|isnt| (isnt, "isn't")),
        Some((
            subject.has(),
            if subject == Subject::They {
                "have"
            } else {
                "has"
            },
        )),
    ]
    .into_iter()
    .flatten()
    .collect();
    let bare: Vec<String> = rest
        .iter()
        .map(|part| {
            heads.iter().find_map(|(head, short)| {
                part.strip_prefix(head)
                    .filter(|tail| tail.starts_with(' '))
                    .map(|tail| format!("{short}{tail}"))
            })
        })
        .collect::<Option<_>>()?;
    let (last, middle) = bare.split_last()?;
    let that = if subject == Subject::Copy {
        "that "
    } else {
        ""
    };
    let mut text = format!("except {that}{first}");
    for part in middle {
        text.push_str(", ");
        text.push_str(part);
    }
    Some(format!("{text}, and {last}"))
}

/// The parts of the exceptions, one for each change; a type alone is "it's an artifact" with `terse`.
fn exception_parts(mods: &[Modification], subject: Subject, terse: bool) -> Option<Vec<String>> {
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
            // The subtypes of a type that replaces the others, and the keywords that come with them.
            Modification::SetTypes(_) => {
                let subtypes = rest[1..]
                    .iter()
                    .take_while(|m| matches!(m, Modification::AddSubtype(_)))
                    .count();
                let keywords = rest[1 + subtypes..]
                    .iter()
                    .take_while(|m| matches!(m, Modification::AddKeyword(_)))
                    .count();
                1 + subtypes + if subtypes > 0 { keywords } else { 0 }
            }
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
            Modification::RemoveSupertype(Supertype::Legendary) => {
                format!("{} legendary", subject.isnt()?)
            }
            Modification::SetTypes(types) if !types.is_empty() => {
                let types = types
                    .iter()
                    .map(|found| type_name(*found))
                    .collect::<Vec<_>>()
                    .join(" ");
                let subtypes: Vec<&str> = these
                    .iter()
                    .filter_map(|found| match found {
                        Modification::AddSubtype(Subtype(name)) => Some(name.as_str()),
                        _ => None,
                    })
                    .collect();
                let names = subtypes
                    .iter()
                    .copied()
                    .chain([types.as_str()])
                    .collect::<Vec<_>>()
                    .join(" ");
                let keywords: Vec<Modification> = these
                    .iter()
                    .filter(|found| matches!(found, Modification::AddKeyword(_)))
                    .cloned()
                    .collect();
                let with = if keywords.is_empty() {
                    String::new()
                } else {
                    format!(" with {}", granted_text(&keywords)?)
                };
                let loses = if these.len() == 1 && terse {
                    ""
                } else {
                    " and it loses all other card types"
                };
                format!("{} {} {names}{with}{loses}", subject.is(), article(&names))
            }
            Modification::SetColors(colors) => {
                format!("{} {}", subject.is(), colors_text(colors))
            }
            Modification::SetPT { .. } | Modification::AddType(_) | Modification::AddSubtype(_) => {
                kinds_text(subject, these)?
            }
            _ => return None,
        });
    }
    (!parts.is_empty()).then_some(parts)
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
        tapped,
        attacking,
        bind,
    } = effect
    else {
        return None;
    };
    if bind.is_some() != delayed || bind.as_deref().is_some_and(|name| name != COPY) {
        return None;
    }
    // A token enters attacking tapped.
    if *attacking && !*tapped {
        return None;
    }
    // "for each card you've discarded this turn, create a token that's a copy of ~".
    let each = match count {
        Amount::Tallied { .. } => Some(defining::died_text(count, true)?),
        _ => None,
    };
    let (head, plural) = match count {
        Amount::Tallied { .. } | Amount::Fixed(1) if *attacking => (
            "a tapped and attacking token that's a copy of".to_owned(),
            false,
        ),
        Amount::Tallied { .. } if !*tapped => ("a token that's a copy of".to_owned(), false),
        Amount::Fixed(1) if *tapped => ("a tapped token that's a copy of".to_owned(), false),
        Amount::Fixed(1) => ("a token that's a copy of".to_owned(), false),
        _ if *tapped && !*attacking => return None,
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
    let attack = if *attacking && plural {
        " and that are tapped and attacking"
    } else {
        ""
    };
    let each = each
        .map(|each| format!("for each {each}, "))
        .unwrap_or_default();
    Some(format!("{each}create {head} {of}{attack}{tail}"))
}

/// The sentences after the creation of a copy that the effects after it write: what the copy gains ("it gains haste until
/// end of turn") and what is done to it later, and the number of effects they cover.
pub(super) fn after_texts(
    created: &Effect,
    rest: &[Effect],
    several: bool,
) -> Option<(Vec<String>, usize)> {
    let Effect::CreateToken {
        count,
        bind: Some(name),
        ..
    } = created
    else {
        return None;
    };
    (name == COPY).then_some(())?;
    let plural = *count != Amount::one() || several;
    let mut texts = Vec::new();
    let mut used = 0;
    if let Some(Effect::Continuous {
        affected,
        mods,
        duration,
    }) = rest.first()
        && *affected == Ref::Bound(COPY.to_owned())
    {
        let end = match duration {
            Duration::UntilEndOfTurn => " until end of turn",
            Duration::Forever => "",
            _ => return None,
        };
        let verb = if plural { "gain" } else { "gains" };
        texts.push(format!(
            "{} {verb} {}{end}",
            copy_words(plural)[0],
            granted_text(mods)?
        ));
        used += 1;
    }
    if let Some(delayed) = rest.get(used) {
        texts.push(delay_text(created, delayed, plural)?);
        used += 1;
    }
    (used > 0).then_some((texts, used))
}

/// The other ways a card names the copy in the sentences after its creation ("that token gains haste", "exile the
/// tokens"), for each text that names it as "it" or "they".
pub(super) fn pronoun_wordings(texts: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for text in texts {
        if !(text.contains(" copy of ") || text.contains(" copies of ")) {
            continue;
        }
        let mut bases = vec![text.clone()];
        // "For each of those creatures, create ..." is also "For each of them, create ...".
        if let Some(from) = text
            .find("For each of those ")
            .or_else(|| text.find("for each of those "))
            && let Some(len) = text[from..].find(", create")
        {
            let head = &text[..from + "For each of ".len()];
            bases.push(format!("{head}them{}", &text[from + len..]));
        }
        for base in &bases {
            for plural in [false, true] {
                let (subjects, objects) = (copy_words(plural), copy_objects(plural));
                for subject in subjects {
                    for object in objects {
                        let swapped = base
                            .replace(
                                &format!(". {} ", render::capitalize(subjects[0])),
                                &format!(". {} ", render::capitalize(subject)),
                            )
                            .replace(&format!(" {} at ", objects[0]), &format!(" {object} at "));
                        if swapped != *text {
                            found.push(swapped);
                        }
                    }
                }
            }
        }
    }
    found
}

/// "create a tapped and attacking token that's a copy of X" is also "create a token that's a copy of X and that's
/// tapped and attacking" when no exception follows X.
pub(super) fn attacking_wordings(texts: &[String]) -> Vec<String> {
    const HEAD: &str = "reate a tapped and attacking token that's a copy of ";
    let single = texts.iter().filter_map(|text| {
        let from = text.find(HEAD)? + HEAD.len();
        let end = from + text[from..].find(['.', ','])?;
        (text[end..].starts_with('.')).then(|| {
            format!(
                "{}reate a token that's a copy of {} and that's tapped and attacking{}",
                &text[..from - HEAD.len()],
                &text[from..end],
                &text[end..]
            )
        })
    });
    // The card that acts is also "it" in the copies of itself.
    let itself = texts
        .iter()
        .filter(|text| text.contains("copy of ~") || text.contains("copies of ~"))
        .map(|text| {
            text.replace("copy of ~", "copy of it")
                .replace("copies of ~", "copies of it")
        });
    let mut found: Vec<String> = single.chain(itself).collect();
    let entering = entering_wordings(texts)
        .into_iter()
        .chain(entering_wordings(&found))
        .collect::<Vec<_>>();
    found.extend(entering);
    found
}

/// "create a tapped and attacking token that's a copy of X, except ..." is also "create a token that's a copy of X, except
/// .... The token enters tapped and attacking" (the tokens enter, after "for each ...").
fn entering_wordings(texts: &[String]) -> Vec<String> {
    const HEAD: &str = "create a tapped and attacking token that's a copy of ";
    let mut found = Vec::new();
    for text in texts {
        let Some(at) = text.find(HEAD) else {
            continue;
        };
        let from = at + HEAD.len();
        // The sentence ends at the first period that is not inside a quoted ability.
        let mut quoted = false;
        let Some(end) = text[from..]
            .char_indices()
            .find(|(_, c)| {
                quoted ^= *c == '"';
                *c == '.' && !quoted
            })
            .map(|(at, _)| from + at)
        else {
            continue;
        };
        for sentence in [
            "The token enters tapped and attacking",
            "The tokens enter tapped and attacking",
        ] {
            found.push(format!(
                "{}create a token that's a copy of {}. {sentence}{}",
                &text[..at],
                &text[from..end],
                &text[end..]
            ));
        }
    }
    found
}

/// The words that name the copy as what a sentence acts on: one token or several.
const fn copy_objects(plural: bool) -> [&'static str; 3] {
    if plural {
        ["them", "those tokens", "the tokens"]
    } else {
        ["it", "that token", "the token"]
    }
}

/// The sentence that acts on the copy at the next end step: "exile it at the beginning of the next end step".
pub(super) fn delay_text(created: &Effect, delayed: &Effect, plural: bool) -> Option<String> {
    let Effect::CreateToken { .. } = created else {
        return None;
    };
    let Effect::DelayedTrigger {
        when:
            TriggerPattern::StepBegins {
                step,
                whose: Ref::EachPlayer,
            },
        once: true,
        this_turn: false,
        effects,
    } = delayed
    else {
        return None;
    };
    let when = match step {
        Step::End => "at the beginning of the next end step",
        Step::EndCombat => "at end of combat",
        _ => return None,
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
    let pronoun = copy_objects(plural)[0];
    Some(format!("{verb} {pronoun} {when}"))
}

/// "a creature you control" or "any creature on the battlefield": the permanents a permanent may enter as a copy of.
fn choice_text(of: &Selector) -> Option<String> {
    // "any creature card in a graveyard".
    if of.0.contains(&Pred::In(ZoneKind::Graveyard)) {
        let cards = Selector(
            of.0.iter()
                .filter(|pred| **pred != Pred::In(ZoneKind::Graveyard))
                .cloned()
                .collect(),
        );
        return Some(format!(
            "any {} card in a graveyard",
            phrase_text(&cards, Form::Target)?
        ));
    }
    let text = phrase_text(of, Form::Target)?;
    // "a creature you control", "a creature an opponent controls".
    if text.contains(" you control") || text.contains(" an opponent controls") {
        Some(format!("{} {text}", article(&text)))
    } else {
        Some(format!("any {text} on the battlefield"))
    }
}

/// How a permanent that enters as a copy is written in each wording of its exceptions, without its first capital and
/// period.
pub(super) fn enters_texts(
    of: &Selector,
    except: &[Modification],
    tapped: bool,
    until_end_of_turn: bool,
) -> Vec<String> {
    let Some(choice) = choice_text(of) else {
        return Vec::new();
    };
    let head = match (tapped, until_end_of_turn) {
        (false, false) => format!("you may have ~ enter as a copy of {choice}"),
        (true, false) => format!("you may have ~ enter tapped as a copy of {choice}"),
        (false, true) => {
            format!("as ~ enters, you may have it become a copy of {choice} until end of turn")
        }
        (true, true) => return Vec::new(),
    };
    if except.is_empty() {
        return vec![head];
    }
    exceptions_texts(except, Subject::It)
        .into_iter()
        .map(|text| format!("{head}, {text}"))
        .collect()
}
