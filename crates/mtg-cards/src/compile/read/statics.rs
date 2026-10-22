//! Statics: the abilities of a permanent that need no event. A permanent enters tapped (also
//! unless something holds), with counters, after a choice or after a payment of life; a spell
//! can't be countered; a creature can't block, or attacks each combat; the top card of a library
//! is played revealed; a creature, the creature an Aura or Equipment is attached to, or a group of
//! creatures gets a bonus or a keyword; spells cost more or less. Read from the wording and written back the same way.

use std::sync::Arc;

use mtg_core::card::Subtype;

use super::abilities;
use super::amounts::{count_word, counted_stats, number_word};
use super::copies;
use super::counters;
use super::effects::{granted, power_toughness};
use super::lands;
use super::prevention;
use super::render::{self, capitalize, counts_stats, is_a_type, stats_text};
use super::selectors::{Form, all, granted_text, phrase, phrase_text, subtype};
use super::tokens::Reader;
use super::triggers::{self, article};
use crate::ability::{Ability, CostChange};
use crate::mana::ManaCostX;
use crate::modify::{GameRule, Modification};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{EntersChoice, ReplaceBy, ReplaceWhen};
use crate::value::{Amount, CastWay, Cmp, Condition};

type Template = fn(&mut Reader<'_>) -> Option<Ability>;

/// The static shapes, tried in this order.
const TEMPLATES: [Template; 10] = [
    entering,
    copies::enters_as_copy,
    as_entering,
    rule,
    lands::from_a_zone,
    cannot,
    cost_modifier,
    modifies,
    prevention::static_rule,
    super::costs::additional,
];

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
/// "~ enters with two +1/+1 counters on it.", "~ escapes with a +1/+1 counter on it."
fn entering(r: &mut Reader<'_>) -> Option<Ability> {
    r.source().then_some(())?;
    let escaped = r.word("escapes");
    if !escaped {
        r.need("enters")?;
    }
    if !escaped && r.word("tapped") {
        let condition = if r.word("unless") {
            Some(Condition::Not(Box::new(holds(r)?)))
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
        escaped.then_some(Condition::CastWith(CastWay::Escape)),
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
    if r.words("choose a player") {
        return Some(replacement(ReplaceBy::Choose(EntersChoice::Player), None));
    }
    if r.words("choose an opponent") {
        return Some(replacement(ReplaceBy::Choose(EntersChoice::Opponent), None));
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

/// What must hold for a permanent not to enter tapped: one thing, or several joined by "or" ("you have two or more
/// opponents or a player has 13 or less life").
fn holds(r: &mut Reader<'_>) -> Option<Condition> {
    let mut any = vec![held(r)?];
    while let Some(next) = r.attempt(|r| {
        r.need("or")?;
        held(r)
    }) {
        any.push(next);
    }
    if any.len() == 1 {
        any.pop()
    } else {
        Some(Condition::Any(any))
    }
}

/// One thing that can hold: what a player controls, how many opponents there are, or how low a life total is.
fn held(r: &mut Reader<'_>) -> Option<Condition> {
    r.attempt(control)
        .or_else(|| r.attempt(opponents))
        .or_else(|| r.attempt(low_life))
}

/// "you have two or more opponents".
fn opponents(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("you have")?;
    let number = number_word(r.take_word()?)?;
    r.need("or more opponents")?;
    Some(Condition::Compare {
        left: Amount::Players(Ref::Opponents),
        op: Cmp::Ge,
        right: Amount::Fixed(i32::try_from(number).ok()?),
    })
}

/// The players a life total is about and whether one of them is enough ("a player", "an opponent") or all must have it
/// ("each opponent", "each player").
const LIFE_SUBJECTS: [(&str, Ref, bool); 4] = [
    ("a player has", Ref::EachPlayer, true),
    ("an opponent has", Ref::Opponents, true),
    ("each opponent has", Ref::Opponents, false),
    ("each player has", Ref::EachPlayer, false),
];

/// "a player has 13 or less life", "an opponent has 10 or less life", "each opponent has 20 or more life": the lowest or
/// highest life total among the players, whichever one player or every player having it comes to.
fn low_life(r: &mut Reader<'_>) -> Option<Condition> {
    let (who, any) = LIFE_SUBJECTS
        .iter()
        .find_map(|(words, who, any)| r.words(words).then(|| (who.clone(), *any)))?;
    let life = i32::try_from(r.number()?).ok()?;
    let less = if r.words("or less life") {
        true
    } else if r.words("or more life") {
        false
    } else {
        return None;
    };
    // One player with at most the amount is the lowest total at most it; all of them is the highest total at most it.
    let left = if any == less {
        Amount::LowestLife(who)
    } else {
        Amount::HighestLife(who)
    };
    Some(Condition::Compare {
        left,
        op: if less { Cmp::Le } else { Cmp::Ge },
        right: Amount::Fixed(life),
    })
}

/// What a permanent must control for it not to enter tapped: "you control a Forest or a Plains",
/// "you control two or fewer other lands", "your opponents control eight or more lands".
pub(super) fn control(r: &mut Reader<'_>) -> Option<Condition> {
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
    } else if let Some(rule) = r.attempt(player_rule) {
        rule
    } else if r.source() {
        if r.words("can be your commander") {
            GameRule::CanBeCommander
        } else if r.words("can't be countered") {
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

/// The rules about a player's hand, land plays and library: "You have no maximum hand size.", "You may play an
/// additional land on each of your turns.", "You may look at the top card of your library any time."
fn player_rule(r: &mut Reader<'_>) -> Option<GameRule> {
    if r.words("you have no maximum hand size") {
        return Some(GameRule::NoMaxHandSize(Ref::You));
    }
    if r.words("players have no maximum hand size") {
        return Some(GameRule::NoMaxHandSize(Ref::EachPlayer));
    }
    if r.words("you may look at the top card of your library any time") {
        return Some(GameRule::LookAtTopAnyTime(Ref::You));
    }
    lands::additional(r)
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

/// "Enchanted land has ...": an Aura that enchants something other than a creature gives it an ability, and the data
/// holds the permanent as `Attached` all the same. Only a verb "has" follows; the writer says "land", "artifact" and
/// "permanent" for an ability given and no other noun.
fn enchanted_other(r: &mut Reader<'_>) -> Option<(Ref, bool)> {
    r.attempt(|r| {
        r.word("enchanted").then_some(())?;
        (r.word("land") || r.word("artifact") || r.word("permanent")).then_some(())?;
        (r.peek_word() == Some("has")).then_some((Ref::Attached, false))
    })
}

/// "~ can't block.", "Enchanted creature can't attack or block.", "~ can't be blocked.", "~ can't be blocked by
/// creatures with power 2 or less."
fn cannot(r: &mut Reader<'_>) -> Option<Ability> {
    let (affected, _) = subject(r, true)?;
    r.need("can't")?;
    let mods = if r.words("attack or block") {
        vec![Modification::CantAttack, Modification::CantBlock]
    } else if r.words("block and can't be blocked") {
        vec![Modification::CantBlock, Modification::CantBeBlocked]
    } else if r.words("be blocked by") {
        vec![Modification::CantBeBlockedBy(phrase(r, Form::All)?)]
    } else if r.words("be blocked") {
        vec![Modification::CantBeBlocked]
    } else if r.word("attack") {
        vec![Modification::CantAttack]
    } else if r.word("block") {
        vec![Modification::CantBlock]
    } else {
        return None;
    };
    Some(statics(affected, mods, None))
}

/// "Equipped creature gets +2/+0.", "Creatures you control have haste.", "Enchanted creature gets
/// +1/+1 and has flying.", "Equipped creature gets +2/+2 and has protection from white and from black.", "~ has haste as
/// long as an opponent has 10 or less life."; an X means nothing to a permanent, so it is not read. The card itself is
/// given something only under a condition.
fn modifies(r: &mut Reader<'_>) -> Option<Ability> {
    // The condition may come first: "As long as you're the monarch, permanents you control have hexproof."
    let leading = r.attempt(|r| {
        let condition = as_long_as(r)?;
        r.punct(',').then_some(condition)
    });
    let (affected, plural) = subject(r, true).or_else(|| enchanted_other(r))?;
    let x_free = std::mem::replace(&mut r.x_free, false);
    let mods = r.attempt(|r| {
        let (gets, has) = if plural {
            ("get", "have")
        } else {
            ("gets", "has")
        };
        if r.word(gets) {
            let stats = power_toughness(r)?;
            let mut mods = vec![counted_stats(r, stats)?];
            if r.word("and") {
                r.word(has).then_some(())?;
                mods.extend(gives(r)?);
            }
            Some(mods)
        } else {
            r.word(has).then_some(())?;
            gives(r)
        }
    });
    r.x_free = x_free;
    let mods = mods?;
    let condition = leading.or_else(|| r.attempt(as_long_as));
    if affected == Ref::This && condition.is_none() && !counts_stats(&mods) {
        return None;
    }
    Some(statics(affected, mods, condition))
}

/// What a permanent is given: keywords, or one activated ability in quotes ("Enchanted creature has \"{R}, {T}: Copy
/// target instant or sorcery spell you control.\""). A mana ability stays with the table.
fn gives(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    match r.quoted(|r| abilities::activated(r).filter(|ability| !ability.is_mana_ability())) {
        Some(ability) => Some(vec![Modification::GrantAbility(Arc::new(ability))]),
        None => granted(r),
    }
}

/// What `gives` reads, written back.
fn given_text(mods: &[Modification]) -> Option<String> {
    match mods {
        [Modification::GrantAbility(ability)] if matches!(**ability, Ability::Activated { .. }) => {
            Some(format!("\"{}\"", render::ability(ability)?))
        }
        _ => granted_text(mods),
    }
}

/// " as long as an opponent has 10 or less life", " as long as you control a Plains", " as long as it's your turn".
fn as_long_as(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("as long as")?;
    if r.words("it's your turn") {
        return Some(Condition::YourTurn);
    }
    triggers::monarch_condition(r).or_else(|| holds(r))
}

/// What `as_long_as` reads, written back.
fn as_long_as_texts(condition: &Condition) -> Vec<String> {
    if *condition == Condition::YourTurn {
        vec!["it's your turn".to_owned()]
    } else if let Some(monarch) = triggers::monarch_text(condition) {
        vec![monarch]
    } else {
        holds_texts(condition)
    }
}

/// The count of a spell's own cost increase for each target beyond the first.
fn beyond_the_first() -> Amount {
    Amount::Sum(vec![Amount::TargetCount(0), Amount::Fixed(-1)])
}

/// "Noncreature spells cost {1} more to cast.", "Artifact spells you cast cost {1} less to cast.", "Spells your
/// opponents cast cost {1} more to cast.", "This spell costs {1} more to cast for each target beyond the first."
fn cost_modifier(r: &mut Reader<'_>) -> Option<Ability> {
    let itself = r.source();
    let mut applies_to = if itself {
        Selector::this()
    } else {
        let spells = phrase(r, Form::All)?;
        (spells.0.first() == Some(&Pred::Spell)).then_some(())?;
        spells
    };
    if !itself {
        if r.words("you cast") {
            applies_to.0.push(Pred::ControlledBy(Ref::You));
        } else if r.words("your opponents cast") {
            applies_to.0.push(Pred::ControlledBy(Ref::Opponents));
        }
    }
    r.need(if itself { "costs" } else { "cost" })?;
    let mut text = String::new();
    while let Some(symbol) = r.symbol() {
        text.extend(["{", symbol, "}"]);
    }
    let mana: ManaCostX = text.parse().ok().filter(|mana: &ManaCostX| !mana.has_x())?;
    let change = if r.word("more") {
        CostChange::Increase
    } else if r.word("less") {
        CostChange::Reduce
    } else {
        return None;
    };
    r.need("to cast")?;
    let per = if itself && r.words("for each target beyond the first") {
        beyond_the_first()
    } else {
        Amount::one()
    };
    Some(Ability::CostModifier {
        applies_to,
        change,
        mana,
        per,
        condition: None,
    })
}

/// The wording of a cost modifier the template reads.
fn cost_modifier_texts(
    applies_to: &Selector,
    change: CostChange,
    mana: &ManaCostX,
    per: &Amount,
) -> Vec<String> {
    let head = if *applies_to == Selector::this() {
        "~ costs".to_owned()
    } else {
        let (who, spells): (Vec<&Pred>, Vec<&Pred>) = applies_to
            .0
            .iter()
            .partition(|pred| matches!(pred, Pred::ControlledBy(_)));
        let cast = match who.as_slice() {
            [] => "",
            [Pred::ControlledBy(Ref::You)] => " you cast",
            [Pred::ControlledBy(Ref::Opponents)] => " your opponents cast",
            _ => return Vec::new(),
        };
        let selector = Selector(spells.into_iter().cloned().collect());
        if selector.0.first() != Some(&Pred::Spell) || !per.is_one() {
            return Vec::new();
        }
        let Some(phrase) = phrase_text(&selector, Form::All) else {
            return Vec::new();
        };
        format!("{phrase}{cast} cost")
    };
    let tail = if *applies_to == Selector::this() && *per == beyond_the_first() {
        " for each target beyond the first"
    } else if per.is_one() {
        ""
    } else {
        return Vec::new();
    };
    let more = match change {
        CostChange::Increase => "more",
        CostChange::Reduce => "less",
    };
    vec![format!("{head} {mana} {more} to cast{tail}")]
}

fn statics(affected: Ref, mods: Vec<Modification>, condition: Option<Condition>) -> Ability {
    Ability::Static {
        affected,
        mods,
        condition,
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
            condition,
            defining: false,
            from: ZoneKind::Battlefield,
        } => static_texts(affected, mods, condition.as_ref()),
        Ability::CostModifier {
            applies_to,
            change,
            mana,
            per,
            condition: None,
        } => cost_modifier_texts(applies_to, *change, mana, per),
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
        (ReplaceBy::Tapped, Some(Condition::Not(held))) => holds_texts(held)
            .into_iter()
            .map(|held| format!("~ enters tapped unless {held}"))
            .collect(),
        (ReplaceBy::WithCounters { counter, count }, None) => {
            counters::entering_text(counter, count)
                .map(|counters| format!("~ enters with {counters} on it"))
                .into_iter()
                .collect()
        }
        (
            ReplaceBy::WithCounters { counter, count },
            Some(Condition::CastWith(CastWay::Escape)),
        ) => counters::entering_text(counter, count)
            .map(|counters| format!("~ escapes with {counters} on it"))
            .into_iter()
            .collect(),
        (ReplaceBy::Choose(EntersChoice::Color), None) => {
            vec!["As ~ enters, choose a color".to_owned()]
        }
        (ReplaceBy::Choose(EntersChoice::CreatureType), None) => {
            vec!["As ~ enters, choose a creature type".to_owned()]
        }
        (ReplaceBy::Choose(EntersChoice::Player), None) => {
            vec!["As ~ enters, choose a player".to_owned()]
        }
        (ReplaceBy::Choose(EntersChoice::Opponent), None) => {
            vec!["As ~ enters, choose an opponent".to_owned()]
        }
        (ReplaceBy::PayLifeOrTapped(life), None) => vec![format!(
            "As ~ enters, you may pay {life} life. If you don't, it enters tapped"
        )],
        (ReplaceBy::AsCopy { of, except }, None) => {
            copies::enters_text(of, except).into_iter().collect()
        }
        _ => Vec::new(),
    }
}

/// What holds for a permanent not to enter tapped, as `holds` reads it: one thing, or several joined by "or".
fn holds_texts(condition: &Condition) -> Vec<String> {
    let Condition::Any(list) = condition else {
        return control_texts(condition);
    };
    list.iter().fold(vec![String::new()], |written, item| {
        let choices = control_texts(item);
        written
            .iter()
            .flat_map(|start| {
                choices.iter().map(move |choice| {
                    if start.is_empty() {
                        choice.clone()
                    } else {
                        format!("{start} or {choice}")
                    }
                })
            })
            .collect()
    })
}

/// "you control two or more basic lands", "you have two or more opponents", "an opponent has 10 or less life": one thing
/// that holds, as `held` reads it.
pub(super) fn control_texts(condition: &Condition) -> Vec<String> {
    if let Condition::Compare {
        left: Amount::Players(Ref::Opponents),
        op: Cmp::Ge,
        right: Amount::Fixed(number),
    } = condition
    {
        return u32::try_from(*number)
            .ok()
            .and_then(count_word)
            .map(|words| format!("you have {words} or more opponents"))
            .into_iter()
            .collect();
    }
    if let Condition::Compare {
        left: left @ (Amount::LowestLife(who) | Amount::HighestLife(who)),
        op: op @ (Cmp::Le | Cmp::Ge),
        right: Amount::Fixed(life),
    } = condition
    {
        let (less, any) = (
            *op == Cmp::Le,
            matches!(left, Amount::LowestLife(_)) == (*op == Cmp::Le),
        );
        return LIFE_SUBJECTS
            .iter()
            .find(|(_, subject, one)| subject == who && *one == any)
            .map(|(words, _, _)| {
                let end = if less { "less" } else { "more" };
                format!("{words} {life} or {end} life")
            })
            .into_iter()
            .collect();
    }
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
        (GameRule::NoMaxHandSize(Ref::You), ZoneKind::Battlefield) => {
            vec!["You have no maximum hand size".to_owned()]
        }
        (GameRule::NoMaxHandSize(Ref::EachPlayer), ZoneKind::Battlefield) => {
            vec!["Players have no maximum hand size".to_owned()]
        }
        (GameRule::LookAtTopAnyTime(Ref::You), ZoneKind::Battlefield) => {
            vec!["You may look at the top card of your library any time".to_owned()]
        }
        (GameRule::AdditionalLandPlays { who, count }, ZoneKind::Battlefield) => {
            lands::additional_texts(who, count)
        }
        (GameRule::MayCast(permission), ZoneKind::Battlefield) => {
            lands::from_a_zone_text(permission).into_iter().collect()
        }
        (GameRule::CanBeCommander, ZoneKind::Battlefield) => {
            vec!["~ can be your commander".to_owned()]
        }
        (GameRule::MustAttack(who), ZoneKind::Battlefield) => each_combat(who, "attack"),
        (GameRule::MustBlock(who), ZoneKind::Battlefield) => each_combat(who, "block"),
        (GameRule::DamageCantBePrevented { source, combat }, ZoneKind::Battlefield) => {
            prevention::static_texts(source, *combat)
        }
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
            let plain = format!("{prefix}{group}{chosen}");
            // Slivers and tokens are everyone's: "All Sliver creatures get +1/+1", but "Creature tokens get -1/-0".
            let everyones = prefix.is_empty()
                && preds
                    .iter()
                    .any(|pred| matches!(pred, Pred::Subtype(_) | Pred::Token))
                && !preds
                    .iter()
                    .any(|pred| matches!(pred, Pred::ControlledBy(_) | Pred::Other));
            if everyones {
                vec![(format!("all {plain}"), true), (plain, true)]
            } else {
                vec![(plain, true)]
            }
        }
        _ => Vec::new(),
    }
}

fn static_texts(
    affected: &Ref,
    mods: &[Modification],
    condition: Option<&Condition>,
) -> Vec<String> {
    // The wordings of the condition, each as the tail of the sentence; one empty tail for no condition.
    let helds: Vec<String> = condition.map(as_long_as_texts).unwrap_or_default();
    let tails: Vec<String> = if condition.is_some() {
        helds
            .iter()
            .map(|held| format!(" as long as {held}"))
            .collect()
    } else {
        vec![String::new()]
    };
    let mut subjects = subjects(affected, true);
    // An Aura that gives an ability may enchant a land, an artifact or any permanent.
    if *affected == Ref::Attached && matches!(mods, [Modification::GrantAbility(_)]) {
        subjects.extend(
            [
                "Enchanted land",
                "Enchanted artifact",
                "Enchanted permanent",
            ]
            .map(|subject| (subject.to_owned(), false)),
        );
    }
    subjects
        .into_iter()
        .filter_map(|(subject, plural)| {
            let predicate = predicate(
                mods,
                plural,
                matches!(affected, Ref::This),
                condition.is_some(),
            )?;
            Some((subject, predicate))
        })
        .flat_map(|(subject, predicate)| {
            // The condition may also come first: "As long as you're the monarch, creatures you control have flying."
            let leading = helds.iter().map(|held| {
                let mut words = subject.chars();
                let lowered: String = words.next().map_or_else(String::new, |first| {
                    first.to_lowercase().chain(words).collect()
                });
                format!("as long as {held}, {lowered} {predicate}")
            });
            let trailing = tails
                .iter()
                .map(|tail| format!("{subject} {predicate}{tail}"));
            trailing.chain(leading).collect::<Vec<_>>()
        })
        .collect()
}

/// "can't attack or block", "gets +1/+1 and has flying", "have haste". The card itself gets a bonus or a keyword only
/// under a condition, and a restriction is not read under one.
fn predicate(
    mods: &[Modification],
    plural: bool,
    itself: bool,
    conditional: bool,
) -> Option<String> {
    if !conditional {
        match mods {
            [Modification::CantAttack, Modification::CantBlock] => {
                return Some("can't attack or block".to_owned());
            }
            [Modification::CantAttack] => return Some("can't attack".to_owned()),
            [Modification::CantBlock] => return Some("can't block".to_owned()),
            [Modification::CantBeBlocked] => return Some("can't be blocked".to_owned()),
            [Modification::CantBlock, Modification::CantBeBlocked] => {
                return Some("can't block and can't be blocked".to_owned());
            }
            [Modification::CantBeBlockedBy(blockers)] => {
                return Some(format!(
                    "can't be blocked by {}",
                    phrase_text(blockers, Form::All)?
                ));
            }
            // A card does not give itself a bonus, but it may grow by a count: "This creature gets +1/+1 for each Elf."
            _ if itself && !counts_stats(mods) => return None,
            _ => {}
        }
    }
    let (gets, has) = if plural {
        ("get", "have")
    } else {
        ("gets", "has")
    };
    let (stats, rest) = match mods.split_first() {
        Some((Modification::ModifyPT { power, toughness }, rest)) => {
            let (stats, tail) = stats_text(power, toughness)?;
            (Some(format!("{gets} {stats}{tail}")), rest)
        }
        _ => (None, mods),
    };
    let mut parts: Vec<String> = stats.into_iter().collect();
    if !rest.is_empty() {
        parts.push(format!("{has} {}", given_text(rest)?));
    }
    (!parts.is_empty()).then(|| parts.join(" and "))
}
