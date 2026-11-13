//! r40-3 wordings of excess damage (CR 120.10): "Whenever a creature an opponent controls is dealt excess noncombat
//! damage, S.", "Whenever one or more creatures your opponents control are dealt excess noncombat damage, S." (S read and
//! written under the twin head "Whenever you gain life"; "amass Orcs X, where X is that excess damage" as itself), the
//! condition "if a creature or planeswalker an opponent controlled was dealt excess damage this turn" after a trigger's
//! head, and "Whenever ~ deals damage to a creature, if that creature was dealt excess damage this turn, that creature's
//! controller sacrifices a noncreature, nonland permanent."

use mtg_core::card::CardType;

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::{Reader, tokenize};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition, TallyEvent};

/// The head of the twin a body is read and written under.
const TWIN_HEAD: &str = "Whenever you gain life, ";
/// The body that amasses as many Orcs as the excess.
const AMASS_EXCESS: &str = "amass Orcs X, where X is that excess damage.";
/// The condition that an opponent's creature or planeswalker was dealt excess damage this turn.
const OPPONENT_EXCESS: &str =
    ", if a creature or planeswalker an opponent controlled was dealt excess damage this turn";
/// The whole trigger of a creature that punishes a creature dealt excess damage.
const PUNISHES: &str = "Whenever ~ deals damage to a creature, if that creature was dealt excess damage this turn, that \
                        creature's controller sacrifices a noncreature, nonland permanent.";

/// The amass of as many Orcs as the excess.
fn amass_excess() -> Effect {
    Effect::Amass {
        subtype: mtg_core::card::Subtype("Orc".to_owned()),
        count: Amount::Triggering,
        bind: None,
    }
}

/// The condition of `OPPONENT_EXCESS`.
fn opponent_excess() -> Condition {
    Condition::Compare {
        left: Amount::Tally {
            event: TallyEvent::DealtExcess,
            who: Ref::Opponents,
            filter: Selector(vec![Pred::AnyOf(vec![
                Pred::Type(CardType::Creature),
                Pred::Type(CardType::Planeswalker),
            ])]),
        },
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    }
}

/// The trigger of `PUNISHES`.
fn punishes() -> Ability {
    Ability::Triggered {
        when: TriggerPattern::DealsDamage {
            source: Selector::this(),
            to: Some(Ref::Each(Selector(vec![Pred::Type(CardType::Creature)]))),
            combat: None,
        },
        intervening_if: Some(Condition::Matches {
            subject: Ref::Triggering,
            selector: Selector(vec![Pred::DidThisTurn {
                event: TallyEvent::DealtExcess,
                by: None,
                to: None,
                combat: false,
            }]),
        }),
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![Effect::Sacrifice {
            who: Ref::ControllerOf(Box::new(Ref::Triggering)),
            filter: Selector(vec![
                Pred::Not(Box::new(Pred::Type(CardType::Creature))),
                Pred::Not(Box::new(Pred::Type(CardType::Land))),
            ]),
            count: Amount::Fixed(1),
            bind: None,
        }],
    }
}

/// The damage words after "excess": "noncombat damage", "combat damage" or "damage".
fn damage_words(combat: Option<bool>) -> &'static str {
    match combat {
        Some(false) => "noncombat damage",
        Some(true) => "combat damage",
        None => "damage",
    }
}

/// The head of a trigger on excess damage, without its comma.
fn head_text(when: &TriggerPattern) -> Option<String> {
    let (batch, what, combat) = match when {
        TriggerPattern::DealtExcess { what, combat } => (false, what, *combat),
        TriggerPattern::Batch(inner) => match &**inner {
            TriggerPattern::DealtExcess { what, combat } => (true, what, *combat),
            _ => return None,
        },
        _ => return None,
    };
    let words = damage_words(combat);
    Some(if batch {
        format!(
            "Whenever one or more {} are dealt excess {words}",
            phrase_text(what, Form::All)?
        )
    } else {
        let noun = phrase_text(what, Form::Subject)?;
        let article = if noun.starts_with(['a', 'e', 'i', 'o', 'u']) {
            "an"
        } else {
            "a"
        };
        format!("Whenever {article} {noun} is dealt excess {words}")
    })
}

/// The head `head_text` writes, read: the pattern and the rest of the text after its comma.
fn head_read(text: &str) -> Option<(TriggerPattern, &str)> {
    let (batch, rest) = if let Some(rest) = text.strip_prefix("Whenever one or more ") {
        (true, rest)
    } else {
        let rest = text.strip_prefix("Whenever ")?;
        (
            false,
            rest.strip_prefix("a ")
                .or_else(|| rest.strip_prefix("an "))?,
        )
    };
    let verb = if batch {
        " are dealt excess "
    } else {
        " is dealt excess "
    };
    let (noun, rest) = rest.split_once(verb)?;
    let (combat, body) = [Some(false), Some(true), None]
        .into_iter()
        .find_map(|combat| {
            Some((
                combat,
                rest.strip_prefix(&format!("{}, ", damage_words(combat)))?,
            ))
        })?;
    let tokens = tokenize(noun);
    let mut r = Reader::new(&tokens, false);
    let what = phrase(&mut r, if batch { Form::All } else { Form::Subject })?;
    r.end().then_some(())?;
    let pattern = TriggerPattern::DealtExcess { what, combat };
    Some((
        if batch {
            TriggerPattern::Batch(Box::new(pattern))
        } else {
            pattern
        },
        body,
    ))
}

/// A trigger on excess damage, or one under the condition of `OPPONENT_EXCESS`, read from its twin.
pub(super) fn r403_excess_read(
    text: &str,
    read: &impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    if text == PUNISHES {
        return Some(punishes());
    }
    if let Some((head, rest)) = text.split_once(OPPONENT_EXCESS) {
        let mut ability = read(&format!("{head}{rest}"))?;
        let Ability::Triggered { intervening_if, .. } = &mut ability else {
            return None;
        };
        intervening_if.is_none().then_some(())?;
        *intervening_if = Some(opponent_excess());
        return Some(ability);
    }
    let (when, body) = head_read(text)?;
    if body == AMASS_EXCESS {
        return Some(Ability::Triggered {
            when,
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![amass_excess()],
        });
    }
    let mut ability = read(&format!("{TWIN_HEAD}{body}"))?;
    let Ability::Triggered {
        when: read_when, ..
    } = &mut ability
    else {
        return None;
    };
    *read_when = when;
    Some(ability)
}

/// The wordings of a trigger on excess damage or under the condition of `OPPONENT_EXCESS`.
pub(super) fn r403_excess_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    more: &mut Vec<String>,
) {
    if *ability == punishes() {
        more.push(PUNISHES.to_owned());
        return;
    }
    let Ability::Triggered {
        when,
        intervening_if,
        effects,
        ..
    } = ability
    else {
        return;
    };
    if intervening_if.as_ref() == Some(&opponent_excess()) {
        let mut twin = ability.clone();
        if let Ability::Triggered { intervening_if, .. } = &mut twin {
            *intervening_if = None;
        }
        for text in written(&twin) {
            if let Some((head, body)) = text.split_once(", ") {
                more.push(format!("{head}{OPPONENT_EXCESS}, {body}"));
            }
        }
        return;
    }
    let Some(head) = head_text(when) else {
        return;
    };
    if intervening_if.is_none() && effects.as_slice() == [amass_excess()] {
        more.push(format!("{head}, {AMASS_EXCESS}"));
        return;
    }
    let mut twin = ability.clone();
    if let Ability::Triggered { when, .. } = &mut twin {
        *when = TriggerPattern::LifeGained { who: Ref::You };
    }
    for text in written(&twin) {
        if let Some(body) = text.strip_prefix(TWIN_HEAD) {
            more.push(format!("{head}, {body}"));
        }
    }
}
