//! Preventions that name the damage they prevent and a token replacement for one kind of token: "If equipped creature
//! would be dealt damage, prevent that damage and put that many +1/+1 counters on it." (`ReplaceBy::PreventAnd`), the
//! same as a shield of a triggered ability ("If damage would be dealt to that creature this turn, ..."), "If a source
//! would deal damage to another Dinosaur you control, prevent all but 1 of that damage." (`ReplaceBy::PreventAllBut`)
//! and "If one or more creature tokens would be created under your control, that many 4/4 white Angel creature tokens
//! with flying and vigilance are created instead." (`ReplaceWhen::TokenCreation` with `what`). Read and written back.

use super::tokens::Reader;
use super::triggers::{subject, subject_text};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::Duration;
use crate::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::{Amount, CounterKind};
use mtg_core::card::CardType;

/// What "put that many +1/+1 counters on it" does after the prevention: counters on the creature dealt the damage.
fn w9c_counters() -> Vec<Effect> {
    vec![Effect::PutCounters {
        on: Ref::Triggering,
        counter: CounterKind::PlusOnePlusOne,
        count: Amount::Triggering,
    }]
}

/// The words after the comma of a prevention that becomes counters.
const W9C_COUNTERS_TEXT: &str = "prevent that damage and put that many +1/+1 counters on it";

/// "prevent that damage and put that many +1/+1 counters on it"
fn w9c_counters_words(r: &mut Reader<'_>) -> Option<()> {
    r.need("prevent that damage and put that many")?;
    (super::counters::kind(r)? == CounterKind::PlusOnePlusOne).then_some(())?;
    r.need("counters on it")
}

/// The recipients a static prevention names and their words.
fn w9c_recipients() -> [(Ref, &'static str); 3] {
    [
        (Ref::Attached, "equipped creature"),
        (Ref::Attached, "enchanted creature"),
        (Ref::This, "~"),
    ]
}

/// A replacement of the battlefield with no condition.
fn w9c_ability(when: ReplaceWhen, by: ReplaceBy) -> Ability {
    Ability::Replacement {
        when,
        by,
        condition: None,
        from: ZoneKind::Battlefield,
    }
}

/// Damage to the recipient from any source.
fn w9c_damage_to(to: Ref) -> ReplaceWhen {
    ReplaceWhen::Damage {
        to: Some(to),
        source: Selector::default(),
        combat: None,
    }
}

/// Any of the static shapes below.
pub(super) fn w9c_replaced(r: &mut Reader<'_>) -> Option<Ability> {
    r.attempt(w9c_counters_static)
        .or_else(|| r.attempt(w9c_all_but))
        .or_else(|| r.attempt(w9c_token_kind))
}

/// "If equipped creature would be dealt damage, prevent that damage and put that many +1/+1 counters on it."
fn w9c_counters_static(r: &mut Reader<'_>) -> Option<Ability> {
    r.word("if").then_some(())?;
    let to = if r.source() {
        Ref::This
    } else {
        w9c_recipients()
            .into_iter()
            .find_map(|(to, words)| (words != "~" && r.words(words)).then_some(to))?
    };
    r.need("would be dealt damage")?;
    r.punct(',').then_some(())?;
    w9c_counters_words(r)?;
    Some(w9c_ability(
        w9c_damage_to(to),
        ReplaceBy::PreventAnd(w9c_counters()),
    ))
}

/// "If a source would deal damage to another Dinosaur you control, prevent all but 1 of that damage."
fn w9c_all_but(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("if a source would deal damage to")?;
    let to = subject(r)?;
    r.punct(',').then_some(())?;
    r.need("prevent all but")?;
    let kept = match r.number() {
        Some(kept) => kept,
        None => super::amounts::number_word(r.take_word()?)?,
    };
    r.need("of that damage")?;
    Some(w9c_ability(
        w9c_damage_to(Ref::Each(to)),
        ReplaceBy::PreventAllBut(kept),
    ))
}

/// "If one or more creature tokens would be created under your control, that many 4/4 white Angel creature tokens
/// with flying and vigilance are created instead."
fn w9c_token_kind(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("if one or more")?;
    let kind = W9C_KINDS
        .iter()
        .find_map(|(kind, words)| r.word(words).then_some(*kind))?;
    r.need("tokens would be created under your control")?;
    r.punct(',').then_some(())?;
    r.need("that many")?;
    let token = super::create::w9c_plural_token(r)?;
    r.need("are created instead")?;
    Some(w9c_ability(
        ReplaceWhen::TokenCreation {
            controller: Ref::You,
            what: Selector(vec![Pred::Type(kind)]),
        },
        ReplaceBy::Instead(vec![w9c_create(token)]),
    ))
}

/// The kinds of token a replacement names and their words.
const W9C_KINDS: [(CardType, &str); 2] = [
    (CardType::Creature, "creature"),
    (CardType::Artifact, "artifact"),
];

/// The tokens made instead: as many as would have been created.
fn w9c_create(token: crate::effect::TokenRef) -> Effect {
    Effect::CreateToken {
        who: Ref::You,
        token,
        count: Amount::Triggering,
        tapped: false,
        attacking: false,
        bind: None,
    }
}

/// "If damage would be dealt to that creature this turn, prevent that damage and put that many +1/+1 counters on it.":
/// a shield over the one target of the ability for the turn, which "Choose target creature you control." names first
/// when no other sentence does.
pub(super) fn w9c_shield(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    if r.targets.is_empty() {
        r.need("choose")?;
        super::selectors::target(r)?;
        r.punct('.').then_some(())?;
    }
    (r.targets.len() == 1 && matches!(r.targets[0].what, Targetable::Object(_))).then_some(())?;
    r.need("if damage would be dealt to that creature this turn")?;
    r.punct(',').then_some(())?;
    w9c_counters_words(r)?;
    Some(vec![w9c_shield_effect()])
}

fn w9c_shield_effect() -> Effect {
    Effect::Shield {
        when: w9c_damage_to(Ref::Target(0)),
        by: ReplaceBy::PreventAnd(w9c_counters()),
        duration: Duration::UntilEndOfTurn,
    }
}

/// Whether the effect is the shield `w9c_shield` reads.
pub(super) fn w9c_is_shield(effect: &Effect) -> bool {
    *effect == w9c_shield_effect()
}

/// The wording of the shield `w9c_shield` reads, without a capital and a period; "choose <target>." first when the
/// target is not named yet.
pub(super) fn w9c_shield_text(lead: Option<String>) -> String {
    let shield = format!("damage would be dealt to that creature this turn, {W9C_COUNTERS_TEXT}");
    match lead {
        Some(target) => format!("choose {target}. If {shield}"),
        None => format!("if {shield}"),
    }
}

/// What `w9c_replaced` reads, written back; empty for any other ability.
pub(super) fn w9c_sentences(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when,
        by,
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    w9c_texts(when, by).unwrap_or_default()
}

fn w9c_texts(when: &ReplaceWhen, by: &ReplaceBy) -> Option<Vec<String>> {
    Some(match (when, by) {
        (
            ReplaceWhen::Damage {
                to: Some(to),
                source,
                combat: None,
            },
            ReplaceBy::PreventAnd(effects),
        ) if source.is_empty() && *effects == w9c_counters() => w9c_recipients()
            .into_iter()
            .filter(|(named, _)| named == to)
            .map(|(_, words)| format!("if {words} would be dealt damage, {W9C_COUNTERS_TEXT}"))
            .collect(),
        (
            ReplaceWhen::Damage {
                to: Some(Ref::Each(to)),
                source,
                combat: None,
            },
            ReplaceBy::PreventAllBut(kept),
        ) if source.is_empty() => vec![format!(
            "if a source would deal damage to {}, prevent all but {kept} of that damage",
            subject_text(to)?
        )],
        (
            ReplaceWhen::TokenCreation {
                controller: Ref::You,
                what,
            },
            ReplaceBy::Instead(effects),
        ) => {
            let [Pred::Type(kind)] = what.0.as_slice() else {
                return None;
            };
            let (_, words) = W9C_KINDS.iter().find(|(found, _)| found == kind)?;
            let [made @ Effect::CreateToken { .. }] = effects.as_slice() else {
                return None;
            };
            let text = super::create::text(made, false)?;
            let tokens = text.strip_prefix("create that many ")?;
            vec![format!(
                "if one or more {words} tokens would be created under your control, that many {tokens} are created instead"
            )]
        }
        _ => return None,
    })
}
