//! The seat's answers to every question besides priority and combat: mulligans by land count, the least useful cards
//! bottomed or discarded, scry by what the seat needs, and the other choices searched among a few candidates.

use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{
    CardsChoice, CardsOptions, ChoiceOption, ModeChoice, NumberChoice, OptionChoice, ScryChoice,
    ScryOptions, YesNoChoice,
};
use mtg_engine::decision::{Answer, DecisionKind, MulliganOptions, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine};

use super::eval::permanent_value;
use super::seen::Seen;
use super::sim::{Horizon, Search};
use crate::bot::RandomBot;

/// Whether `id` is a land, as the seat reads it.
fn is_land(seen: &Seen<'_>, id: ObjectId) -> bool {
    seen.chars(id)
        .is_some_and(|chars| chars.types.contains(CardType::Land))
}

/// The lands the seat controls.
fn lands_out(seen: &Seen<'_>) -> usize {
    seen.battlefield()
        .iter()
        .filter(|id| seen.controller(**id) == Some(seen.seat()) && is_land(seen, **id))
        .count()
}

/// Whether to keep the opening hand, by a small opening book: three to five lands in seven cards, two only with a
/// spell of mana value two or less to cast early (fewer cards ask for fewer lands); always after two mulligans or when
/// no mulligan is offered.
pub fn keep(seen: &Seen<'_>, options: &MulliganOptions) -> bool {
    if !options.can_mulligan || options.mulligans_taken >= 2 {
        return true;
    }
    let lands = options.hand.iter().filter(|id| is_land(seen, **id)).count();
    let kept = options.hand.len().saturating_sub(options.bottom_if_keep);
    if kept >= 7 {
        let early = options
            .hand
            .iter()
            .any(|id| !is_land(seen, *id) && seen.mana_value(*id) <= 2);
        return (3..=5).contains(&lands) || (lands == 2 && early);
    }
    let (least, most) = (2.min(kept), kept.saturating_sub(2).max(2));
    (least..=most).contains(&lands)
}

/// How useful a card of the seat's hand is: a land while the seat wants lands, a spell it can cast soon, then the
/// rest by how far out of reach they are.
fn usefulness(seen: &Seen<'_>, id: ObjectId, lands: usize) -> f64 {
    if is_land(seen, id) {
        return if count(lands) < super::knobs::knobs().lands_wanted {
            8.0 - 0.5 * count(lands)
        } else {
            0.5
        };
    }
    let cost = count(seen.mana_value(id) as usize);
    let reach = count(lands + 1);
    if cost <= reach {
        5.0 + 0.5 * cost
    } else {
        5.0 - (cost - reach)
    }
}

/// A small count as a float.
fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// The `n` least useful cards of `hand`: lands go once the lands in hand and in play are enough, else the spells
/// furthest out of reach. Each card taken changes what the rest are worth.
pub fn least_useful(seen: &Seen<'_>, hand: &[ObjectId], n: usize) -> Vec<ObjectId> {
    let mut kept: Vec<ObjectId> = hand.to_vec();
    let mut out = Vec::new();
    for _ in 0..n.min(hand.len()) {
        let lands = lands_out(seen) + kept.iter().filter(|id| is_land(seen, **id)).count();
        let Some((at, _)) = kept
            .iter()
            .enumerate()
            .map(|(at, id)| {
                // A land counts itself among the lands it would be one of.
                let without = if is_land(seen, *id) {
                    lands.saturating_sub(1)
                } else {
                    lands
                };
                (at, usefulness(seen, *id, without.min(lands_out(seen) + 3)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
        else {
            break;
        };
        out.push(kept.remove(at));
    }
    out
}

/// Scry: lands stay on top while the seat wants lands, spells while it can cast them soon; the rest go to the bottom.
pub fn scry(seen: &Seen<'_>, options: &ScryOptions) -> ScryChoice {
    let lands = lands_out(seen)
        + seen
            .own_hand()
            .iter()
            .filter(|id| is_land(seen, **id))
            .count();
    let (top, bottom) = options
        .cards
        .iter()
        .partition(|id| usefulness(seen, **id, lands) >= 4.0);
    ScryChoice { top, bottom }
}

/// Cards chosen from `options` without a search: the seat's own permanents least valuable first (a sacrifice), its
/// hand least useful first (a discard), anything else most valuable first; as many as the least count asks, or as
/// many as allowed when they are not the seat's own.
pub fn pick_cards(seen: &Seen<'_>, options: &CardsOptions) -> Vec<ObjectId> {
    let own = |id: ObjectId| match seen.zone(id) {
        Some(Zone::Battlefield) => seen.controller(id).is_some_and(|p| seen.is_teammate(p)),
        Some(Zone::Hand(owner)) => owner == seen.seat(),
        _ => false,
    };
    let mut scored: Vec<(f64, ObjectId)> = options
        .from
        .iter()
        .map(|id| {
            let worth = match seen.zone(*id) {
                Some(Zone::Battlefield) => {
                    seen.body(*id).map_or(0.0, |body| permanent_value(&body))
                }
                Some(Zone::Hand(_)) => usefulness(seen, *id, lands_out(seen)),
                _ => 1.0 + count(seen.mana_value(*id) as usize),
            };
            (if own(*id) { worth } else { -worth }, *id)
        })
        .collect();
    scored.sort_by(|a, b| a.0.total_cmp(&b.0));
    let taking_own = scored.first().is_some_and(|(_, id)| own(*id));
    let wanted = if taking_own || options.max == 0 {
        options.min
    } else {
        options.max
    } as usize;
    // A cost that takes any number of creatures for their power (crew) is asked only because together they are enough.
    let wanted = if options.can_cancel && options.max > options.min {
        options.from.len()
    } else {
        wanted
    };
    scored.into_iter().take(wanted).map(|(_, id)| id).collect()
}

/// The seat's answer to `decision` without a search: the heuristics above where they apply, an automatic payment,
/// yes to a "you may", and otherwise the question alone decides ([`RandomBot::choose`]).
pub fn quick_choice(engine: &Engine, decision: &Decision, random: &mut RandomBot) -> Answer {
    let seen = Seen::new(engine.state(), decision.player);
    match &decision.kind {
        DecisionKind::Mulligan(options) => Answer::Mulligan(if keep(&seen, options) {
            mtg_engine::decision::MulliganChoice::Keep
        } else {
            mtg_engine::decision::MulliganChoice::Mulligan
        }),
        DecisionKind::BottomCards(options) => {
            Answer::BottomCards(least_useful(&seen, &options.hand, options.count))
        }
        DecisionKind::Discard(options) => {
            Answer::Discard(least_useful(&seen, &options.hand, options.count))
        }
        DecisionKind::Scry(options) => Answer::Scry(scry(&seen, options)),
        DecisionKind::ChooseCards(options)
            if !options.one_per_name && options.groups.is_empty() =>
        {
            Answer::ChooseCards(CardsChoice(
                options.within_total(pick_cards(&seen, options)),
            ))
        }
        DecisionKind::YesNo(options) => Answer::YesNo(YesNoChoice(!options.only_no)),
        DecisionKind::PayMana(options) if options.auto_available => {
            Answer::PayMana(PayChoice::Auto)
        }
        DecisionKind::ChooseOption(options) => threatening_color(&seen, &options.options)
            .map_or_else(
                || random.choose_in(decision, engine.state()),
                |at| Answer::ChooseOption(OptionChoice(at)),
            ),
        _ => random.choose_in(decision, engine.state()),
    }
}

/// Among colour options, the colour of the opposing spell or ability nearest the top of the stack, else the colour
/// most opposing permanents share: what protection or a hoser is best named for.
fn threatening_color(seen: &Seen<'_>, options: &[ChoiceOption]) -> Option<usize> {
    let colors: Vec<(usize, mtg_core::color::Color)> = options
        .iter()
        .enumerate()
        .filter_map(|(at, option)| match option {
            ChoiceOption::Color(color) => Some((at, *color)),
            _ => None,
        })
        .collect();
    if colors.is_empty() {
        return None;
    }
    let opposing = |id: &mtg_core::ids::ObjectId| {
        seen.controller(*id)
            .is_some_and(|player| !seen.is_teammate(player))
    };
    for id in seen.stack().iter().rev().filter(|id| opposing(id)) {
        if let Some(chars) = seen.chars(*id)
            && let Some((at, _)) = colors
                .iter()
                .find(|(_, color)| chars.colors.contains(*color))
        {
            return Some(*at);
        }
    }
    let bodies = seen.bodies();
    colors
        .iter()
        .map(|(at, color)| {
            let shared = bodies
                .iter()
                .filter(|body| {
                    !seen.is_teammate(body.controller) && body.chars.colors.contains(*color)
                })
                .count();
            (shared, std::cmp::Reverse(*at))
        })
        .max()
        .filter(|(shared, _)| *shared > 0)
        .map(|(_, std::cmp::Reverse(at))| at)
}

/// A few candidate answers to a choice that a search can tell apart: the quick answer, then each simple alternative.
fn candidates(engine: &Engine, decision: &Decision, random: &mut RandomBot) -> Vec<Answer> {
    let mut answers = vec![quick_choice(engine, decision, random)];
    match &decision.kind {
        DecisionKind::YesNo(options) if !options.only_no => {
            answers.push(Answer::YesNo(YesNoChoice(false)));
        }
        DecisionKind::ChooseMode(options) if options.min <= 1 && options.cheapest.is_empty() => {
            for mode in options.modes.iter().take(6) {
                answers.push(Answer::ChooseMode(ModeChoice::Modes(vec![mode.index])));
            }
        }
        DecisionKind::ChooseNumber(options) => {
            let top = options.max.unwrap_or(options.min.saturating_add(3));
            for number in [top, options.min, options.min.midpoint(top)] {
                if !options.except.contains(&number) {
                    answers.push(Answer::ChooseNumber(NumberChoice::Number(number)));
                }
            }
        }
        DecisionKind::ChooseOption(options) => {
            for at in 0..options.options.len().min(6) {
                answers.push(Answer::ChooseOption(OptionChoice(at)));
            }
        }
        DecisionKind::ChooseCards(_)
        | DecisionKind::ChooseCosts(_)
        | DecisionKind::DivideAmount(_)
        | DecisionKind::ChooseMode(_) => {
            for _ in 0..3 {
                answers.push(random.choose(decision));
            }
        }
        _ => {}
    }
    let mut unique: Vec<Answer> = Vec::new();
    for answer in answers {
        if !unique.contains(&answer) {
            unique.push(answer);
        }
    }
    unique
}

/// The seat's answer to a choice, searched among [`candidates`] played on until what asked it has resolved; the quick
/// answer when there is one candidate or no search is possible.
pub fn searched_choice(
    search: &mut Search,
    engine: &Engine,
    decision: &Decision,
    random: &mut RandomBot,
) -> Answer {
    let mut answers = candidates(engine, decision, random);
    if answers.len() < 2 || engine.decision() != Some(decision) {
        return answers.swap_remove(0);
    }
    let fallback = answers[0].clone();
    search
        .best(engine, answers, Horizon::Settle)
        .map_or(fallback, |(answer, _)| answer)
}
