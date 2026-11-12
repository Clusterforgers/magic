//! Wordings of round 28's sixth pack: a permanent that enters with counters and a quoted ability ("it enters with a +1/+1
//! counter on it and with \"Pay 3 life: Regenerate this creature.\""), the same entry under one kicker of two ("If ~ was
//! kicked with its {1}{U} kicker, it enters with ..."), damage prevented and turned into another event ("If damage would
//! be dealt to ~ while it has a +1/+1 counter on it, prevent that damage and remove that many +1/+1 counters from it.")
//! a casting condition said after "Cast this spell only if", and a bonus counted over permanents with a name ("~ gets
//! +1/+1 for each land you control named Wastes."), and the card's join rules of combat ("~ can't attack unless at
//! least two other creatures attack", "~ can only attack alone"), and an attack
//! never made twice at one player in a turn ("~ can't attack a player it has already attacked this turn."), the quoted
//! "bands with other" of a grant, damage prevented from the creatures the card blocks, and an entry with counters
//! unless the card was cast from its owner's hand, and life the other players gain in place of the mana cost.

use crate::ability::Ability;
use crate::compile::read::conditional::condition_texts;
use crate::compile::read::selectors::{Form, phrase_text};
use crate::compile::read::triggers::condition_text;
use crate::effect::Effect;
use crate::modify::{CombatQuirk, GameRule, Modification};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::{Amount, Cmp, Condition, CounterKind, StatKind};
use mtg_core::card::CardType;

/// Adds the wordings of the shapes this family writes.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for text in kicked_entry(ability)
        .into_iter()
        .chain(prevent_and(ability))
        .chain(cast_only(ability))
        .chain(for_each_named(ability))
        .chain(joins(ability))
        .chain(not_again(ability))
        .chain(bands_with_other(ability))
        .chain(by_blocked(ability))
        .chain(not_cast_from_hand(ability))
        .chain(others_gain_instead(ability))
    {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The words of every condition form the readers know.
fn said(condition: &Condition) -> Vec<String> {
    let mut all = condition_texts(condition);
    if let Some(one) = condition_text(condition)
        && !all.contains(&one)
    {
        all.push(one);
    }
    all
}

/// The wordings of an entry with counters, and with a quoted ability when the entry grants one, under no condition.
fn entry_texts(ability: &Ability) -> Vec<String> {
    let Ability::Replacement { when, by, from, .. } = ability else {
        return Vec::new();
    };
    let plain = || {
        crate::compile::written_variants(&Ability::Replacement {
            when: when.clone(),
            by: by.clone(),
            condition: None,
            from: *from,
        })
    };
    let (counter, count, granted) = match by {
        ReplaceBy::WithCountersAnd {
            counter,
            count,
            mods,
        } => {
            let [Modification::GrantAbility(granted)] = mods.as_slice() else {
                return plain();
            };
            (counter, count, granted)
        }
        ReplaceBy::WithCounters { .. } => return plain(),
        _ => return Vec::new(),
    };
    let twin = Ability::Replacement {
        when: when.clone(),
        by: ReplaceBy::WithCounters {
            counter: counter.clone(),
            count: count.clone(),
            tapped: false,
        },
        condition: None,
        from: *from,
    };
    let quoted: Vec<String> = crate::compile::written_variants(granted)
        .into_iter()
        .map(|text| text.replace('~', "this creature"))
        .collect();
    let mut texts = Vec::new();
    for text in crate::compile::written_variants(&twin) {
        let Some(rest) = text.strip_suffix(" on it.") else {
            continue;
        };
        for inner in &quoted {
            texts.push(format!("{rest} on it and with \"{inner}\""));
        }
    }
    texts
}

/// "If ~ was kicked with its {1}{U} kicker, it enters with ...", "If ~ was kicked, it enters with ... and with \"...\"".
fn kicked_entry(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when: ReplaceWhen::Enters { .. },
        condition: Some(condition),
        ..
    } = ability
    else {
        return Vec::new();
    };
    let head = match condition {
        Condition::Kicked => "If ~ was kicked".to_owned(),
        Condition::KickedWith(cost) => {
            let Ok(serde_json::Value::String(symbols)) = serde_json::to_value(cost) else {
                return Vec::new();
            };
            format!("If ~ was kicked with its {symbols} kicker")
        }
        _ => return Vec::new(),
    };
    entry_texts(ability)
        .into_iter()
        .filter_map(|text| {
            let rest = text.strip_prefix("~ enters with ")?;
            Some(format!("{head}, it enters with {rest}"))
        })
        .collect()
}

/// The words of one effect that reads the prevented damage ("remove that many +1/+1 counters from it").
fn after_prevention(effect: &Effect) -> Vec<String> {
    match effect {
        Effect::RemoveCounters {
            from: Ref::This,
            counter: Some(kind),
            count: Amount::Triggering,
            up_to: false,
        } => {
            let kind = counter_name(kind);
            vec![
                format!("remove that many {kind} counters from it"),
                format!("remove that many {kind} counters from ~"),
            ]
        }
        Effect::PutCounters {
            on: Ref::This,
            counter,
            count: Amount::Triggering,
        } => vec![format!(
            "put that many {} counters on it",
            counter_name(counter)
        )],
        Effect::Mill {
            who: Ref::You,
            count: Amount::Times { by: 2, of },
            bind: None,
        } if **of == Amount::Triggering => vec!["mill twice that many cards".to_owned()],
        _ => Vec::new(),
    }
}

/// The printed name of a counter kind.
fn counter_name(kind: &CounterKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// "If damage would be dealt to ~ while <condition>, prevent that damage and <effect>."
fn prevent_and(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when: ReplaceWhen::Damage { to, .. },
        by: ReplaceBy::PreventAnd(effects),
        condition,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let Ok(pattern) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let damage = pattern.pointer("/Replacement/when/Damage");
    if damage
        .and_then(serde_json::Value::as_object)
        .map(serde_json::Map::len)
        != Some(1)
    {
        return Vec::new();
    }
    let target = match to {
        Some(Ref::This) => "~",
        Some(Ref::You) => "you",
        _ => return Vec::new(),
    };
    let [effect] = effects.as_slice() else {
        return Vec::new();
    };
    let whiles: Vec<String> = match condition {
        None => vec![String::new()],
        Some(Condition::Monarch(Ref::You)) => vec![" while you're the monarch".to_owned()],
        Some(Condition::Compare { left, op, right })
            if *op == Cmp::Ge && *right == Amount::Fixed(1) =>
        {
            match left {
                Amount::Counters {
                    on: Ref::This,
                    counter,
                } => {
                    let kind = counter_name(counter);
                    let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
                        "an"
                    } else {
                        "a"
                    };
                    vec![format!(" while it has {article} {kind} counter on it")]
                }
                _ => return Vec::new(),
            }
        }
        Some(other) => said(other)
            .into_iter()
            .map(|words| format!(" while {words}"))
            .collect(),
    };
    let mut texts = Vec::new();
    for words in after_prevention(effect) {
        for during in &whiles {
            texts.push(format!(
                "If damage would be dealt to {target}{during}, prevent that damage and {words}."
            ));
        }
    }
    texts
}

/// "Cast this spell only if <condition>."
fn cast_only(ability: &Ability) -> Vec<String> {
    let Ability::CastOnly { condition } = ability else {
        return Vec::new();
    };
    let mut words = said(condition);
    if let Condition::Compare { left, op, right } = condition
        && *op == Cmp::Ge
        && *right == Amount::Fixed(1)
        && serde_json::to_value(left).ok()
            == Some(serde_json::json!({"Tallied": {"who": "Opponents", "what": "LifeLost"}}))
    {
        words.push("an opponent lost life this turn".to_owned());
    }
    let mut texts: Vec<String> = words
        .iter()
        .flat_map(|words| {
            [
                format!("Cast this spell only if {words}."),
                format!("You can't cast ~ unless {words}."),
            ]
        })
        .collect();
    if let Condition::Compare { left, op, right } = condition
        && *op == Cmp::Ge
        && *right == Amount::Fixed(1)
        && serde_json::to_value(left).ok()
            == Some(serde_json::json!({"Tallied": {"who": "EachPlayer", "what": "CreaturesDied"}}))
    {
        texts.push("Cast this spell only if a creature died this turn.".to_owned());
    }
    texts
}

/// "~ gets +1/+1 for each land you control named Wastes.": the count's selector without its name, then "named" and the
/// name.
fn for_each_named(ability: &Ability) -> Vec<String> {
    let Ability::Static {
        affected: Ref::This,
        mods,
        condition: None,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let [
        Modification::ModifyPT {
            power: Amount::Count(power),
            toughness: Amount::Count(toughness),
        },
    ] = mods.as_slice()
    else {
        return Vec::new();
    };
    if power != toughness {
        return Vec::new();
    }
    let mut name = None;
    let rest: Vec<Pred> = power
        .0
        .iter()
        .filter(|pred| match pred {
            Pred::Named(named) => {
                name = Some(named.clone());
                false
            }
            _ => true,
        })
        .cloned()
        .collect();
    let Some(name) = name else {
        return Vec::new();
    };
    let Some(noun) = phrase_text(&Selector(rest), Form::Each) else {
        return Vec::new();
    };
    vec![format!("~ gets +1/+1 for each {noun} named {name}.")]
}

/// The words of a join rule of the card itself: "~ can't attack unless at least two other creatures attack", "~ can't
/// block unless a creature with greater power also blocks", "~ can only attack alone".
fn joins(ability: &Ability) -> Vec<String> {
    let Ability::Rule {
        rule: GameRule::Combat {
            who: Ref::This,
            quirk,
        },
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    let (verb, verbs, others, among) = match quirk {
        CombatQuirk::AttackAlone => return vec!["~ can only attack alone.".to_owned()],
        CombatQuirk::AttackWithAll => {
            return vec!["If ~ attacks, all creatures you control attack if able.".to_owned()];
        }
        CombatQuirk::AttackWithOthers { others, among } => ("attack", "attacks", *others, among),
        CombatQuirk::BlockWithOthers { others, among } => ("block", "blocks", *others, among),
        _ => return Vec::new(),
    };
    if among.is_empty() {
        let Some(count) = ["two", "three", "four", "five"].get(others.wrapping_sub(2) as usize)
        else {
            return Vec::new();
        };
        return vec![format!(
            "~ can't {verb} unless at least {count} other creatures {verb}."
        )];
    }
    if others != 1 {
        return Vec::new();
    }
    let greater = Pred::Compare {
        stat: StatKind::Power,
        op: Cmp::Gt,
        to: Amount::Stat {
            of: Ref::This,
            stat: StatKind::Power,
        },
    };
    let noun = if among.0 == [greater] {
        "creature with greater power".to_owned()
    } else {
        let mut words = vec![Pred::Type(CardType::Creature)];
        words.extend(among.0.iter().cloned());
        let Some(noun) = phrase_text(&Selector(words), Form::Subject) else {
            return Vec::new();
        };
        noun
    };
    let article = if noun.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    vec![format!(
        "~ can't {verb} unless {article} {noun} also {verbs}."
    )]
}

/// "~ can't attack a player it has already attacked this turn.": the attack is open only toward a player the card has
/// not attacked this turn.
fn not_again(ability: &Ability) -> Vec<String> {
    let Ok(value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let again = serde_json::json!({"Rule": {"rule": {"Cant": {"who": "This", "action": {"Attack": {"unless": {"Not": {
        "Matches": {"subject": "This", "selector": [{"DidThisTurn": {"event": "Attacked", "to": "You"}}]}
    }}}}}}}});
    if value == again {
        vec!["~ can't attack a player it has already attacked this turn.".to_owned()]
    } else {
        Vec::new()
    }
}

/// "Red legendary creatures you control have \"bands with other legendary creatures.\"": the quoted keyword of a grant.
fn bands_with_other(ability: &Ability) -> Vec<String> {
    let Ability::Static {
        affected: Ref::Each(who),
        mods,
        condition: None,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let [Modification::AddKeyword(crate::keyword::Keyword::BandsWithOther(quality))] =
        mods.as_slice()
    else {
        return Vec::new();
    };
    let (Some(who), Some(quality)) = (phrase_text(who, Form::All), phrase_text(quality, Form::All))
    else {
        return Vec::new();
    };
    let mut first = who.chars();
    let Some(head) = first.next() else {
        return Vec::new();
    };
    vec![format!(
        "{}{} have \"bands with other {quality}.\"",
        head.to_uppercase(),
        first.as_str()
    )]
}

/// "Prevent all damage that would be dealt to ~ by creatures it's blocking."
fn by_blocked(ability: &Ability) -> Vec<String> {
    let Ok(value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let wanted = serde_json::json!({"Replacement": {"when": {"Damage": {"to": "This", "source": [
        {"Type": "Creature"}, {"BlockedBy": "This"}
    ]}}, "by": "Prevent"}});
    if value == wanted {
        vec!["Prevent all damage that would be dealt to ~ by creatures it's blocking.".to_owned()]
    } else {
        Vec::new()
    }
}

/// "~ enters with three +1/+1 counters on it if you didn't cast it from your hand.": an entry of the card itself unless
/// its owner cast it from their hand, written from the entry without the condition.
fn not_cast_from_hand(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when: ReplaceWhen::Enters { what, .. },
        by,
        condition: None,
        from,
    } = ability
    else {
        return Vec::new();
    };
    let Ok(value) = serde_json::to_value(what) else {
        return Vec::new();
    };
    if value != serde_json::json!(["This", {"Not": {"WasCast": {"by": "You", "from": "Hand"}}}]) {
        return Vec::new();
    }
    let Ok(plain) =
        serde_json::from_value::<ReplaceWhen>(serde_json::json!({"Enters": {"what": ["This"]}}))
    else {
        return Vec::new();
    };
    let twin = Ability::Replacement {
        when: plain,
        by: by.clone(),
        condition: None,
        from: *from,
    };
    crate::compile::written_variants(&twin)
        .into_iter()
        .filter_map(|text| {
            let rest = text.strip_suffix('.')?;
            Some(format!("{rest} if you didn't cast it from your hand."))
        })
        .collect()
}

/// "If you control a Forest, rather than pay this spell's mana cost, you may have each other player gain 6 life."
fn others_gain_instead(ability: &Ability) -> Vec<String> {
    let Ability::FreeCast {
        condition,
        cost: Some(cost),
    } = ability
    else {
        return Vec::new();
    };
    let [crate::cost::CostItem::OthersGainLife(amount)] = cost.0.as_slice() else {
        return Vec::new();
    };
    let tail = format!("you may have each other player gain {amount} life.");
    match condition {
        None => vec![format!("Rather than pay this spell's mana cost, {tail}")],
        Some(condition) => said(condition)
            .into_iter()
            .map(|words| format!("If {words}, rather than pay this spell's mana cost, {tail}"))
            .collect(),
    }
}
