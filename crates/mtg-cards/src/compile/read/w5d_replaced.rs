//! Replacement effects whose steps name the event they replace (`ReplaceBy::Instead` with `Triggering`, `Dealer` and
//! `Amount::Triggering`): "If an opponent would gain life, that player loses that much life instead.", "All damage that
//! would be dealt to you is dealt to enchanted creature instead.", "If a creature an opponent controls would die,
//! instead exile it and ...", "If ~ would deal combat damage to a player, instead put that many +1/+1 counters on ~ and
//! that player mills that many cards."

use super::effects::IMPERATIVES;
use super::tokens::Reader;
use super::triggers::{subject, subject_text};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::{Amount, CounterKind, StatKind};

/// A replacement of the battlefield with no condition.
fn w5d_ability(when: ReplaceWhen, effects: Vec<Effect>) -> Ability {
    Ability::Replacement {
        when,
        by: ReplaceBy::Instead(effects),
        condition: None,
        from: ZoneKind::Battlefield,
    }
}

/// Any of the shapes below.
pub(super) fn w5d_replaced(r: &mut Reader<'_>) -> Option<Ability> {
    r.attempt(w5d_life)
        .or_else(|| r.attempt(w5d_redirect))
        .or_else(|| r.attempt(w5d_dies))
        .or_else(|| r.attempt(w5d_combat_damage))
}

/// The player who would gain life, as "if ... would gain life" names them.
const W5D_GAINERS: [(&str, Ref); 2] = [
    ("an opponent", Ref::Opponents),
    ("a player", Ref::EachPlayer),
];

/// "If an opponent would gain life, that player loses that much life instead."
fn w5d_life(r: &mut Reader<'_>) -> Option<Ability> {
    r.word("if").then_some(())?;
    let who = W5D_GAINERS
        .iter()
        .find_map(|(text, who)| r.words(text).then(|| who.clone()))?;
    r.need("would gain life")?;
    r.punct(',').then_some(())?;
    r.need("that player loses that much life instead")?;
    Some(w5d_ability(
        ReplaceWhen::LifeGain { who },
        vec![Effect::LoseLife {
            who: Ref::Triggering,
            amount: Amount::Triggering,
        }],
    ))
}

/// "All damage that would be dealt to you is dealt to enchanted creature instead." (also "to ~").
fn w5d_redirect(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("all damage that would be dealt to you is dealt to")?;
    let to = if r.source() {
        Ref::This
    } else {
        r.need("enchanted creature")?;
        Ref::Attached
    };
    r.word("instead").then_some(())?;
    Some(w5d_ability(
        ReplaceWhen::Damage {
            to: Some(Ref::You),
            source: Selector::default(),
            combat: None,
        },
        vec![Effect::DealDamage {
            source: Some(Ref::Dealer),
            to,
            amount: Amount::Triggering,
        }],
    ))
}

/// "If a creature an opponent controls would die, instead exile it and put ... on this creature." (also "that card"),
/// one more step after the exile.
fn w5d_dies(r: &mut Reader<'_>) -> Option<Ability> {
    r.word("if").then_some(())?;
    let what = subject(r)?;
    r.need("would die")?;
    r.punct(',').then_some(())?;
    r.need("instead exile")?;
    (r.word("it") || r.words("that card") || r.words("that creature")).then_some(())?;
    r.word("and").then_some(())?;
    // The step after the exile reads "that creature" as the one that would have died.
    r.it = Some(Ref::Triggering);
    let step = r.attempt(w5d_power_counters).or_else(|| {
        IMPERATIVES.iter().find_map(|template| {
            r.attempt(|r| match template(r)?.as_slice() {
                [step] => Some(step.clone()),
                _ => None,
            })
        })
    })?;
    Some(w5d_ability(
        ReplaceWhen::ZoneChange {
            what,
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard),
        },
        vec![
            Effect::Exile {
                what: Ref::Triggering,
                bind: None,
                duration: None,
            },
            step,
        ],
    ))
}

/// The counters of the creature's power put on the card.
fn w5d_power_step() -> Effect {
    Effect::PutCounters {
        on: Ref::This,
        counter: CounterKind::PlusOnePlusOne,
        count: Amount::Stat {
            of: Ref::Triggering,
            stat: StatKind::Power,
        },
    }
}

/// The wording of `w5d_power_step` after the exile.
const W5D_POWER_TEXT: &str = "put a number of +1/+1 counters equal to that creature's power on ~";

/// "put a number of +1/+1 counters equal to that creature's power on this creature"
fn w5d_power_counters(r: &mut Reader<'_>) -> Option<Effect> {
    r.need("put a number of")?;
    (super::counters::kind(r)? == CounterKind::PlusOnePlusOne).then_some(())?;
    r.need("counters equal to that creature's power on")?;
    r.source().then(w5d_power_step)
}

/// "If ~ would deal combat damage to a player, instead put that many +1/+1 counters on ~ and that player mills that
/// many cards."
fn w5d_combat_damage(r: &mut Reader<'_>) -> Option<Ability> {
    r.word("if").then_some(())?;
    r.source().then_some(())?;
    r.need("would deal combat damage to a player")?;
    r.punct(',').then_some(())?;
    r.need("instead put that many")?;
    (super::counters::kind(r)? == CounterKind::PlusOnePlusOne).then_some(())?;
    r.need("counters on")?;
    r.source().then_some(())?;
    r.need("and that player mills that many cards")?;
    Some(w5d_ability(
        ReplaceWhen::Damage {
            to: Some(Ref::EachPlayer),
            source: Selector::this(),
            combat: Some(true),
        },
        vec![
            Effect::PutCounters {
                on: Ref::This,
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Triggering,
            },
            Effect::Mill {
                who: Ref::Triggering,
                count: Amount::Triggering,
            },
        ],
    ))
}

/// A step written alone, as the sentence of a spell writes it, without its capital and period.
fn w5d_step_texts(effect: &Effect) -> Vec<String> {
    if *effect == w5d_power_step() {
        return vec![W5D_POWER_TEXT.to_owned()];
    }
    super::render::variants(&Ability::Spell {
        targets: Vec::new(),
        effects: vec![effect.clone()],
    })
    .into_iter()
    .map(|text| {
        let text = text.strip_suffix('.').unwrap_or(&text);
        let mut chars = text.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_lowercase().chain(chars).collect()
        })
    })
    .collect()
}

/// What `w5d_replaced` reads, written back; empty for any other ability.
pub(super) fn w5d_sentences(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when,
        by: ReplaceBy::Instead(effects),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    w5d_texts(when, effects).unwrap_or_default()
}

fn w5d_texts(when: &ReplaceWhen, effects: &[Effect]) -> Option<Vec<String>> {
    Some(match (when, effects) {
        (
            ReplaceWhen::LifeGain { who },
            [Effect::LoseLife {
                who: Ref::Triggering,
                amount: Amount::Triggering,
            }],
        ) => {
            let (text, _) = W5D_GAINERS.iter().find(|(_, gainer)| gainer == who)?;
            vec![format!("if {text} would gain life, that player loses that much life instead")]
        }
        (
            ReplaceWhen::Damage {
                to: Some(Ref::You),
                source,
                combat: None,
            },
            [Effect::DealDamage {
                source: Some(Ref::Dealer),
                to,
                amount: Amount::Triggering,
            }],
        ) if source.is_empty() => {
            let to = match to {
                Ref::This => "~",
                Ref::Attached => "enchanted creature",
                _ => return None,
            };
            vec![format!("all damage that would be dealt to you is dealt to {to} instead")]
        }
        (
            ReplaceWhen::ZoneChange {
                what,
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
            },
            [Effect::Exile {
                what: Ref::Triggering,
                bind: None,
                duration: None,
            }, step],
        ) => {
            let subject = subject_text(what)?;
            let mut texts = Vec::new();
            for it in ["it", "that card"] {
                for step in w5d_step_texts(step) {
                    texts.push(format!("if {subject} would die, instead exile {it} and {step}"));
                }
            }
            texts
        }
        (
            ReplaceWhen::Damage {
                to: Some(Ref::EachPlayer),
                source,
                combat: Some(true),
            },
            [Effect::PutCounters {
                on: Ref::This,
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Triggering,
            }, Effect::Mill {
                who: Ref::Triggering,
                count: Amount::Triggering,
            }],
        ) if *source == Selector::this() => vec![
            "if ~ would deal combat damage to a player, instead put that many +1/+1 counters on ~ and that player mills that many cards"
                .to_owned(),
        ],
        _ => return None,
    })
}
