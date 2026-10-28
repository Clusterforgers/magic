//! Prevention: damage that can't be prevented, as a static ability ("Combat damage that would be dealt by creatures you
//! control can't be prevented.") and as an effect ("Damage can't be prevented this turn."), and the shield that
//! prevents all damage, or all combat damage, that would be dealt this turn. Read from the wording and written back.

use super::selectors::{Form, phrase, phrase_text, target};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
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

/// The damage a permanent prevents for itself or its controller: "Prevent all combat damage that would be dealt to this
/// creature.", "Prevent all damage that would be dealt to you.", and "If damage would be dealt to this creature, prevent
/// that damage. Remove a +1/+1 counter from this creature."
pub fn static_prevent(r: &mut Reader<'_>) -> Option<Ability> {
    if let Some(group) = r.attempt(super::c11h_prevent::c11h_read) {
        return Some(group);
    }
    let (to, combat, by) = if r.words("prevent all") {
        let combat = kind(r)?.flag();
        r.need("that would be dealt to")?;
        let to = if r.source() {
            Ref::This
        } else {
            r.word("you").then_some(Ref::You)?
        };
        (to, combat, ReplaceBy::Prevent)
    } else {
        r.need("if damage would be dealt to")?;
        r.source().then_some(())?;
        r.punct(',').then_some(())?;
        r.need("prevent that damage")?;
        r.punct('.').then_some(())?;
        r.need("remove a")?;
        let counter = super::counters::kind(r)?;
        r.need("counter from")?;
        r.source().then_some(())?;
        (Ref::This, None, ReplaceBy::PreventRemovingCounter(counter))
    };
    Some(Ability::Replacement {
        when: ReplaceWhen::Damage {
            to: Some(to),
            source: Selector::default(),
            combat,
        },
        by,
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
/// "Prevent all damage that would be dealt this turn by creatures your opponents control.", and "Prevent all damage
/// target instant or sorcery spell would deal this turn." (the source is the target).
pub fn shield(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("prevent all")?;
    let combat = kind(r)?.flag();
    let named = r.attempt(|r| {
        let named = target(r)?;
        r.need("would deal this turn")?;
        Some(named)
    });
    if named.is_none() {
        r.need("that would be dealt this turn")?;
    }
    let source = if let Some(named) = named {
        Selector(vec![Pred::Is(named)])
    } else if r.word("by") {
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
        Effect::Shield {
            when:
                ReplaceWhen::Damage {
                    to: Some(Ref::You),
                    source,
                    combat,
                },
            by: ReplaceBy::Prevent,
            duration: Duration::UntilEndOfTurn,
        } if source.0.is_empty() => Some(format!(
            "prevent all {} that would be dealt to you this turn",
            damage_text(*combat)
        )),
        _ => None,
    }
}

/// The source a shield of all damage this turn names as one object ("prevent all damage target instant or sorcery
/// spell would deal this turn"), for the writer to word.
pub fn c9_fight_named_source(effect: &Effect) -> Option<&Ref> {
    match effect {
        Effect::Shield {
            when:
                ReplaceWhen::Damage {
                    to: None,
                    source,
                    combat: None,
                },
            by: ReplaceBy::Prevent,
            duration: Duration::UntilEndOfTurn,
        } => match source.0.as_slice() {
            [Pred::Is(named)] => Some(named),
            _ => None,
        },
        _ => None,
    }
}
