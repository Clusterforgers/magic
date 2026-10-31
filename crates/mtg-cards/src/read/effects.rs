//! Effects as steps: the common ones in a sentence, the ones that hold other effects as an indented block, and every
//! other effect by its variant and fields.

use super::generic;
use super::parts;
use super::phrase::{self, Ctx};
use super::{Step, steps};
use crate::card::TokenSpec;
use crate::effect::{Destination, Effect, Mode, Order, Place, TokenRef};
use crate::refs::{Bounds, Ref, Selector};
use crate::value::Amount;

/// Adds the steps of one effect at a depth.
pub(super) fn push(ctx: &Ctx, effect: &Effect, depth: u8, out: &mut Vec<Step>) {
    if block(ctx, effect, depth, out) {
        return;
    }
    let line = cards(ctx, effect)
        .or_else(|| permanents(ctx, effect))
        .or_else(|| counters(ctx, effect))
        .or_else(|| game(ctx, effect));
    match line {
        Some(text) => out.push(Step::new(depth, text)),
        None => generic::effect(ctx, effect, depth, out),
    }
}

/// Adds the steps of a list of effects.
pub(super) fn list(ctx: &Ctx, effects: &[Effect], depth: u8, out: &mut Vec<Step>) {
    for effect in effects {
        push(ctx, effect, depth, out);
    }
}

/// A player's action: "draw 2 cards" for you, "each opponent draws 2 cards" for anyone else.
fn act(ctx: &Ctx, player: &Ref, verb: &str, rest: &str) -> String {
    let tail = if rest.is_empty() {
        String::new()
    } else {
        format!(" {rest}")
    };
    if player.is_you() {
        format!("{verb}{tail}")
    } else {
        phrase::verb(ctx, player, verb, rest)
    }
}

/// "one" for the article of a count: "choose one of", not "choose a of".
fn one_or(count: String) -> String {
    if count == "a" {
        "one".to_owned()
    } else {
        count
    }
}

/// "any number of", "up to 2", "a": how many a choice allows.
fn bounds(ctx: &Ctx, bounds: &Bounds) -> String {
    match (&bounds.min, &bounds.max) {
        (Amount::Fixed(1), Some(Amount::Fixed(1))) => "a".to_owned(),
        (Amount::Fixed(0), Some(most)) => format!("up to {}", phrase::amount(ctx, most)),
        (Amount::Fixed(0), None) => "any number of".to_owned(),
        (least, Some(most)) if least == most => phrase::amount(ctx, least),
        (least, Some(most)) => format!(
            "{} to {}",
            phrase::amount(ctx, least),
            phrase::amount(ctx, most)
        ),
        (least, None) => format!("at least {}", phrase::amount(ctx, least)),
    }
}

/// The effects about cards in hands and libraries, and about life and damage.
fn cards(ctx: &Ctx, effect: &Effect) -> Option<String> {
    Some(match effect {
        Effect::Draw { who, count } => act(ctx, who, "draw", &phrase::counted(ctx, count, "card")),
        Effect::Discard {
            who, count, random, ..
        } => {
            let at_random = if *random { " at random" } else { "" };
            act(
                ctx,
                who,
                "discard",
                &format!("{}{at_random}", phrase::counted(ctx, count, "card")),
            )
        }
        Effect::Mill { who, count } => act(ctx, who, "mill", &phrase::counted(ctx, count, "card")),
        Effect::Scry { who, count } => act(ctx, who, "scry", &phrase::amount(ctx, count)),
        Effect::Surveil { who, count } => act(ctx, who, "surveil", &phrase::amount(ctx, count)),
        Effect::GainLife { who, amount } => act(
            ctx,
            who,
            "gain",
            &format!("{} life", phrase::amount(ctx, amount)),
        ),
        Effect::LoseLife { who, amount } => act(
            ctx,
            who,
            "lose",
            &format!("{} life", phrase::amount(ctx, amount)),
        ),
        Effect::DealDamage { source, to, amount } => {
            let damage = format!(
                "{} damage to {}",
                phrase::amount(ctx, amount),
                phrase::who(ctx, to)
            );
            match source {
                Some(source) => format!("{} deals {damage}", phrase::who(ctx, source)),
                None => format!("deal {damage}"),
            }
        }
        Effect::Reveal { what } => format!("reveal {}", phrase::who(ctx, what)),
        Effect::Shuffle { who } => act(
            ctx,
            who,
            "shuffle",
            &format!("{} library", phrase::whose(ctx, who)),
        ),
        Effect::LookAtTop { who, count, .. } => {
            let top = if *count == Amount::one() {
                "top card".to_owned()
            } else {
                format!("top {} cards", phrase::amount(ctx, count))
            };
            act(
                ctx,
                who,
                "look",
                &format!("at the {top} of {} library", phrase::whose(ctx, who)),
            )
        }
        Effect::Search {
            who,
            zone,
            filter,
            count,
            reveal,
            ..
        } => act(
            ctx,
            who,
            "search",
            &format!(
                "{} {} for {} {}{}",
                phrase::whose(ctx, who),
                phrase::zone_text(*zone).trim_start_matches("a "),
                bounds(ctx, count),
                found_cards(ctx, filter, count),
                match (*reveal, bounds(ctx, count) == "a") {
                    (false, _) => "",
                    (true, true) => " and reveal it",
                    (true, false) => " and reveal them",
                }
            ),
        ),
        Effect::Choose {
            who, from, count, ..
        } => act(
            ctx,
            who,
            "choose",
            &format!(
                "{} of {}",
                one_or(bounds(ctx, count)),
                phrase::who(ctx, from)
            ),
        ),
        _ => return None,
    })
}

/// "basic land cards" or "a creature card": what a search finds, in the plural when it may find more than one.
fn found_cards(ctx: &Ctx, filter: &Selector, count: &Bounds) -> String {
    let noun = if filter.0.is_empty() {
        "card".to_owned()
    } else {
        format!("{} card", phrase::selector(ctx, filter))
    };
    if bounds(ctx, count) == "a" {
        noun
    } else {
        format!("{noun}s")
    }
}

/// "you may pay {2}", "you may discard a card": a player who may pay a cost.
fn may_pay(ctx: &Ctx, who: &Ref, cost: &str) -> String {
    let who = phrase::who(ctx, who);
    if cost.starts_with('{') {
        format!("{who} may pay {cost}")
    } else {
        format!("{who} may {cost}")
    }
}

/// "1 token", "X tokens" or "tokens equal to the number of ...": how many tokens.
fn tokens(ctx: &Ctx, count: &Amount) -> String {
    match count {
        Amount::Fixed(1) => "1 token".to_owned(),
        other => phrase::counted(ctx, other, "token"),
    }
}

/// What a sacrifice takes: "this permanent", "a creature", "2 creatures".
fn sacrificed(ctx: &Ctx, count: &Amount, filter: &Selector) -> String {
    let what = phrase::selector(ctx, filter);
    if what.starts_with("this ") {
        what
    } else {
        format!("{} {what}", bounds_of_count(ctx, count))
    }
}

/// "tapped under your control": how a permanent enters.
fn entering(ctx: &Ctx, tapped: bool, under: Option<&Ref>, transformed: bool) -> String {
    let under = under.map(|player| format!(" under {} control", phrase::whose(ctx, player)));
    format!(
        "{}{}{}",
        if tapped { " tapped" } else { "" },
        if transformed { " transformed" } else { "" },
        under.unwrap_or_default()
    )
}

/// A move of objects between zones.
fn moved(ctx: &Ctx, what: &Ref, to: &Destination) -> String {
    let what = phrase::who(ctx, what);
    match to {
        Destination::Hand => format!("return {what} to its owner's hand"),
        Destination::Graveyard => format!("put {what} into its owner's graveyard"),
        Destination::Battlefield {
            tapped,
            under,
            transformed,
            counters,
        } => {
            let with: Vec<String> = counters
                .iter()
                .map(|(kind, count)| format!("{count} {} counter", phrase::counter_text(kind)))
                .collect();
            let with = if with.is_empty() {
                String::new()
            } else {
                format!(" with {}", with.join(", "))
            };
            format!(
                "put {what} onto the battlefield{}{with}",
                entering(ctx, *tapped, under.as_ref(), *transformed)
            )
        }
        Destination::Library { place, order } => {
            let place = match place {
                Place::Top => "on top of",
                Place::Bottom => "on the bottom of",
                Place::Shuffled => "into",
            };
            let order = match order {
                Some(Order::Random) => " in a random order",
                Some(Order::Chosen) => " in any order",
                None => "",
            };
            format!("put {what} {place} its owner's library{order}")
        }
        Destination::BattlefieldAttached { to } => {
            format!(
                "put {what} onto the battlefield attached to {}",
                phrase::who(ctx, to)
            )
        }
    }
}

/// The effects on permanents, spells and the cards they move.
fn permanents(ctx: &Ctx, effect: &Effect) -> Option<String> {
    Some(match effect {
        Effect::Move { what, to } => moved(ctx, what, to),
        Effect::Destroy { what, regenerate } => format!(
            "destroy {}{}",
            phrase::who(ctx, what),
            if *regenerate {
                ""
            } else {
                " (it can't be regenerated)"
            }
        ),
        Effect::Exile { what, duration, .. } => format!(
            "exile {}{}",
            phrase::who(ctx, what),
            duration
                .as_ref()
                .map_or_else(String::new, |until| format!(" {}", phrase::duration(until)))
        ),
        Effect::Sacrifice { who, filter, count } => {
            act(ctx, who, "sacrifice", &sacrificed(ctx, count, filter))
        }
        Effect::Counter { what } => format!("counter {}", phrase::who(ctx, what)),
        Effect::Tap { what, .. } => format!("tap {}", phrase::who(ctx, what)),
        Effect::Untap { what } => format!("untap {}", phrase::who(ctx, what)),
        Effect::Attach { what, to } => format!(
            "attach {} to {}",
            phrase::who(ctx, what),
            phrase::who(ctx, to)
        ),
        Effect::Transform { what } => format!("transform {}", phrase::who(ctx, what)),
        Effect::Fight { what, with } => format!(
            "{} fights {}",
            phrase::who(ctx, what),
            phrase::who(ctx, with)
        ),
        Effect::Goad { what } => format!("goad {}", phrase::who(ctx, what)),
        Effect::Regenerate { what } => format!("regenerate {}", phrase::who(ctx, what)),
        Effect::Explore { what } => format!("{} explores", phrase::who(ctx, what)),
        _ => return None,
    })
}

/// "a" for one, otherwise the amount.
fn bounds_of_count(ctx: &Ctx, count: &Amount) -> String {
    match count {
        Amount::Fixed(1) => "a".to_owned(),
        other => phrase::amount(ctx, other),
    }
}

/// The effects on counters, tokens and mana.
fn counters(ctx: &Ctx, effect: &Effect) -> Option<String> {
    Some(match effect {
        Effect::PutCounters { on, counter, count } => format!(
            "put {} on {}",
            phrase::counted(
                ctx,
                count,
                &format!("{} counter", phrase::counter_text(counter))
            ),
            phrase::who(ctx, on)
        ),
        Effect::RemoveCounters {
            from,
            counter,
            count,
            up_to,
        } => format!(
            "remove {}{} {} counters from {}",
            if *up_to { "up to " } else { "" },
            phrase::amount(ctx, count),
            counter
                .as_ref()
                .map_or_else(|| "any".to_owned(), phrase::counter_text),
            phrase::who(ctx, from)
        ),
        Effect::MoveCounters {
            from,
            to,
            counter,
            count,
        } => format!(
            "move {} {} counters from {} to {}",
            phrase::amount(ctx, count),
            phrase::counter_text(counter),
            phrase::who(ctx, from),
            phrase::who(ctx, to)
        ),
        Effect::AddMana {
            who, mana, times, ..
        } => {
            let times = if *times == Amount::one() {
                String::new()
            } else {
                format!(", {} times", phrase::amount(ctx, times))
            };
            act(
                ctx,
                who,
                "add",
                &format!("{}{times}", parts::mana(ctx, mana)),
            )
        }
        Effect::CreateToken {
            who,
            token,
            count,
            tapped,
            attacking,
            ..
        } => {
            let how = format!(
                "{}{}",
                if *tapped { ", tapped" } else { "" },
                if *attacking { ", attacking" } else { "" }
            );
            act(
                ctx,
                who,
                "create",
                &format!("{}: {}{how}", tokens(ctx, count), token_text(ctx, token)),
            )
        }
        _ => return None,
    })
}

/// The effects of the game itself: turns, the game's end, and other players' status.
fn game(ctx: &Ctx, effect: &Effect) -> Option<String> {
    Some(match effect {
        Effect::WinGame { who } => act(ctx, who, "win", "the game"),
        Effect::ExtraTurn { who, count, .. } => act(
            ctx,
            who,
            "take",
            &format!("{} extra", phrase::counted(ctx, count, "turn")),
        ),
        Effect::BecomeMonarch { who } => act(ctx, who, "become", "the monarch"),
        Effect::Proliferate { who } => act(ctx, who, "proliferate", ""),
        Effect::Rule { rule, duration } => {
            format!("{} {}", parts::rule(ctx, rule), phrase::duration(duration))
        }
        _ => return None,
    })
}

/// A token as its characteristics: "4/4 white Angel creature, flying".
fn token_text(ctx: &Ctx, token: &TokenRef) -> String {
    match token {
        TokenRef::Named(name) => name.clone(),
        TokenRef::Custom(spec) => spec_text(ctx, spec),
        TokenRef::Sized { size, token } => {
            let size = phrase::amount(ctx, size);
            format!("{size}/{size} {}", spec_text(ctx, token))
        }
        TokenRef::CopyOf {
            of, no_mana_cost, ..
        } => format!(
            "a copy of {}{}",
            phrase::who(ctx, of),
            if *no_mana_cost {
                " with no mana cost"
            } else {
                ""
            }
        ),
    }
}

/// The characteristics of a token and its keywords.
fn spec_text(ctx: &Ctx, spec: &TokenSpec) -> String {
    let mut words = Vec::new();
    if let (Some(power), Some(toughness)) = (spec.power, spec.toughness) {
        words.push(format!("{power}/{toughness}"));
    }
    words.extend(
        spec.colors
            .iter()
            .map(|color| generic::words(&format!("{color:?}"))),
    );
    words.push(spec.name.clone());
    words.extend(
        spec.types
            .iter()
            .map(|kind| generic::words(&format!("{kind:?}"))),
    );
    let keywords: Vec<String> = spec
        .abilities
        .iter()
        .map(|ability| match ability {
            crate::ability::Ability::Keyword(keyword) => parts::keyword(ctx, keyword),
            other => format!("an ability ({})", steps::label(other)),
        })
        .collect();
    let abilities = if keywords.is_empty() {
        String::new()
    } else {
        format!(", {}", keywords.join(", "))
    };
    format!("{}{abilities}", words.join(" "))
}

/// A mode of a modal ability: its label, then its targets and effects.
fn mode(number: usize, mode: &Mode, depth: u8, out: &mut Vec<Step>) {
    let inner = Ctx::new(&mode.targets);
    let label = mode.label.as_ref().map_or_else(
        || format!("mode {number}:"),
        |label| format!("mode {number}, \u{201c}{label}\u{201d}:"),
    );
    out.push(Step::new(depth, label));
    steps::targets(&inner, depth + 1, out);
    list(&inner, &mode.effects, depth + 1, out);
}

/// The effects that hold other effects: a block of steps under a line that says when or for whom they run.
fn block(ctx: &Ctx, effect: &Effect, depth: u8, out: &mut Vec<Step>) -> bool {
    match effect {
        Effect::If {
            condition,
            then,
            otherwise,
        } => {
            out.push(Step::new(
                depth,
                format!("if {}:", phrase::condition(ctx, condition)),
            ));
            list(ctx, then, depth + 1, out);
            if !otherwise.is_empty() {
                out.push(Step::new(depth, "otherwise:"));
                list(ctx, otherwise, depth + 1, out);
            }
        }
        Effect::May { who, effects } => {
            out.push(Step::new(depth, format!("{} may:", phrase::who(ctx, who))));
            list(ctx, effects, depth + 1, out);
        }
        Effect::Pay {
            who,
            cost,
            times,
            paid,
            unpaid,
        } => {
            let times = if *times == Amount::one() {
                String::new()
            } else {
                format!(" {} times", phrase::amount(ctx, times))
            };
            out.push(Step::new(
                depth,
                format!("{}{times}:", may_pay(ctx, who, &parts::cost(ctx, cost))),
            ));
            if !paid.is_empty() {
                out.push(Step::new(depth + 1, "if paid:"));
                list(ctx, paid, depth + 2, out);
            }
            if !unpaid.is_empty() {
                out.push(Step::new(depth + 1, "if not paid:"));
                list(ctx, unpaid, depth + 2, out);
            }
        }
        Effect::Modal {
            choose,
            repeat,
            modes,
            ..
        } => {
            let how = bounds(ctx, choose);
            let how = if how == "a" { "one".to_owned() } else { how };
            let again = if *repeat { ", a mode may repeat" } else { "" };
            out.push(Step::new(
                depth,
                format!(
                    "choose {how} mode{}{again}:",
                    if how == "one" { "" } else { "s" }
                ),
            ));
            for (at, each) in modes.iter().enumerate() {
                mode(at + 1, each, depth + 1, out);
            }
        }
        Effect::Either { first, second } => {
            out.push(Step::new(depth, "choose one when this resolves:"));
            list(ctx, first, depth + 1, out);
            out.push(Step::new(depth + 1, "or:"));
            list(ctx, second, depth + 1, out);
        }
        _ => return repeated(ctx, effect, depth, out),
    }
    true
}

/// The blocks that repeat, delay or last: for each, a later trigger, a continuous effect.
fn repeated(ctx: &Ctx, effect: &Effect, depth: u8, out: &mut Vec<Step>) -> bool {
    match effect {
        Effect::RepeatEach { over, effects, .. } => {
            out.push(Step::new(
                depth,
                format!(
                    "for each {}:",
                    phrase::who(ctx, over).trim_start_matches("each ")
                ),
            ));
            list(ctx, effects, depth + 1, out);
        }
        Effect::DelayedTrigger {
            when,
            effects,
            once,
            this_turn,
        } => {
            let scope = if *this_turn { " this turn" } else { "" };
            let times = if *once { "the next time" } else { "each time" };
            out.push(Step::new(
                depth,
                format!("{times}{scope}, {}:", parts::trigger(ctx, when)),
            ));
            list(ctx, effects, depth + 1, out);
        }
        Effect::Continuous {
            affected,
            mods,
            duration,
        } => {
            out.push(Step::new(
                depth,
                format!(
                    "{}, {}:",
                    phrase::who(ctx, affected),
                    phrase::duration(duration)
                ),
            ));
            parts::modifications(ctx, mods, depth + 1, out);
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    //! Goal: the common effects read as one English line each, the effects that hold others as an indented block, and an effect no arm words still shows its variant and fields.
    use rstest::rstest;

    use super::*;

    fn lines(json: &str) -> Vec<String> {
        let effect: Effect = serde_json::from_str(json).unwrap();
        let mut out = Vec::new();
        push(&Ctx::none(), &effect, 0, &mut out);
        out.iter()
            .map(|step| format!("{}{}", "  ".repeat(usize::from(step.depth)), step.text))
            .collect()
    }

    #[rstest]
    #[case::draw(r#"{"Draw":{}}"#, "draw a card")]
    #[case::draw_two_for_them(
        r#"{"Draw":{"who":"Opponents","count":2}}"#,
        "each opponent draws 2 cards"
    )]
    #[case::damage(
        r#"{"DealDamage":{"to":"This","amount":3}}"#,
        "deal 3 damage to this permanent"
    )]
    #[case::life(r#"{"GainLife":{"amount":"X"}}"#, "gain X life")]
    #[case::destroy(
        r#"{"Destroy":{"what":"This","regenerate":false}}"#,
        "destroy this permanent (it can't be regenerated)"
    )]
    #[case::scry(r#"{"Scry":{"count":2}}"#, "scry 2")]
    #[case::mana(r#"{"AddMana":{"mana":{"Fixed":["Red","Red"]}}}"#, "add {R}{R}")]
    #[case::token(
        r#"{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"power":4,"toughness":4,"abilities":[{"Keyword":"Flying"}]}}}}"#,
        "create 1 token: 4/4 white Angel creature, flying"
    )]
    #[case::counters(
        r#"{"PutCounters":{"on":"This","counter":"+1/+1","count":2}}"#,
        "put 2 +1/+1 counters on this permanent"
    )]
    #[case::counted_by_what_it_counts(
        r#"{"Draw":{"count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#,
        "draw cards equal to the number of creatures you control"
    )]
    #[case::sacrifice_itself(r#"{"Sacrifice":{"filter":["This"]}}"#, "sacrifice this permanent")]
    #[case::tokens_by_x(
        r#"{"CreateToken":{"token":{"Named":"Treasure"},"count":"X"}}"#,
        "create X tokens: Treasure"
    )]
    #[case::search_a_card(
        r#"{"Search":{"filter":[{"Type":"Creature"}],"as":"found","reveal":true}}"#,
        "search your library for a creature card and reveal it"
    )]
    #[case::search_cards(
        r#"{"Search":{"filter":[{"Type":"Land"},{"Supertype":"Basic"}],"count":{"min":0,"max":2},"as":"found"}}"#,
        "search your library for up to 2 basic land cards"
    )]
    #[case::unworded(r#"{"Venture":{"who":"You","undercity":true}}"#, "venture: undercity")]
    fn an_effect_is_one_line(#[case] json: &str, #[case] shown: &str) {
        assert_eq!(lines(json), vec![shown.to_owned()]);
    }

    #[test]
    fn a_condition_and_its_effects_are_a_block() {
        let shown = lines(
            r#"{"If":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":40}},"then":[{"Draw":{}}],"otherwise":[{"Scry":{"count":1}}]}}"#,
        );
        assert_eq!(
            shown,
            [
                "if your life total is at least 40:",
                "  draw a card",
                "otherwise:",
                "  scry 1"
            ]
        );
    }

    #[test]
    fn a_modal_ability_lists_its_modes() {
        let shown = lines(
            r#"{"Modal":{"choose":{"min":1,"max":1},"modes":[{"label":"Disarm","effects":[{"Draw":{}}]},{"effects":[{"Scry":{"count":1}}]}]}}"#,
        );
        assert_eq!(shown[0], "choose one mode:");
    }
}
