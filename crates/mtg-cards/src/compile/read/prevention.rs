//! Prevention: damage that can't be prevented, as a static ability ("Combat damage that would be dealt by creatures you
//! control can't be prevented.") and as an effect ("Damage can't be prevented this turn."), and the shield that
//! prevents all damage, or all combat damage, that would be dealt this turn. Read from the wording and written back.

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule};
use crate::refs::{Pred, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// "Damage can't be prevented.", "Combat damage can't be prevented.", "Damage that would be dealt by this creature
/// can't be prevented.", "Combat damage that would be dealt by creatures you control can't be prevented."
pub fn static_rule(r: &mut Reader<'_>) -> Option<Ability> {
    let (source, combat) = cant_be_prevented(r)?;
    Some(Ability::Rule {
        rule: GameRule::DamageCantBePrevented { source, combat },
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// "Damage can't be prevented this turn.", "Combat damage can't be prevented this turn."
pub fn rule_effect(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let (source, combat) = cant_be_prevented(r)?;
    // A source is for the static ability; the effect lasts for every source.
    source.0.is_empty().then_some(())?;
    r.need("this turn")?;
    Some(vec![Effect::Rule {
        rule: GameRule::DamageCantBePrevented { source, combat },
        duration: Duration::UntilEndOfTurn,
    }])
}

/// The sources and the kind of damage of a sentence up to "can't be prevented".
fn cant_be_prevented(r: &mut Reader<'_>) -> Option<(Selector, Option<bool>)> {
    let combat = kind(r)?.flag();
    let source = if r.words("that would be dealt by") {
        if r.source() {
            Selector::this()
        } else {
            let source = phrase(r, Form::All)?;
            (!source.0.is_empty()).then_some(())?;
            source
        }
    } else {
        Selector::default()
    };
    r.need("can't be prevented")?;
    Some((source, combat))
}

/// "damage", "combat damage" or "noncombat damage": whether it is combat damage, or either.
fn kind(r: &mut Reader<'_>) -> Option<Combat> {
    let combat = if r.word("combat") {
        Combat::Only
    } else if r.word("noncombat") {
        Combat::Not
    } else {
        Combat::Either
    };
    r.word("damage").then_some(combat)
}

/// Which damage a sentence is about.
#[derive(Clone, Copy)]
enum Combat {
    /// Combat damage only.
    Only,
    /// Damage that is not combat damage.
    Not,
    /// Both.
    Either,
}

impl Combat {
    /// The flag of the data: `Some(true)` for combat damage only, `Some(false)` for the rest, `None` for both.
    fn flag(self) -> Option<bool> {
        match self {
            Combat::Only => Some(true),
            Combat::Not => Some(false),
            Combat::Either => None,
        }
    }
}

/// "Prevent all damage that would be dealt this turn.", "Prevent all combat damage that would be dealt this turn.",
/// "Prevent all damage that would be dealt this turn by creatures your opponents control."
pub fn shield(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("prevent all")?;
    let combat = kind(r)?.flag();
    r.need("that would be dealt this turn")?;
    let source = if r.word("by") {
        phrase(r, Form::All)?
    } else {
        Selector::default()
    };
    Some(vec![Effect::Shield {
        when: ReplaceWhen::Damage {
            to: None,
            source,
            combat,
        },
        by: ReplaceBy::Prevent,
        duration: Duration::UntilEndOfTurn,
    }])
}

/// "damage", "combat damage", "noncombat damage".
fn damage_text(combat: Option<bool>) -> &'static str {
    match combat {
        None => "damage",
        Some(true) => "combat damage",
        Some(false) => "noncombat damage",
    }
}

/// The sources of the damage in words: "", " that would be dealt by ~", " that would be dealt by creatures you control".
fn sources_text(source: &Selector) -> Option<String> {
    if source.0.is_empty() {
        return Some(String::new());
    }
    if source.0 == [Pred::This] {
        return Some(" that would be dealt by ~".to_owned());
    }
    let text = phrase_text(source, Form::All)?;
    Some(format!(" that would be dealt by {text}"))
}

/// The wordings of the static ability, without a period.
pub fn static_texts(source: &Selector, combat: Option<bool>) -> Vec<String> {
    sources_text(source)
        .map(|sources| format!("{}{sources} can't be prevented", damage_text(combat)))
        .into_iter()
        .collect()
}

/// The wording of an effect this module reads, without a capital and a period.
pub fn effect_text(effect: &Effect) -> Option<String> {
    match effect {
        Effect::Rule {
            rule: GameRule::DamageCantBePrevented { source, combat },
            duration: Duration::UntilEndOfTurn,
        } if source.0.is_empty() => Some(format!(
            "{} can't be prevented this turn",
            damage_text(*combat)
        )),
        Effect::Shield {
            when:
                ReplaceWhen::Damage {
                    to: None,
                    source,
                    combat,
                },
            by: ReplaceBy::Prevent,
            duration: Duration::UntilEndOfTurn,
        } => {
            let by = if source.0.is_empty() {
                String::new()
            } else {
                format!(" by {}", phrase_text(source, Form::All)?)
            };
            Some(format!(
                "prevent all {} that would be dealt this turn{by}",
                damage_text(*combat)
            ))
        }
        _ => None,
    }
}
