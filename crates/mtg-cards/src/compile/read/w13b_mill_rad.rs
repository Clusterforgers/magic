//! Mill and rad leftovers: "Do this only once each turn." after a lone may, "each of up to X target creatures, where X
//! is the number of nonland cards milled this way" after a batch of mills, read and written back.

use super::counters;
use super::library::filter_text;
use super::selectors::{Form, phrase, phrase_in};
use super::tokens::Reader;
use crate::ability::{Ability, Limit};
use crate::effect::Effect;
use crate::refs::{Bounds, Pred, Ref, TargetSpec, Targetable, ZoneKind};
use crate::trigger::{ReplaceBy, TriggerPattern};
use crate::value::{Amount, CounterKind};

/// The limit of a trigger whose one effect is a may, worded as an instruction.
pub(super) const W13B_DO_THIS_ONCE: &str = "do this only once each turn";

/// "put a +1/+1 counter on each of up to X target creatures, where X is the number of nonland cards milled this way.":
/// the most targets is the count of the trigger. The writer checks that the cards are those of the trigger.
pub(super) fn w13b_milled_targets(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    (r.triggered && r.targets.is_empty()).then_some(())?;
    r.word("put").then_some(())?;
    let (counter, count) = counters::entering(r)?;
    r.need("on each of up to")?;
    r.word("x").then_some(())?;
    r.need("target")?;
    let what = phrase(r, Form::Targets)?;
    r.punct(',').then_some(())?;
    r.need("where x is the number of")?;
    phrase_in(r, Form::All, true)?;
    r.need("milled this way")?;
    // The sentence's where clause is read: none is left for `where_x::bind` to find.
    r.where_x = false;
    r.sentence_end().then_some(())?;
    r.targets.push(TargetSpec {
        what: Targetable::Object(what),
        bounds: Bounds::up_to(Some(Amount::Triggering)),
        divide: None,
    });
    effects.push(Effect::PutCounters {
        on: Ref::Target(0),
        counter,
        count,
    });
    Some(())
}

/// "get two rad counters" after "you may": the counters by word are the one choice.
pub(super) fn w13b_may_get(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.word("get").then_some(())?;
    let got = super::w7b_player_counters::w7b_read(r, Ref::You)?;
    Some(vec![Effect::May {
        who: Ref::You,
        effects: vec![got],
    }])
}

/// "counter you have" after "for each rad": the counters of a kind a player holds that you have.
pub(super) fn w13b_you_have(r: &mut Reader<'_>, counter: &CounterKind) -> Option<Amount> {
    super::w7b_player_counters::w7b_player_kind(counter).then_some(())?;
    r.attempt(|r| r.words("counter you have").then_some(()))?;
    Some(Amount::Counters {
        on: Ref::You,
        counter: counter.clone(),
    })
}

/// "rad counter you have": what "for each" counts for the counters of a kind you hold.
pub(super) fn w13b_you_have_text(counter: &CounterKind) -> Option<String> {
    super::w7b_player_counters::w7b_player_kind(counter).then_some(())?;
    Some(format!(
        "{} counter you have",
        counters::kind_text(counter)?
    ))
}

/// "You may have this land enter tapped. If you do, you get two rad counters.": the entry its controller may tap for
/// counters by word.
pub(super) fn w13b_tapped_for_counters(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("you may have")?;
    r.source().then_some(())?;
    r.need("enter tapped")?;
    r.punct('.').then_some(())?;
    r.need("if you do")?;
    r.punct(',').then_some(())?;
    r.need("you get")?;
    let (counter, count) = counters::entering(r)?;
    super::w7b_player_counters::w7b_player_kind(&counter).then_some(())?;
    let Amount::Fixed(count) = count else {
        return None;
    };
    let count = u32::try_from(count).ok().filter(|count| *count > 0)?;
    r.sentence_end().then_some(())?;
    Some(super::statics::replacement(
        ReplaceBy::TappedForCounters { counter, count },
        None,
    ))
}

/// The wording of [`w13b_tapped_for_counters`].
pub(super) fn w13b_tapped_texts(counter: &CounterKind, count: u32) -> Vec<String> {
    let count = i32::try_from(count).ok();
    count
        .and_then(|count| counters::text(counter, &Amount::Fixed(count)))
        .map(|got| format!("You may have ~ enter tapped. If you do, you get {got}"))
        .into_iter()
        .collect()
}

/// "all rad counters" after "loses": every counter of a kind by word that the player has is removed.
pub(super) fn w13b_loses_all(r: &mut Reader<'_>, who: Ref) -> Option<Effect> {
    r.word("all").then_some(())?;
    let counter = counters::kind(r)?;
    r.need("counters")?;
    super::w7b_player_counters::w7b_player_kind(&counter).then_some(())?;
    Some(Effect::RemoveCounters {
        from: who.clone(),
        count: Amount::Counters {
            on: who,
            counter: counter.clone(),
        },
        counter: Some(counter),
        up_to: false,
    })
}

/// "lose all rad counters" ("loses" in the third person) for the removal [`w13b_loses_all`] reads.
pub(super) fn w13b_loses_all_text(effect: &Effect, third: bool) -> Option<String> {
    let Effect::RemoveCounters {
        from,
        counter: Some(counter),
        count: Amount::Counters { on, counter: kind },
        up_to: false,
    } = effect
    else {
        return None;
    };
    (on == from && kind == counter && super::w7b_player_counters::w7b_player_kind(counter))
        .then_some(())?;
    let verb = if third { "loses" } else { "lose" };
    Some(format!(
        "{verb} all {} counters",
        counters::kind_text(counter)?
    ))
}

/// "in a graveyard" before "that was milled this turn": a card in any graveyard, which the next words narrow.
pub(super) fn w13b_in_a_graveyard(r: &mut Reader<'_>) -> Option<Vec<Pred>> {
    r.need("in a graveyard")?;
    r.lookahead(|r| r.words(W13B_MILLED_THIS_TURN).then_some(()))?;
    Some(vec![Pred::In(ZoneKind::Graveyard)])
}

/// The words of `Pred::MilledThisTurn`.
pub(super) const W13B_MILLED_THIS_TURN: &str = "that was milled this turn";

/// The wordings these leftovers add to the ones `texts` holds for `ability`.
pub(super) fn w13b_wordings(ability: &Ability, texts: &mut Vec<String>) {
    for text in texts.iter_mut() {
        // The counters by word are written with their subject ("you get"), which "you may" already says.
        if text.contains("you may you get ") {
            *text = text.replace("you may you get ", "you may get ");
        }
        // A card milled this turn lies "in a graveyard", said before when it was milled.
        if text.contains(" that was milled this turn from a graveyard ") {
            *text = text.replace(
                " that was milled this turn from a graveyard ",
                " in a graveyard that was milled this turn ",
            );
        }
    }
    w13b_once_wordings(ability, texts);
    texts.extend(w13b_milled_targets_wordings(ability));
}

/// "Do this only once each turn." for a trigger limited to once each turn whose one effect is a may.
fn w13b_once_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        limit: Some(Limit::OncePerTurn),
        effects,
        ..
    } = ability
    else {
        return;
    };
    if !matches!(effects.as_slice(), [Effect::May { .. }]) {
        return;
    }
    const SAID: &str = " This ability triggers only once each turn.";
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.ends_with(SAID))
        .map(|text| text.replacen(SAID, " Do this only once each turn.", 1))
        .collect();
    texts.extend(said);
}

/// The wordings of a trigger on a batch of mills whose targets are at most the count of the trigger: the ability is
/// written with two targets and the sentence says X and what X is.
fn w13b_milled_targets_wordings(ability: &Ability) -> Vec<String> {
    let Ability::Triggered { when, targets, .. } = ability else {
        return Vec::new();
    };
    let TriggerPattern::Batch(inner) = when else {
        return Vec::new();
    };
    let TriggerPattern::Milled { what, .. } = &**inner else {
        return Vec::new();
    };
    let [spec] = targets.as_slice() else {
        return Vec::new();
    };
    if spec.bounds != Bounds::up_to(Some(Amount::Triggering)) {
        return Vec::new();
    }
    let Some(cards) = filter_text(what, true) else {
        return Vec::new();
    };
    let mut twin = ability.clone();
    if let Ability::Triggered { targets, .. } = &mut twin {
        targets[0].bounds = Bounds::up_to(Some(Amount::Fixed(2)));
    }
    let tail = format!(", where X is the number of {cards} milled this way");
    super::render::variants(&twin)
        .iter()
        .filter_map(|text| {
            let at = text.find("each of up to two target ")?;
            let text = text.replacen("each of up to two target ", "each of up to X target ", 1);
            let end = at + text[at..].find('.')?;
            Some(format!("{}{tail}{}", &text[..end], &text[end..]))
        })
        .collect()
}
