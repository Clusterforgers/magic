//! Statics: the abilities of a permanent that need no event. A permanent enters tapped (also
//! unless something holds), with counters, after a choice or after a payment of life; a spell
//! can't be countered; a creature can't block, or attacks each combat; the top card of a library
//! is played revealed; a creature, the creature an Aura or Equipment is attached to, or a group of
//! creatures gets a bonus or a keyword. Read from the wording and written back the same way.

use mtg_core::card::Subtype;

use super::amounts::{count_word, number_word};
use super::counters;
use super::effects::{granted, power_toughness};
use super::render::{capitalize, is_a_type, signed_text};
use super::selectors::{Form, all, keyword_text, phrase, phrase_text, subtype};
use super::tokens::Reader;
use super::triggers::article;
use crate::ability::Ability;
use crate::modify::{GameRule, Modification};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{EntersChoice, ReplaceBy, ReplaceWhen};
use crate::value::{Amount, Cmp, Condition};

type Template = fn(&mut Reader<'_>) -> Option<Ability>;

/// The static shapes, tried in this order.
const TEMPLATES: [Template; 5] = [entering, as_entering, rule, cannot, modifies];

/// A static ability that is a whole sentence: the shape and its period.
pub fn ability(r: &mut Reader<'_>) -> Option<Ability> {
    TEMPLATES.iter().find_map(|template| {
        r.attempt(|r| {
            let found = template(r)?;
            r.sentence_end().then_some(found)
        })
    })
}

/// The replacement of the card's own entering.
fn replacement(by: ReplaceBy, condition: Option<Condition>) -> Ability {
    Ability::Replacement {
        when: ReplaceWhen::Enters {
            what: Selector::this(),
        },
        by,
        condition,
        from: ZoneKind::Battlefield,
    }
}

/// "~ enters tapped.", "~ enters tapped unless you control two or more basic lands.",
/// "~ enters with two +1/+1 counters on it."
fn entering(r: &mut Reader<'_>) -> Option<Ability> {
    r.source().then_some(())?;
    r.need("enters")?;
    if r.word("tapped") {
        let condition = if r.word("unless") {
            Some(Condition::Not(Box::new(control(r)?)))
        } else {
            None
        };
        return Some(replacement(ReplaceBy::Tapped, condition));
    }
    r.need("with")?;
    let (counter, count) = counters::entering(r)?;
    r.need("on it")?;
    Some(replacement(
        ReplaceBy::WithCounters { counter, count },
        None,
    ))
}

/// "As ~ enters, choose a color.", "As ~ enters, you may pay 2 life. If you don't, it enters tapped."
fn as_entering(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("as")?;
    r.source().then_some(())?;
    r.need("enters")?;
    r.punct(',').then_some(())?;
    if r.words("choose a color") {
        return Some(replacement(ReplaceBy::Choose(EntersChoice::Color), None));
    }
    if r.words("choose a creature type") {
        return Some(replacement(
            ReplaceBy::Choose(EntersChoice::CreatureType),
            None,
        ));
    }
    r.need("you may pay")?;
    let life = r.number().filter(|life| *life > 0)?;
    r.need("life")?;
    r.punct('.').then_some(())?;
    r.need("if you don't")?;
    r.punct(',').then_some(())?;
    r.need("it enters tapped")?;
    Some(replacement(ReplaceBy::PayLifeOrTapped(life), None))
}

/// What a permanent must control for it not to enter tapped: "you control a Forest or a Plains",
/// "you control two or fewer other lands", "your opponents control eight or more lands".
fn control(r: &mut Reader<'_>) -> Option<Condition> {
    let who = if r.words("you control") {
        Ref::You
    } else if r.words("your opponents control") {
        Ref::Opponents
    } else {
        return None;
    };
    let (op, number, selector) = if r.word("a") || r.word("an") {
        (Cmp::Ge, 1, one_kind(r)?)
    } else {
        let number = number_word(r.take_word()?)?;
        let op = if r.words("or more") {
            Cmp::Ge
        } else if r.words("or fewer") {
            Cmp::Le
        } else {
            return None;
        };
        (op, number, phrase(r, Form::All)?)
    };
    // Who controls it is the verb's, not a suffix of the phrase.
    let mut preds = selector.0;
    if preds
        .iter()
        .any(|pred| matches!(pred, Pred::ControlledBy(_)))
    {
        return None;
    }
    preds.push(Pred::ControlledBy(who));
    Some(Condition::Compare {
        left: Amount::Count(Selector(preds)),
        op,
        right: Amount::Fixed(i32::try_from(number).ok()?),
    })
}

/// One thing after "a": "planeswalker", "basic land", "Forest or a Plains", "Mount or Vehicle".
fn one_kind(r: &mut Reader<'_>) -> Option<Selector> {
    let first = phrase(r, Form::Subject)?;
    let [Pred::Subtype(_)] = first.0.as_slice() else {
        return Some(first);
    };
    let mut kinds = first.0;
    while r.word("or") {
        let _ = r.word("a") || r.word("an");
        kinds.push(Pred::Subtype(Subtype(subtype(r)?)));
    }
    Some(Selector(if kinds.len() > 1 {
        vec![Pred::AnyOf(kinds)]
    } else {
        kinds
    }))
}

/// "~ can't be countered.", "~ attacks each combat if able.", "All creatures attack each combat if able.",
/// "Play with the top card of your library revealed."
fn rule(r: &mut Reader<'_>) -> Option<Ability> {
    let rule = if r.words("play with the top card of your library revealed") {
        GameRule::RevealTop(Ref::You)
    } else if r.source() {
        if r.words("can't be countered") {
            return Some(Ability::Rule {
                rule: GameRule::CantBeCountered(Ref::This),
                condition: None,
                from: ZoneKind::Stack,
            });
        } else if r.words("attacks each combat if able") {
            GameRule::MustAttack(Ref::This)
        } else if r.words("blocks each combat if able") {
            GameRule::MustBlock(Ref::This)
        } else {
            return None;
        }
    } else {
        let who = plural(r)?;
        if r.words("attack each combat if able") {
            GameRule::MustAttack(who)
        } else if r.words("block each combat if able") {
            GameRule::MustBlock(who)
        } else {
            return None;
        }
    };
    Some(Ability::Rule {
        rule,
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// A group of permanents: "all creatures", "creatures you control", "other creatures you control",
/// "creatures you control of the chosen type".
fn plural(r: &mut Reader<'_>) -> Option<Ref> {
    if let Some(objects) = all(r) {
        return Some(objects);
    }
    let Selector(mut preds) = phrase(r, Form::All)?;
    if r.words("of the chosen color") {
        preds.push(Pred::ChosenColor);
    } else if r.words("of the chosen type") {
        preds.push(Pred::ChosenSubtype);
    }
    Some(Ref::Each(Selector(preds)))
}

/// The permanent a static ability changes: the card, what it is attached to or a group.
fn subject(r: &mut Reader<'_>, itself: bool) -> Option<(Ref, bool)> {
    if itself && r.source() {
        return Some((Ref::This, false));
    }
    let attached = r.attempt(|r| {
        (r.word("equipped") || r.word("enchanted")).then_some(())?;
        r.word("creature").then_some(())
    });
    if attached.is_some() {
        return Some((Ref::Attached, false));
    }
    plural(r).map(|group| (group, true))
}

/// "~ can't block.", "Enchanted creature can't attack or block."
fn cannot(r: &mut Reader<'_>) -> Option<Ability> {
    let (affected, _) = subject(r, true)?;
    r.need("can't")?;
    let mods = if r.words("attack or block") {
        vec![Modification::CantAttack, Modification::CantBlock]
    } else if r.word("attack") {
        vec![Modification::CantAttack]
    } else if r.word("block") {
        vec![Modification::CantBlock]
    } else {
        return None;
    };
    Some(statics(affected, mods))
}

/// "Equipped creature gets +2/+0.", "Creatures you control have haste.", "Enchanted creature gets
/// +1/+1 and has flying."; an X means nothing to a permanent, so it is not read.
fn modifies(r: &mut Reader<'_>) -> Option<Ability> {
    let (affected, plural) = subject(r, false)?;
    let x_free = std::mem::replace(&mut r.x_free, false);
    let mods = r.attempt(|r| {
        let (gets, has) = if plural {
            ("get", "have")
        } else {
            ("gets", "has")
        };
        if r.word(gets) {
            let mut mods = vec![power_toughness(r)?];
            if r.word("and") {
                r.word(has).then_some(())?;
                mods.extend(granted(r)?);
            }
            Some(mods)
        } else {
            r.word(has).then_some(())?;
            granted(r)
        }
    });
    r.x_free = x_free;
    Some(statics(affected, mods?))
}

fn statics(affected: Ref, mods: Vec<Modification>) -> Ability {
    Ability::Static {
        affected,
        mods,
        condition: None,
        defining: false,
        from: ZoneKind::Battlefield,
    }
}

/// Every wording of a static ability, a replacement of the entering or a rule the readers produce;
/// empty for any other ability.
pub fn texts(ability: &Ability) -> Vec<String> {
    let sentences = match ability {
        Ability::Replacement {
            when: ReplaceWhen::Enters { what },
            by,
            condition,
            from: ZoneKind::Battlefield,
        } if *what == Selector::this() => entering_texts(by, condition.as_ref()),
        Ability::Rule {
            rule,
            condition: None,
            from,
        } => rule_texts(rule, *from),
        Ability::Static {
            affected,
            mods,
            condition: None,
            defining: false,
            from: ZoneKind::Battlefield,
        } => static_texts(affected, mods),
        _ => Vec::new(),
    };
    sentences
        .iter()
        .map(|sentence| format!("{}.", capitalize(sentence)))
        .collect()
}

fn entering_texts(by: &ReplaceBy, condition: Option<&Condition>) -> Vec<String> {
    match (by, condition) {
        (ReplaceBy::Tapped, None) => vec!["~ enters tapped".to_owned()],
        (ReplaceBy::Tapped, Some(Condition::Not(held))) => control_texts(held)
            .into_iter()
            .map(|held| format!("~ enters tapped unless {held}"))
            .collect(),
        (ReplaceBy::WithCounters { counter, count }, None) => {
            counters::entering_text(counter, count)
                .map(|counters| format!("~ enters with {counters} on it"))
                .into_iter()
                .collect()
        }
        (ReplaceBy::Choose(EntersChoice::Color), None) => {
            vec!["As ~ enters, choose a color".to_owned()]
        }
        (ReplaceBy::Choose(EntersChoice::CreatureType), None) => {
            vec!["As ~ enters, choose a creature type".to_owned()]
        }
        (ReplaceBy::PayLifeOrTapped(life), None) => vec![format!(
            "As ~ enters, you may pay {life} life. If you don't, it enters tapped"
        )],
        _ => Vec::new(),
    }
}

/// "you control two or more basic lands": a count of what a player controls, as `control` reads it.
fn control_texts(condition: &Condition) -> Vec<String> {
    let Condition::Compare {
        left: Amount::Count(Selector(preds)),
        op,
        right: Amount::Fixed(number),
    } = condition
    else {
        return Vec::new();
    };
    let Ok(number) = u32::try_from(*number) else {
        return Vec::new();
    };
    let mut rest: Vec<Pred> = preds.clone();
    let Some(at) = rest
        .iter()
        .position(|pred| matches!(pred, Pred::ControlledBy(_)))
    else {
        return Vec::new();
    };
    let (verb, one) = match rest.remove(at) {
        Pred::ControlledBy(Ref::You) => ("you control", true),
        Pred::ControlledBy(Ref::Opponents) => ("your opponents control", false),
        _ => return Vec::new(),
    };
    let selector = Selector(rest);
    match (op, number) {
        (Cmp::Ge, 1) if one => single_texts(&selector)
            .into_iter()
            .map(|kind| format!("{verb} {kind}"))
            .collect(),
        (Cmp::Ge | Cmp::Le, 2..) | (Cmp::Le, 1) => {
            let (Some(words), Some(group)) =
                (count_word(number), phrase_text(&selector, Form::All))
            else {
                return Vec::new();
            };
            let more = if *op == Cmp::Ge { "more" } else { "fewer" };
            vec![format!("{verb} {words} or {more} {group}")]
        }
        _ => Vec::new(),
    }
}

/// "a planeswalker", "a Forest or a Plains" and "a Forest or Plains".
fn single_texts(selector: &Selector) -> Vec<String> {
    if let [Pred::AnyOf(kinds)] = selector.0.as_slice() {
        let names: Option<Vec<&str>> = kinds
            .iter()
            .map(|kind| match kind {
                Pred::Subtype(Subtype(name)) => Some(name.as_str()),
                _ => None,
            })
            .collect();
        let Some((first, others)) = names.as_deref().and_then(|names| names.split_first()) else {
            return Vec::new();
        };
        let with_articles: Vec<String> = others
            .iter()
            .map(|name| format!("{} {name}", article(name)))
            .collect();
        return vec![
            format!(
                "{} {first} or {}",
                article(first),
                with_articles.join(" or ")
            ),
            format!("{} {first} or {}", article(first), others.join(" or ")),
        ];
    }
    if selector.0.contains(&Pred::Other) {
        return Vec::new();
    }
    phrase_text(selector, Form::Subject)
        .map(|text| format!("{} {text}", article(&text)))
        .into_iter()
        .collect()
}

fn rule_texts(rule: &GameRule, from: ZoneKind) -> Vec<String> {
    match (rule, from) {
        (GameRule::CantBeCountered(Ref::This), ZoneKind::Stack) => {
            vec!["~ can't be countered".to_owned()]
        }
        (GameRule::RevealTop(Ref::You), ZoneKind::Battlefield) => {
            vec!["Play with the top card of your library revealed".to_owned()]
        }
        (GameRule::MustAttack(who), ZoneKind::Battlefield) => each_combat(who, "attack"),
        (GameRule::MustBlock(who), ZoneKind::Battlefield) => each_combat(who, "block"),
        _ => Vec::new(),
    }
}

/// "~ attacks each combat if able", "all creatures attack each combat if able".
fn each_combat(who: &Ref, verb: &str) -> Vec<String> {
    subjects(who, true)
        .into_iter()
        .map(|(subject, plural)| {
            let verb = if plural {
                verb.to_owned()
            } else {
                format!("{verb}s")
            };
            format!("{subject} {verb} each combat if able")
        })
        .collect()
}

/// The subjects of a static ability and whether each is plural: the card, what it is attached to
/// (an Equipment says "equipped", an Aura "enchanted") or a group.
fn subjects(affected: &Ref, itself: bool) -> Vec<(String, bool)> {
    match affected {
        Ref::This if itself => vec![("~".to_owned(), false)],
        Ref::Attached => vec![
            ("Equipped creature".to_owned(), false),
            ("Enchanted creature".to_owned(), false),
        ],
        Ref::Each(selector) => {
            let (preds, chosen) = match selector.0.split_last() {
                Some((Pred::ChosenColor, rest)) => (rest, " of the chosen color"),
                Some((Pred::ChosenSubtype, rest)) => (rest, " of the chosen type"),
                _ => (selector.0.as_slice(), ""),
            };
            let chosen_inside = preds
                .iter()
                .any(|pred| matches!(pred, Pred::ChosenColor | Pred::ChosenSubtype));
            if chosen_inside {
                return Vec::new();
            }
            let Some(group) = phrase_text(&Selector(preds.to_vec()), Form::All) else {
                return Vec::new();
            };
            // "All creatures", but "Creatures you control" and "Attacking creatures".
            let prefix = if !preds.is_empty() && preds.iter().all(is_a_type) {
                "all "
            } else {
                ""
            };
            vec![(format!("{prefix}{group}{chosen}"), true)]
        }
        _ => Vec::new(),
    }
}

fn static_texts(affected: &Ref, mods: &[Modification]) -> Vec<String> {
    subjects(affected, true)
        .into_iter()
        .filter_map(|(subject, plural)| {
            let predicate = predicate(mods, plural, matches!(affected, Ref::This))?;
            Some(format!("{subject} {predicate}"))
        })
        .collect()
}

/// "can't attack or block", "gets +1/+1 and has flying", "have haste".
fn predicate(mods: &[Modification], plural: bool, itself: bool) -> Option<String> {
    match mods {
        [Modification::CantAttack, Modification::CantBlock] => {
            return Some("can't attack or block".to_owned());
        }
        [Modification::CantAttack] => return Some("can't attack".to_owned()),
        [Modification::CantBlock] => return Some("can't block".to_owned()),
        // A card does not give itself a bonus.
        _ if itself => return None,
        _ => {}
    }
    let (gets, has) = if plural {
        ("get", "have")
    } else {
        ("gets", "has")
    };
    let (stats, rest) = match mods.split_first() {
        Some((Modification::ModifyPT { power, toughness }, rest)) => (
            Some(format!(
                "{gets} {}/{}",
                signed_text(power)?,
                signed_text(toughness)?
            )),
            rest,
        ),
        _ => (None, mods),
    };
    let keywords = rest
        .iter()
        .map(|modification| match modification {
            Modification::AddKeyword(keyword) => keyword_text(keyword),
            _ => None,
        })
        .collect::<Option<Vec<String>>>()?;
    let mut parts: Vec<String> = stats.into_iter().collect();
    if !keywords.is_empty() {
        parts.push(format!("{has} {}", keywords.join(" and ")));
    }
    (!parts.is_empty()).then(|| parts.join(" and "))
}
