//! A cost change for a time: "Artifact spells you cast this turn cost {1} less to cast.", "Until your next turn, spells
//! your opponents cast cost {1} more to cast.", "Noncreature spells your opponents cast cost {2} more to cast until your
//! next turn." Read into `GameRule::SpellCost` with its duration and written back; and "target creature becomes the color
//! or colors of your choice until end of turn" written back.

use super::statics::{cost_mana, cost_subject, cost_subject_text};
use super::tokens::Reader;
use crate::ability::CostChange;
use crate::effect::{Effect, ValueKind};
use crate::modify::{Duration, GameRule, Modification};
use crate::refs::{Pred, Ref, Selector};
use crate::value::Amount;
use mtg_core::color::Color;

/// The sentence: a duration before the spells, after who casts them ("this turn") or at its end, exactly once.
pub fn g10d_spell_cost(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let leading = r.words("until your next turn");
    if leading {
        r.punct(',').then_some(())?;
    }
    let (mut applies_to, itself) = cost_subject(r)?;
    (!itself && applies_to.0.first() == Some(&Pred::Spell)).then_some(())?;
    let this_turn = r.words("this turn");
    if this_turn && let Some(colors) = r.attempt(r4703_that_are_colors) {
        applies_to.0.push(colors);
    }
    r.need("cost")?;
    let (mana, x) = cost_mana(r)?;
    (!mana.rest().symbols().is_empty()).then_some(())?;
    let change = if r.word("more") {
        CostChange::Increase
    } else if r.word("less") {
        CostChange::Reduce
    } else {
        return None;
    };
    r.need("to cast")?;
    let per = if x {
        super::c11a_turn_costs::c11a_turn_count(r, true)?
    } else {
        Amount::one()
    };
    let trailing = !x && r.words("until your next turn");
    let duration = match (leading, this_turn, trailing) {
        (true, false, false) | (false, false, true) => Duration::UntilYourNextTurn,
        (false, true, false) => Duration::UntilEndOfTurn,
        _ => return None,
    };
    Some(vec![Effect::Rule {
        rule: GameRule::SpellCost {
            applies_to,
            change,
            mana,
            per,
        },
        duration,
    }])
}

/// The colors a spell may have, joined by "and/or": "that are black and/or red".
fn r4703_that_are_colors(r: &mut Reader<'_>) -> Option<Pred> {
    r.need("that are")?;
    let first = r4703_color(r)?;
    (r.word("and") && r.punct('/') && r.word("or")).then_some(())?;
    let second = r4703_color(r)?;
    Some(Pred::AnyOf(vec![Pred::Color(first), Pred::Color(second)]))
}

/// One color word.
fn r4703_color(r: &mut Reader<'_>) -> Option<Color> {
    Color::ALL
        .into_iter()
        .find(|color| r.word(super::selectors::color_name(*color)))
}

/// The words of a trailing `AnyOf` of two colors: " that are black and/or red".
fn r4703_that_are_text(applies_to: &Selector) -> Option<(Selector, String)> {
    let (last, rest) = applies_to.0.split_last()?;
    let Pred::AnyOf(kinds) = last else {
        return None;
    };
    let [Pred::Color(first), Pred::Color(second)] = kinds.as_slice() else {
        return None;
    };
    let (first, second) = (
        super::selectors::color_name(*first),
        super::selectors::color_name(*second),
    );
    Some((
        Selector(rest.to_vec()),
        format!(" that are {first} and/or {second}"),
    ))
}

/// "Until your next turn, spells ... cost {2} more to cast." is also "Spells ... cost {2} more to cast until your next
/// turn.": the duration moved to the end of the cost sentence.
pub fn g10d_spell_cost_wordings(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter_map(|text| {
            let (start, upper) = ["Until your next turn, ", "until your next turn, "]
                .iter()
                .find_map(|lead| text.find(lead).map(|at| (at, lead.starts_with('U'))))?;
            let rest = &text[start + "until your next turn, ".len()..];
            let end = rest.find(" to cast")? + " to cast".len();
            let clause = &rest[..end];
            clause.contains(" cost {").then_some(())?;
            let mut chars = clause.chars();
            let first = chars.next()?;
            let first = if upper {
                first.to_uppercase().collect::<String>()
            } else {
                first.to_string()
            };
            Some(format!(
                "{}{first}{} until your next turn{}",
                &text[..start],
                chars.as_str(),
                &rest[end..]
            ))
        })
        .collect()
}

/// The object and the duration of "<object> becomes the color or colors of your choice": your choice of one or more
/// colours, then a `Continuous` effect that sets the object's colours to that choice and nothing else.
pub fn g10d_colors_parts<'a>(
    choose: &'a Effect,
    change: &'a Effect,
) -> Option<(&'a Ref, &'a Duration)> {
    let (
        Effect::ChooseValue {
            who: Ref::You,
            kind: ValueKind::Colors,
            bind,
        },
        Effect::Continuous {
            affected,
            mods,
            duration,
        },
    ) = (choose, change)
    else {
        return None;
    };
    matches!(mods.as_slice(), [Modification::SetColorsChosen(name)] if name == bind)
        .then_some((affected, duration))
}

/// "target creature becomes the color or colors of your choice until end of turn"; one that lasts is written without a
/// duration.
pub fn g10d_colors_text(subject: &str, duration: &Duration) -> Option<String> {
    let lasting = match duration {
        Duration::UntilEndOfTurn => " until end of turn",
        Duration::Forever => "",
        _ => return None,
    };
    Some(format!(
        "{subject} becomes the color or colors of your choice{lasting}"
    ))
}

/// What [`g10d_spell_cost`] reads, written: the first wording of the duration.
pub fn g10d_spell_cost_text(effect: &Effect) -> Option<String> {
    let Effect::Rule {
        rule:
            GameRule::SpellCost {
                applies_to,
                change,
                mana,
                per,
            },
        duration,
    } = effect
    else {
        return None;
    };
    if *applies_to == Selector::this() || mana.has_x() {
        return None;
    }
    let (applies_to, colors) = match r4703_that_are_text(applies_to) {
        Some((rest, colors)) => (rest, colors),
        None => (applies_to.clone(), String::new()),
    };
    let subject = cost_subject_text(&applies_to)?;
    let more = match change {
        CostChange::Increase => "more",
        CostChange::Reduce => "less",
    };
    if !per.is_one() {
        // A count is only said as X of a cost change for this turn: "cost {X} less to cast, where X is ...".
        let wordings = super::c11a_turn_costs::c11a_turn_wordings(mana, per)?;
        let (mana, tail) = wordings.into_iter().find(|(mana, _)| mana == "{X}")?;
        return matches!(duration, Duration::UntilEndOfTurn)
            .then(|| format!("{subject} this turn{colors} cost {mana} {more} to cast{tail}"));
    }
    if !colors.is_empty() {
        return matches!(duration, Duration::UntilEndOfTurn)
            .then(|| format!("{subject} this turn{colors} cost {mana} {more} to cast"));
    }
    match duration {
        Duration::UntilEndOfTurn => Some(format!("{subject} this turn cost {mana} {more} to cast")),
        Duration::UntilYourNextTurn => Some(format!(
            "until your next turn, {subject} cost {mana} {more} to cast"
        )),
        _ => None,
    }
}

#[cfg(test)]
mod r4703_tests {
    //! Goal: a cost change for the turn that counts life lost or gained reads and writes back.
    use super::*;
    use crate::compile::read::tokens::{same_text, tokenize};
    use rstest::rstest;

    #[rstest]
    #[case(
        "spells you cast this turn that are black and/or red cost {X} less to cast, where X is the amount of life you lost this turn"
    )]
    #[case(
        "spells you cast this turn that are white and/or blue cost {X} less to cast, where X is the amount of life you gained this turn"
    )]
    fn r4703_counted_spell_costs_read_back(#[case] text: &str) {
        let tokens = tokenize(text);
        let mut r = Reader::new(&tokens, false);
        let effects = g10d_spell_cost(&mut r).expect("read");
        assert!(r.end(), "{text}");
        let written = g10d_spell_cost_text(&effects[0]).expect("written");
        assert!(same_text(&written, text), "{written}");
    }
}
