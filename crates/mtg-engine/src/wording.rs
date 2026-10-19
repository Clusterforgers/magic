//! Words for what the rules data says, for the prompts of players: a target specification ("up to two target
//! creatures you control") and the event a triggered ability waits for ("When it enters the battlefield").
//!
//! They are hints for a person choosing, not rules: a predicate with no short phrase is left out.

use mtg_cards::refs::{Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::{Amount, Cmp, StatKind};

/// "a" or "an" ahead of `word`.
fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    }
}

fn lower(debug: &impl std::fmt::Debug) -> String {
    format!("{debug:?}").to_lowercase()
}

fn stat_word(stat: StatKind) -> &'static str {
    match stat {
        StatKind::Power => "power",
        StatKind::Toughness => "toughness",
        StatKind::ManaValue => "mana value",
        StatKind::Loyalty => "loyalty",
    }
}

fn comparison(stat: StatKind, op: Cmp, to: i32) -> String {
    let stat = stat_word(stat);
    match op {
        Cmp::Eq => format!("with {stat} {to}"),
        Cmp::Ne => format!("with {stat} other than {to}"),
        Cmp::Lt => format!("with {stat} less than {to}"),
        Cmp::Le => format!("with {stat} {to} or less"),
        Cmp::Gt => format!("with {stat} greater than {to}"),
        Cmp::Ge => format!("with {stat} {to} or greater"),
    }
}

fn zone_phrase(kind: ZoneKind, yours: bool) -> String {
    let (place, owner) = match kind {
        ZoneKind::Battlefield => return "on the battlefield".to_owned(),
        ZoneKind::Stack => return "on the stack".to_owned(),
        ZoneKind::Exile => return "in exile".to_owned(),
        ZoneKind::Hand => ("hand", "a"),
        ZoneKind::Library => ("library", "a"),
        ZoneKind::Graveyard => ("graveyard", "a"),
        ZoneKind::Command => ("command zone", "a"),
    };
    format!("in {} {place}", if yours { "your" } else { owner })
}

/// A noun phrase for what a selector matches, split into the part that is pluralised and the part after it:
/// ("tapped creature", "you control").
fn phrase(selector: &Selector) -> (String, String) {
    let (mut adjectives, mut nouns, mut tail) = (Vec::new(), Vec::new(), Vec::new());
    let (mut zone, mut yours, mut this) = (None, false, false);
    for pred in &selector.0 {
        match pred {
            Pred::This => this = true,
            Pred::Other => adjectives.push("other".to_owned()),
            Pred::Type(card_type) => nouns.push(lower(card_type)),
            Pred::Subtype(subtype) => nouns.push(subtype.0.clone()),
            Pred::Supertype(supertype) => adjectives.push(lower(supertype)),
            Pred::Color(color) => adjectives.push(lower(color)),
            Pred::Colorless => adjectives.push("colorless".to_owned()),
            Pred::Token => adjectives.push("token".to_owned()),
            Pred::Spell => nouns.push("spell".to_owned()),
            Pred::Tapped => adjectives.push("tapped".to_owned()),
            Pred::Untapped => adjectives.push("untapped".to_owned()),
            Pred::Attacking => adjectives.push("attacking".to_owned()),
            Pred::Blocking => adjectives.push("blocking".to_owned()),
            Pred::ControlledBy(Ref::You) => tail.push("you control".to_owned()),
            Pred::ControlledBy(Ref::Opponents) => tail.push("an opponent controls".to_owned()),
            Pred::OwnedBy(Ref::You) => yours = true,
            Pred::OwnedBy(Ref::Opponents) => tail.push("an opponent owns".to_owned()),
            Pred::In(kind) => zone = Some(*kind),
            Pred::Named(name) => nouns.push(format!("card named {name}")),
            Pred::HasKeyword(keyword) => {
                tail.push(format!("with {}", keyword.name().to_lowercase()));
            }
            Pred::Compare {
                stat,
                op,
                to: Amount::Fixed(to),
            } => tail.push(comparison(*stat, *op, *to)),
            Pred::AnyOf(any) if any.iter().all(|p| matches!(p, Pred::Type(_))) => {
                let words: Vec<String> = any
                    .iter()
                    .filter_map(|p| match p {
                        Pred::Type(card_type) => Some(lower(card_type)),
                        _ => None,
                    })
                    .collect();
                nouns.push(words.join(" or "));
            }
            _ => {}
        }
    }
    match zone {
        Some(kind) => tail.push(zone_phrase(kind, yours)),
        None if yours => tail.push("you own".to_owned()),
        None => {}
    }
    let noun = if this {
        "permanent".to_owned()
    } else if nouns.is_empty() {
        if zone.is_some() { "card" } else { "permanent" }.to_owned()
    } else {
        nouns.join(" ")
    };
    adjectives.push(noun);
    (adjectives.join(" "), tail.join(" "))
}

fn join(head: &str, tail: &str) -> String {
    if tail.is_empty() {
        head.to_owned()
    } else {
        format!("{head} {tail}")
    }
}

/// What a selector matches as a noun phrase: "creature you control".
fn noun(selector: &Selector) -> String {
    let (head, tail) = phrase(selector);
    join(&head, &tail)
}

/// A selector as the subject of a clause: "it" for the source itself, else "a creature you control".
fn subject(selector: &Selector) -> String {
    if selector.0 == [Pred::This] {
        return "it".to_owned();
    }
    let noun = noun(selector);
    format!("{} {noun}", article(&noun))
}

/// Who, as the subject of a verb that takes an "s" after a third person: ("you", "") or ("an opponent", "s").
fn who(by: &Ref) -> (&'static str, &'static str) {
    match by {
        Ref::You => ("you", ""),
        Ref::Opponents => ("an opponent", "s"),
        _ => ("a player", "s"),
    }
}

fn kind_of(what: &Targetable) -> (String, String) {
    match what {
        Targetable::AnyTarget => ("target".to_owned(), String::new()),
        Targetable::Player => ("player".to_owned(), String::new()),
        Targetable::Opponent => ("opponent".to_owned(), String::new()),
        Targetable::Object(selector) => phrase(selector),
        Targetable::Either(kinds) => (
            kinds
                .iter()
                .map(|kind| join(&kind_of(kind).0, &kind_of(kind).1))
                .collect::<Vec<_>>()
                .join(" or "),
            String::new(),
        ),
    }
}

/// A target specification in the words of a card: "target creature you control", "up to two target players",
/// "any target".
pub(crate) fn spec(spec: &TargetSpec) -> String {
    let (head, tail) = kind_of(&spec.what);
    let any = matches!(spec.what, Targetable::AnyTarget);
    let one = |head: &str| {
        if any {
            "any target".to_owned()
        } else {
            join(&format!("target {head}"), &tail)
        }
    };
    let several = |head: &str| {
        if any {
            "targets".to_owned()
        } else if head.contains(" or ") {
            join(&format!("target {head}"), &tail)
        } else {
            join(&format!("target {head}s"), &tail)
        }
    };
    let (min, max) = (spec.bounds.min, &spec.bounds.max);
    match max {
        Some(Amount::Fixed(1)) if min == 1 => one(&head),
        Some(Amount::Fixed(1)) => {
            format!("up to one {}", one(&head).replace("any target", "target"))
        }
        Some(Amount::Fixed(n)) if i64::from(min) == i64::from(*n) => {
            format!("{n} {}", several(&head))
        }
        Some(Amount::Fixed(n)) if min == 0 => format!("up to {n} {}", several(&head)),
        Some(Amount::Fixed(n)) => format!("{min} to {n} {}", several(&head)),
        _ if min == 0 => format!("any number of {}", several(&head)),
        _ => format!("{min} or more {}", several(&head)),
    }
}

fn step_words(step: Step) -> &'static str {
    match step {
        Step::Untap => "untap step",
        Step::Upkeep => "upkeep",
        Step::Draw => "draw step",
        Step::Main1 => "precombat main phase",
        Step::BeginCombat => "combat",
        Step::DeclareAttackers => "declare attackers step",
        Step::DeclareBlockers => "declare blockers step",
        Step::FirstStrikeDamage => "first-strike damage step",
        Step::CombatDamage => "combat damage step",
        Step::EndCombat => "end of combat step",
        Step::Main2 => "postcombat main phase",
        Step::End => "end step",
        Step::Cleanup => "cleanup step",
    }
}

/// The clause of one pattern, such as "it enters the battlefield", and whether it speaks of the source itself.
fn clause(pattern: &TriggerPattern) -> (String, bool) {
    match pattern {
        TriggerPattern::ZoneChange { what, from, to } => {
            let moves = match (from, to) {
                (_, Some(ZoneKind::Battlefield)) => "enters the battlefield".to_owned(),
                (Some(ZoneKind::Battlefield), Some(ZoneKind::Graveyard)) => "dies".to_owned(),
                (Some(ZoneKind::Battlefield), None) => "leaves the battlefield".to_owned(),
                (None, Some(ZoneKind::Graveyard)) => "is put into a graveyard".to_owned(),
                (Some(ZoneKind::Library), Some(ZoneKind::Hand)) => "is drawn".to_owned(),
                (Some(from), Some(to)) => format!(
                    "moves from {} to {}",
                    zone_phrase(*from, false).trim_start_matches("in "),
                    zone_phrase(*to, false).trim_start_matches("in ")
                ),
                _ => "changes zones".to_owned(),
            };
            (format!("{} {moves}", subject(what)), what.0 == [Pred::This])
        }
        TriggerPattern::Attacks { what } => {
            (format!("{} attacks", subject(what)), what.0 == [Pred::This])
        }
        TriggerPattern::BecomesTarget { what, .. } => (
            format!("{} becomes the target of a spell or ability", subject(what)),
            what.0 == [Pred::This],
        ),
        TriggerPattern::SpellCast { what, by } => {
            let (who, s) = who(by);
            let spell = if what.is_empty() {
                "a spell".to_owned()
            } else {
                let noun = noun(what);
                format!("{} {noun} spell", article(&noun))
            };
            (format!("{who} cast{s} {spell}"), false)
        }
        TriggerPattern::Sacrificed { what, by } => {
            let (who, s) = who(by);
            (format!("{who} sacrifice{s} {}", subject(what)), false)
        }
        TriggerPattern::StepBegins { step, whose } => {
            let owner = match whose {
                Ref::You => "your",
                Ref::Opponents => "an opponent's",
                _ => "each",
            };
            let when = if *step == Step::BeginCombat {
                format!("the beginning of combat on {owner} turn").replace("each turn", "each turn")
            } else {
                format!("the beginning of {owner} {}", step_words(*step))
            };
            (when, false)
        }
        TriggerPattern::AnyOf(patterns) => {
            let clauses: Vec<(String, bool)> = patterns.iter().map(clause).collect();
            (
                clauses
                    .iter()
                    .map(|(text, _)| text.as_str())
                    .collect::<Vec<_>>()
                    .join(" or "),
                clauses.iter().all(|(_, this)| *this),
            )
        }
    }
}

/// The event a triggered ability waits for, as its card would say it: "When it enters the battlefield", "Whenever
/// an opponent casts a spell", "At the beginning of your upkeep".
pub(crate) fn pattern(pattern: &TriggerPattern) -> String {
    let (text, itself) = clause(pattern);
    if text.starts_with("the beginning of") {
        return format!("At {text}");
    }
    format!("{} {text}", if itself { "When" } else { "Whenever" })
}

#[cfg(test)]
mod tests {
    //! Goal: target specifications and trigger patterns read as the cards say them, for the shapes the corpus and the prompts use, and a predicate with no phrase is left out instead of breaking the sentence.
    use rstest::rstest;

    use super::*;

    fn target(json: &str) -> String {
        spec(&serde_json::from_str(json).unwrap())
    }

    fn words(json: &str) -> String {
        pattern(&serde_json::from_str(json).unwrap())
    }

    #[rstest]
    #[case::any_target(r#"{"what":"AnyTarget"}"#, "any target")]
    #[case::a_player(r#"{"what":"Player"}"#, "target player")]
    #[case::an_opponent(r#"{"what":"Opponent"}"#, "target opponent")]
    #[case::a_creature(r#"{"what":{"Object":[{"Type":"Creature"}]}}"#, "target creature")]
    #[case::yours(
        r#"{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
        "target creature you control"
    )]
    #[case::a_card_in_your_graveyard(
        r#"{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}"#,
        "target card in your graveyard"
    )]
    #[case::coloured_and_tapped(
        r#"{"what":{"Object":[{"Color":"White"},{"Tapped":null},{"Type":"Creature"}]}}"#,
        "target white tapped creature"
    )]
    #[case::small(
        r#"{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}]}}"#,
        "target creature with power 2 or less"
    )]
    #[case::two_kinds(
        r#"{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]}}"#,
        "target creature or planeswalker"
    )]
    #[case::up_to_one(
        r#"{"what":"Player","bounds":{"min":0,"max":1}}"#,
        "up to one target player"
    )]
    #[case::up_to_two(
        r#"{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":2}}"#,
        "up to 2 target creatures"
    )]
    #[case::exactly_two(r#"{"what":"Player","bounds":{"min":2,"max":2}}"#, "2 target players")]
    #[case::any_number(
        r#"{"what":{"Object":[{"Type":"Artifact"}]},"bounds":{"min":0,"max":null}}"#,
        "any number of target artifacts"
    )]
    #[case::either(
        r#"{"what":{"Either":["Player",{"Object":[{"Type":"Creature"}]}]}}"#,
        "target player or creature"
    )]
    fn a_target_specification_reads_as_a_card_says_it(#[case] json: &str, #[case] expected: &str) {
        assert_eq!(target(json), expected);
    }

    #[rstest]
    #[case::enters(
        r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#,
        "When it enters the battlefield"
    )]
    #[case::dies(
        r#"{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}"#,
        "When it dies"
    )]
    #[case::another_dies(
        r#"{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}}"#,
        "Whenever a creature you control dies"
    )]
    #[case::lands(
        r#"{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}}"#,
        "Whenever a land you control enters the battlefield"
    )]
    #[case::attacks(r#"{"Attacks":{"what":["This"]}}"#, "When it attacks")]
    #[case::opponent_casts(
        r#"{"SpellCast":{"by":"Opponents"}}"#,
        "Whenever an opponent casts a spell"
    )]
    #[case::you_cast(
        r#"{"SpellCast":{"what":[{"Type":"Instant"}],"by":"You"}}"#,
        "Whenever you cast an instant spell"
    )]
    #[case::sacrificed(
        r#"{"Sacrificed":{"what":[{"Subtype":"Clue"}],"by":"You"}}"#,
        "Whenever you sacrifice a Clue"
    )]
    #[case::upkeep(
        r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#,
        "At the beginning of your upkeep"
    )]
    #[case::any_end_step(
        r#"{"StepBegins":{"step":"End"}}"#,
        "At the beginning of each end step"
    )]
    #[case::combat(
        r#"{"StepBegins":{"step":"BeginCombat","whose":"Opponents"}}"#,
        "At the beginning of combat on an opponent's turn"
    )]
    #[case::target(
        r#"{"BecomesTarget":{"what":["This"]}}"#,
        "When it becomes the target of a spell or ability"
    )]
    #[case::either(
        r#"{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Attacks":{"what":["This"]}}]}"#,
        "When it enters the battlefield or it attacks"
    )]
    fn a_trigger_pattern_reads_as_a_card_says_it(#[case] json: &str, #[case] expected: &str) {
        assert_eq!(words(json), expected);
    }
}
