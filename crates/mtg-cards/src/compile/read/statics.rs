//! Statics: the abilities of a permanent that need no event. A permanent enters tapped (also
//! unless something holds), with counters, after a choice or after a payment of life; a spell
//! can't be countered; a creature can't block, or attacks each combat; the top card of a library
//! is played revealed; a creature, the creature an Aura or Equipment is attached to, or a group of
//! creatures gets a bonus or a keyword; spells cost more or less. Read from the wording and written back the same way.

use std::sync::Arc;

use mtg_core::card::Subtype;

use super::abilities;
use super::amounts::{count_word, counted_stats, for_each, for_each_text, number_word};
use super::copies;
use super::counters;
use super::defining;
use super::effects::{granted, power_toughness};
use super::evasion;
use super::lands;
use super::life_gain;
use super::prevention;
use super::render::{self, capitalize, counts_stats, is_a_type, stats_text};
use super::selectors::{
    Form, all, color, color_name, each, granted_text, phrase, phrase_text, subtype,
};
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
const TEMPLATES: [Template; 19] = [
    entering,
    entering_tapped_if,
    group_enters_tapped,
    life_gain::replacement,
    no_life_loss,
    copies::enters_as_copy,
    as_entering,
    rule,
    lands::from_a_zone,
    lands::cast_from_the_top,
    cannot,
    cost_modifier,
    modifies,
    defining::ability,
    prevention::static_rule,
    super::costs::additional,
    super::attack_tax::static_rule,
    evasion::cannot_block,
    free_cast,
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

/// "Damage doesn't cause you to lose life.", also "As long as you're the monarch, damage doesn't cause you to lose life.": a
/// rule of the game for its controller, under the condition when there is one.
fn no_life_loss(r: &mut Reader<'_>) -> Option<Ability> {
    let condition = r.attempt(|r| {
        let held = as_long_as(r)?;
        r.punct(',').then_some(held)
    });
    r.words("damage doesn't cause you to lose life").then_some(())?;
    let condition = condition.or_else(|| r.attempt(as_long_as));
    Some(Ability::Rule {
        rule: GameRule::NoLifeLossFromDamage(Ref::You),
        condition,
        from: ZoneKind::Battlefield,
    })
}

/// What `no_life_loss` reads, written back.
fn no_life_loss_texts(condition: Option<&Condition>) -> Vec<String> {
    let rule = "damage doesn't cause you to lose life";
    match condition {
        None => vec![rule.to_owned()],
        Some(held) => as_long_as_texts(held)
            .into_iter()
            .flat_map(|held| {
                [
                    format!("as long as {held}, {rule}"),
                    format!("{rule} as long as {held}"),
                ]
            })
            .collect(),
    }
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
/// "~ enters with two +1/+1 counters on it.", "~ enters tapped with two charge counters on it.",
/// "~ escapes with a +1/+1 counter on it."
fn entering(r: &mut Reader<'_>) -> Option<Ability> {
    r.source().then_some(())?;
    let escaped = r.word("escapes");
    if !escaped {
        r.need("enters")?;
    }
    let tapped = !escaped && r.word("tapped");
    if tapped && !r.words("with") {
        let condition = if r.word("unless") {
            Some(Condition::Not(Box::new(holds(r)?)))
        } else {
            None
        };
        return Some(replacement(ReplaceBy::Tapped, condition));
    }
    if !tapped {
        r.need("with")?;
    }
    let (counter, count) = counters::entering(r)?;
    r.need("on it")?;
    Some(replacement(
        ReplaceBy::WithCounters {
            counter,
            count,
            tapped,
        },
        escaped.then_some(Condition::CastWith(CastWay::Escape)),
    ))
}

/// "If you control two or more other lands, ~ enters tapped.": the card enters tapped when what a player controls holds.
fn entering_tapped_if(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("if")?;
    let condition = control(r)?;
    r.punct(',').then_some(())?;
    r.source().then_some(())?;
    r.need("enters tapped")?;
    Some(replacement(ReplaceBy::Tapped, Some(condition)))
}

/// "If you control a commander, you may cast this spell without paying its mana cost.": a cast open while the condition holds.
fn free_cast(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("if")?;
    let condition = control(r)?;
    r.punct(',').then_some(())?;
    r.need("you may cast")?;
    r.source().then_some(())?;
    r.need("without paying its mana cost")?;
    Some(Ability::FreeCast {
        condition: Some(condition),
    })
}

/// "Creatures your opponents control enter tapped.", "Artifacts and creatures your opponents control enter tapped.": a
/// replacement of the entering of every permanent the group names.
fn group_enters_tapped(r: &mut Reader<'_>) -> Option<Ability> {
    let what = phrase(r, Form::All)?;
    let permanents = !what.0.is_empty()
        && !what
            .0
            .iter()
            .any(|pred| matches!(pred, Pred::Spell | Pred::This | Pred::In(_)));
    permanents.then_some(())?;
    r.words("enter tapped").then_some(())?;
    Some(Ability::Replacement {
        when: ReplaceWhen::Enters { what },
        by: ReplaceBy::Tapped,
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// "As ~ enters, choose a color.", "As it enters, choose a color other than red.", "As ~ enters, you may pay 2 life.
/// If you don't, it enters tapped."
fn as_entering(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("as")?;
    (r.source() || r.word("it")).then_some(())?;
    r.need("enters")?;
    r.punct(',').then_some(())?;
    if r.words("choose a color other than") {
        let other = color(r.take_word()?)?;
        return Some(replacement(
            ReplaceBy::Choose(EntersChoice::ColorOtherThan(other)),
            None,
        ));
    }
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
    if r.words("skip your draw step") {
        return Some(GameRule::SkipDrawStep(Ref::You));
    }
    if r.words("you may look at the top card of your library any time") {
        return Some(GameRule::LookAtTopAnyTime(Ref::You));
    }
    if let Some(rule) = life_gain::rule(r) {
        return Some(rule);
    }
    lands::additional(r)
}

/// A group of permanents: "all creatures", "creatures you control", "other creatures you control",
/// "creatures you control of the chosen type".
pub(super) fn plural(r: &mut Reader<'_>) -> Option<Ref> {
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
    if let Some(group) = each(r) {
        return Some((group, false));
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
    } else if r.words("be blocked except by") {
        vec![Modification::CantBeBlockedBy(evasion::except(phrase(
            r,
            Form::All,
        )?)?)]
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
    // After a condition about the card itself, "it" is the card: "As long as ~ is monstrous, it has menace."
    let it = leading
        .as_ref()
        .is_some_and(|condition| *condition == is_monstrous())
        && r.word("it");
    let (affected, plural) = if it {
        (Ref::This, false)
    } else {
        subject(r, true).or_else(|| enchanted_other(r))?
    };
    let x_free = std::mem::replace(&mut r.x_free, false);
    let mods = r.attempt(|r| predicate_mods(r, plural));
    r.x_free = x_free;
    let mods = mods?;
    let condition = leading.or_else(|| r.attempt(as_long_as));
    if affected == Ref::This && condition.is_none() && !counts_stats(&mods) {
        return None;
    }
    Some(statics(affected, mods, condition))
}

/// What a permanent gets and has: "gets +1/+1", "has flying", "gets +1/+1 and has flying", and as the last part "is
/// goaded" ("gets +2/+2 and is goaded", "has indestructible and is goaded", "gets +1/+1, has deathtouch, and is goaded").
fn predicate_mods(r: &mut Reader<'_>, plural: bool) -> Option<Vec<Modification>> {
    let (gets, has, is) = if plural {
        ("get", "have", "are")
    } else {
        ("gets", "has", "is")
    };
    let goaded = |r: &mut Reader<'_>| r.words(&format!("and {is} goaded"));
    let mut mods = Vec::new();
    if r.word(gets) {
        let stats = power_toughness(r)?;
        mods.push(counted_stats(r, stats)?);
        if goaded(r) {
            mods.push(Modification::Goaded);
            return Some(mods);
        }
        let listed = r.punct(',');
        if listed || r.word("and") {
            r.word(has).then_some(())?;
            mods.extend(gives(r)?);
        }
        if listed {
            r.punct(',').then_some(())?;
            goaded(r).then_some(())?;
            mods.push(Modification::Goaded);
        }
        return Some(mods);
    }
    r.word(has).then_some(())?;
    mods.extend(gives(r)?);
    if goaded(r) {
        mods.push(Modification::Goaded);
    }
    Some(mods)
}

/// What a permanent is given: keywords, or one activated ability in quotes ("Enchanted creature has \"{R}, {T}: Copy
/// target instant or sorcery spell you control.\"", "Lands you control have \"{T}: Add one mana of any color.\"").
fn gives(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    match r.quoted(abilities::activated) {
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
    triggers::monarch_condition(r)
        .or_else(|| r.attempt(monstrous))
        .or_else(|| holds(r))
        .or_else(|| r.attempt(triggers::condition))
}

/// "~ is monstrous": the card itself has become monstrous (CR 701.37).
fn monstrous(r: &mut Reader<'_>) -> Option<Condition> {
    r.source().then_some(())?;
    r.need("is monstrous")?;
    Some(is_monstrous())
}

/// The card itself is monstrous.
fn is_monstrous() -> Condition {
    Condition::Matches {
        subject: Ref::This,
        selector: Selector(vec![Pred::Monstrous]),
    }
}

/// What `as_long_as` reads, written back.
fn as_long_as_texts(condition: &Condition) -> Vec<String> {
    if *condition == Condition::YourTurn {
        vec!["it's your turn".to_owned()]
    } else if *condition == is_monstrous() {
        vec!["~ is monstrous".to_owned()]
    } else if let Some(written) = triggers::condition_text(condition) {
        vec![written]
    } else {
        holds_texts(condition)
    }
}

/// The count of a spell's own cost increase for each target beyond the first.
fn beyond_the_first() -> Amount {
    Amount::Sum(vec![Amount::TargetCount(0), Amount::Fixed(-1)])
}

/// Whether the count is the targets beyond the first.
pub(super) fn counts_targets(per: &Amount) -> bool {
    *per == beyond_the_first()
}

/// The spells a cost modifier names: "This spell", "Noncreature spells", "Artifact spells you cast", "Spells your opponents
/// cast", "Spells you cast from anywhere other than your hand". The flag says the spell is the card itself.
fn cost_subject(r: &mut Reader<'_>) -> Option<(Selector, bool)> {
    if r.source() {
        return Some((Selector::this(), true));
    }
    let mut applies_to = phrase(r, Form::All)?;
    (applies_to.0.first() == Some(&Pred::Spell)).then_some(())?;
    if r.words("you cast") {
        applies_to.0.push(Pred::ControlledBy(Ref::You));
    } else if r.words("your opponents cast") {
        applies_to.0.push(Pred::ControlledBy(Ref::Opponents));
    }
    if r.words("from anywhere other than your hand") {
        applies_to.0.push(not_cast_from_hand());
    }
    Some((applies_to, false))
}

/// "Cast from anywhere other than your hand".
fn not_cast_from_hand() -> Pred {
    Pred::Not(Box::new(Pred::CastFrom(ZoneKind::Hand)))
}

/// The mana of a cost change and whether it is `{X}` (then one mana is changed as many times as "where X is" counts).
fn cost_mana(r: &mut Reader<'_>) -> Option<(ManaCostX, bool)> {
    let mut text = String::new();
    while let Some(symbol) = r.symbol() {
        text.extend(["{", symbol, "}"]);
    }
    let mana: ManaCostX = text.parse().ok()?;
    if text == "{X}" {
        return Some(("{1}".parse().ok()?, true));
    }
    (!mana.has_x()).then_some((mana, false))
}

/// How many times the mana changes the cost: once, for each target beyond the first, for each thing counted, or as
/// many times as the number "where X is" gives.
fn cost_times(r: &mut Reader<'_>, itself: bool, x: bool) -> Option<Amount> {
    if x {
        r.punct(',').then_some(())?;
        r.need("where x is")?;
        let amount = defining::counting(r)?;
        return matches!(amount, Amount::Greatest { .. } | Amount::Total { .. }).then_some(amount);
    }
    if itself && r.words("for each target beyond the first") {
        return Some(beyond_the_first());
    }
    Some(r.attempt(for_each).unwrap_or_else(Amount::one))
}

/// "Noncreature spells cost {1} more to cast.", "Artifact spells you cast cost {1} less to cast.", "Spells your
/// opponents cast cost {1} more to cast.", "This spell costs {1} more to cast for each target beyond the first.",
/// "This spell costs {1} less to cast for each creature card in your graveyard.", "This spell costs {X} less to cast,
/// where X is the total power of creatures you control."
fn cost_modifier(r: &mut Reader<'_>) -> Option<Ability> {
    let (applies_to, itself) = cost_subject(r)?;
    r.need(if itself { "costs" } else { "cost" })?;
    let (mana, x) = cost_mana(r)?;
    let change = if r.word("more") {
        CostChange::Increase
    } else if r.word("less") {
        CostChange::Reduce
    } else {
        return None;
    };
    r.need("to cast")?;
    let per = cost_times(r, itself, x)?;
    Some(Ability::CostModifier {
        applies_to,
        change,
        mana,
        per,
        condition: None,
    })
}

/// The spells a cost modifier names, as the card words them before "cost".
fn cost_subject_text(applies_to: &Selector) -> Option<String> {
    if *applies_to == Selector::this() {
        return Some("~".to_owned());
    }
    let mut other = false;
    let (mut who, mut spells) = (None, Vec::new());
    for pred in &applies_to.0 {
        match pred {
            Pred::ControlledBy(Ref::You) if who.is_none() => who = Some(" you cast"),
            Pred::ControlledBy(Ref::Opponents) if who.is_none() => {
                who = Some(" your opponents cast");
            }
            Pred::ControlledBy(_) => return None,
            _ if *pred == not_cast_from_hand() => other = true,
            _ => spells.push(pred.clone()),
        }
    }
    let selector = Selector(spells);
    (selector.0.first() == Some(&Pred::Spell)).then_some(())?;
    let cast = who.unwrap_or("");
    let from = if other && who.is_some() {
        " from anywhere other than your hand"
    } else if other {
        return None;
    } else {
        ""
    };
    Some(format!(
        "{}{cast}{from}",
        phrase_text(&selector, Form::All)?
    ))
}

/// The wording of a cost modifier the template reads.
fn cost_modifier_texts(
    applies_to: &Selector,
    change: CostChange,
    mana: &ManaCostX,
    per: &Amount,
) -> Vec<String> {
    let itself = *applies_to == Selector::this();
    let Some(subject) = cost_subject_text(applies_to) else {
        return Vec::new();
    };
    let verb = if itself { "costs" } else { "cost" };
    let more = match change {
        CostChange::Increase => "more",
        CostChange::Reduce => "less",
    };
    let (mana, tail) = if per.is_one() {
        (mana.to_string(), String::new())
    } else if itself && *per == beyond_the_first() {
        (
            mana.to_string(),
            " for each target beyond the first".to_owned(),
        )
    } else if let Some(each) = for_each_text(per) {
        (mana.to_string(), format!(" for each {each}"))
    } else if matches!(per, Amount::Greatest { .. } | Amount::Total { .. })
        && mana.to_string() == "{1}"
    {
        let Some(count) = defining::counting_text(per) else {
            return Vec::new();
        };
        ("{X}".to_owned(), format!(", where X is {count}"))
    } else {
        return Vec::new();
    };
    vec![format!("{subject} {verb} {mana} {more} to cast{tail}")]
}

pub(super) fn statics(
    affected: Ref,
    mods: Vec<Modification>,
    condition: Option<Condition>,
) -> Ability {
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
        Ability::Replacement {
            when: ReplaceWhen::Enters { what },
            by: ReplaceBy::Tapped,
            condition: None,
            from: ZoneKind::Battlefield,
        } => phrase_text(what, Form::All)
            .map(|group| format!("{group} enter tapped"))
            .into_iter()
            .collect(),
        Ability::Rule {
            rule: GameRule::NoLifeLossFromDamage(Ref::You),
            condition,
            from: ZoneKind::Battlefield,
        } => no_life_loss_texts(condition.as_ref()),
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
        } => {
            let mut written = static_texts(affected, mods, condition.as_ref());
            written.extend(evasion::texts(affected, mods, condition.as_ref()));
            written
        }
        Ability::CostModifier {
            applies_to,
            change,
            mana,
            per,
            condition: None,
        } => cost_modifier_texts(applies_to, *change, mana, per),
        Ability::FreeCast {
            condition: Some(held),
        } => control_texts(held)
            .into_iter()
            .map(|held| format!("if {held}, you may cast ~ without paying its mana cost"))
            .collect(),
        Ability::Replacement {
            when: ReplaceWhen::LifeGain { .. },
            ..
        } => life_gain::replacement_sentences(ability),
        Ability::Static { defining: true, .. } => defining::sentences(ability),
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
        (ReplaceBy::Tapped, Some(held)) => control_texts(held)
            .into_iter()
            .map(|held| format!("if {held}, ~ enters tapped"))
            .collect(),
        (
            ReplaceBy::WithCounters {
                counter,
                count,
                tapped,
            },
            None,
        ) => counters::entering_text(counter, count)
            .map(|counters| {
                let tapped = if *tapped { " tapped" } else { "" };
                format!("~ enters{tapped} with {counters} on it")
            })
            .into_iter()
            .collect(),
        (
            ReplaceBy::WithCounters { counter, count, .. },
            Some(Condition::CastWith(CastWay::Escape)),
        ) => counters::entering_text(counter, count)
            .map(|counters| format!("~ escapes with {counters} on it"))
            .into_iter()
            .collect(),
        (ReplaceBy::Choose(EntersChoice::Color), None) => {
            vec!["As ~ enters, choose a color".to_owned()]
        }
        (ReplaceBy::Choose(EntersChoice::ColorOtherThan(other)), None) => {
            let other = color_name(*other);
            vec![
                format!("As ~ enters, choose a color other than {other}"),
                format!("As it enters, choose a color other than {other}"),
            ]
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
        (GameRule::SkipDrawStep(Ref::You), ZoneKind::Battlefield) => {
            vec!["Skip your draw step".to_owned()]
        }
        (GameRule::LookAtTopAnyTime(Ref::You), ZoneKind::Battlefield) => {
            vec!["You may look at the top card of your library any time".to_owned()]
        }
        (GameRule::AdditionalLandPlays { who, count }, ZoneKind::Battlefield) => {
            lands::additional_texts(who, count)
        }
        (GameRule::MayCast(permission), ZoneKind::Battlefield) => {
            lands::from_a_zone_text(permission)
                .into_iter()
                .chain(lands::cast_from_the_top_text(permission))
                .collect()
        }
        (GameRule::CantGainLife(who), ZoneKind::Battlefield) => life_gain::rule_sentences(who),
        (GameRule::CanBeCommander, ZoneKind::Battlefield) => {
            vec!["~ can be your commander".to_owned()]
        }
        (GameRule::MustAttack(who), ZoneKind::Battlefield) => each_combat(who, "attack"),
        (GameRule::MustBlock(who), ZoneKind::Battlefield) => each_combat(who, "block"),
        (GameRule::DamageCantBePrevented { source, combat }, ZoneKind::Battlefield) => {
            prevention::static_texts(source, *combat)
        }
        (GameRule::AttackTax { .. }, ZoneKind::Battlefield) => {
            super::attack_tax::static_texts(rule)
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
pub(super) fn subjects(affected: &Ref, itself: bool) -> Vec<(String, bool)> {
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
            // One at a time: "Each creature you control with a +1/+1 counter on it has flying."
            let each = phrase_text(&Selector(preds.to_vec()), Form::Each)
                .map(|one| (format!("Each {one}{chosen}"), false));
            let Some(group) = phrase_text(&Selector(preds.to_vec()), Form::All) else {
                return each.into_iter().collect();
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
            let mut written = if everyones {
                vec![(format!("all {plain}"), true), (plain, true)]
            } else {
                vec![(plain, true)]
            };
            written.extend(each);
            written
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
            // "As long as ~ is monstrous, it has menace."
            let pronoun = helds
                .iter()
                .filter(|held| held.as_str() == "~ is monstrous")
                .map(|held| format!("as long as {held}, it {predicate}"));
            trailing.chain(leading).chain(pronoun).collect::<Vec<_>>()
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
            // A card does not give itself a bonus, but it may grow by a count: "This creature gets +1/+1 for each Elf."
            _ if itself && !counts_stats(mods) => return None,
            _ => {}
        }
    }
    let (gets, has, is) = if plural {
        ("get", "have", "are")
    } else {
        ("gets", "has", "is")
    };
    // "is goaded" is the last part of the sentence.
    let (mods, goaded) = match mods.split_last() {
        Some((Modification::Goaded, rest)) if !rest.is_empty() => (rest, true),
        _ => (mods, false),
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
    if goaded {
        let last = format!("{is} goaded");
        return match parts.as_slice() {
            [one] => Some(format!("{one} and {last}")),
            [first, second] => Some(format!("{first}, {second}, and {last}")),
            _ => None,
        };
    }
    (!parts.is_empty()).then(|| parts.join(" and "))
}
