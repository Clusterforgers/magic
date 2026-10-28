//! The cost of attacking: "Creatures can't attack you unless their controller pays {2} for each creature they control
//! that's attacking you.", "Creatures can't attack you or planeswalkers you control unless their controller pays {1}
//! for each of those creatures." (a static ability) and the same sentence after "Until your next turn," (an effect), with
//! "{X} ..., where X is the number of enchantments you control" for a count and "pays 2 life" for life. Read into
//! `GameRule::AttackTax` and written back.

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::mana::ManaCostX;
use crate::modify::{Attacked, Duration, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;
use mtg_core::card::CardType;

/// The static ability: the sentence of `tax`.
pub fn static_rule(r: &mut Reader<'_>) -> Option<Ability> {
    Some(Ability::Rule {
        rule: tax(r)?,
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// "Until your next turn, creatures can't attack you or planeswalkers you control unless their controller pays {2} for
/// each of those creatures."
pub fn rule_effect(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("until your next turn")?;
    r.punct(',').then_some(())?;
    let rule = tax(r)?;
    r.sentence_end().then_some(())?;
    Some(vec![Effect::Rule {
        rule,
        duration: Duration::UntilYourNextTurn,
    }])
}

/// "creatures can't attack you unless their controller pays {2} for each creature they control that's attacking you".
fn tax(r: &mut Reader<'_>) -> Option<GameRule> {
    let attackers = attackers(r)?;
    r.need("can't attack")?;
    let attacked = whom(r)?;
    r.need("unless their controller pays")?;
    // "pays 2 life for each of those creatures"
    if let Some(life) = r.attempt(|r| r.number().filter(|_| r.word("life"))) {
        r.need("for each")?;
        each(r, attacked)?;
        return (life > 0).then_some(GameRule::AttackTax {
            who: Ref::You,
            attacked,
            attackers,
            mana: ManaCostX::default(),
            life,
            per: Amount::one(),
        });
    }
    let mut symbols = String::new();
    while let Some(symbol) = r.symbol() {
        symbols.push('{');
        symbols.push_str(symbol);
        symbols.push('}');
    }
    r.need("for each")?;
    each(r, attacked)?;
    let (mana, per) = if symbols == "{X}" {
        r.punct(',').then_some(())?;
        r.need("where x is the number of")?;
        let counted = phrase(r, Form::All)?;
        ("{1}".parse::<ManaCostX>().ok()?, Amount::Count(counted))
    } else {
        (symbols.parse::<ManaCostX>().ok()?, Amount::one())
    };
    (!symbols.is_empty() && !mana.has_x()).then_some(())?;
    Some(GameRule::AttackTax {
        who: Ref::You,
        attacked,
        attackers,
        mana,
        life: 0,
        per,
    })
}

/// "creatures", "nonblack creatures": the creatures the tax is for, none named when every creature is.
fn attackers(r: &mut Reader<'_>) -> Option<Selector> {
    let selector = phrase(r, Form::All)?;
    selector
        .0
        .contains(&Pred::Type(CardType::Creature))
        .then_some(())?;
    Some(if selector.0 == [Pred::Type(CardType::Creature)] {
        Selector::default()
    } else {
        selector
    })
}

/// "you", "you or planeswalkers you control", "planeswalkers you control": what is protected.
fn whom(r: &mut Reader<'_>) -> Option<Attacked> {
    if r.words("you or planeswalkers you control") {
        Some(Attacked::Both)
    } else if r.words("planeswalkers you control") {
        Some(Attacked::Planeswalkers)
    } else if r.word("you") {
        Some(Attacked::Player)
    } else {
        None
    }
}

/// What "for each" counts, which says again who is attacked: "of those creatures", "creature they control that's
/// attacking you", "creature they control that's attacking a planeswalker you control".
fn each(r: &mut Reader<'_>, attacked: Attacked) -> Option<()> {
    if r.words("of those creatures") {
        return Some(());
    }
    r.need("creature they control that's attacking")?;
    match attacked {
        Attacked::Player => r.need("you"),
        Attacked::Planeswalkers => r.need("a planeswalker you control"),
        Attacked::Both => None,
    }
}

/// The wording of the creatures and what they attack, in each way a card says it.
fn sentences(rule: &GameRule) -> Vec<String> {
    let GameRule::AttackTax {
        who: Ref::You,
        attacked,
        attackers,
        mana,
        life,
        per,
    } = rule
    else {
        return Vec::new();
    };
    let no_mana = mana.rest().symbols().is_empty() && !mana.has_x();
    if (*life > 0) == !no_mana {
        return Vec::new();
    }
    let Some(creatures) = (if attackers.is_empty() {
        Some("creatures".to_owned())
    } else {
        phrase_text(attackers, Form::All)
    }) else {
        return Vec::new();
    };
    let (price, tail) = match per {
        Amount::Fixed(1) if *life > 0 => (format!("{life} life"), String::new()),
        Amount::Fixed(1) if !mana.has_x() => (mana.to_string(), String::new()),
        Amount::Count(counted) if mana.to_string() == "{1}" => {
            match phrase_text(counted, Form::All) {
                Some(counted) => (
                    "{X}".to_owned(),
                    format!(", where X is the number of {counted}"),
                ),
                None => return Vec::new(),
            }
        }
        _ => return Vec::new(),
    };
    let (whom, each): (&str, Vec<&str>) = match attacked {
        Attacked::Player => (
            "you",
            vec![
                "creature they control that's attacking you",
                "of those creatures",
            ],
        ),
        Attacked::Both => (
            "you or planeswalkers you control",
            vec!["of those creatures"],
        ),
        Attacked::Planeswalkers => (
            "planeswalkers you control",
            vec![
                "creature they control that's attacking a planeswalker you control",
                "of those creatures",
            ],
        ),
    };
    each.into_iter()
        .map(|each| {
            format!(
                "{creatures} can't attack {whom} unless their controller pays {price} for each {each}{tail}"
            )
        })
        .collect()
}

/// What `static_rule` reads, written back.
pub fn static_texts(rule: &GameRule) -> Vec<String> {
    sentences(rule)
}

/// What `rule_effect` reads, written back, without a capital and a period.
pub fn effect_text(effect: &Effect) -> Option<String> {
    let Effect::Rule {
        rule,
        duration: Duration::UntilYourNextTurn,
    } = effect
    else {
        return None;
    };
    sentences(rule)
        .into_iter()
        .next()
        .map(|sentence| format!("until your next turn, {sentence}"))
}
