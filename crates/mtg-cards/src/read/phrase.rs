//! Noun phrases of the rules: who, which objects, how many, under what condition, for how long.

use super::{generic, parts};
use crate::modify::Duration;
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use crate::value::{Amount, Cmp, Condition, CounterKind, StatKind};

/// What a phrase needs to know about the ability it sits in: the targets its `Target(n)` references name.
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    /// The ability's target specifications.
    pub targets: &'a [TargetSpec],
    /// How deep the phrases of targets nest, which stops a target that names itself.
    depth: u8,
}

impl<'a> Ctx<'a> {
    /// A context with the targets of an ability.
    pub(super) const fn new(targets: &'a [TargetSpec]) -> Self {
        Ctx { targets, depth: 0 }
    }

    /// A context with no targets.
    pub(super) const fn none() -> Ctx<'static> {
        Ctx {
            targets: &[],
            depth: 0,
        }
    }

    /// The same targets, one level down.
    const fn deeper(self) -> Self {
        Ctx {
            depth: self.depth + 1,
            ..self
        }
    }
}

/// A player or object reference as words.
pub(super) fn who(ctx: &Ctx, reference: &Ref) -> String {
    match reference {
        Ref::You => "you".to_owned(),
        Ref::Opponents => "each opponent".to_owned(),
        Ref::EachPlayer => "each player".to_owned(),
        Ref::This => "this permanent".to_owned(),
        Ref::Target(index) => target(ctx, *index),
        Ref::Triggering => "the triggering object".to_owned(),
        Ref::Dealer => "the source of the damage".to_owned(),
        Ref::Attached => "the permanent it is attached to".to_owned(),
        Ref::AttachedPlayer => "the player it is attached to".to_owned(),
        Ref::Monarch => "the monarch".to_owned(),
        Ref::Chosen => "the chosen object".to_owned(),
        Ref::Promised => "the promised object".to_owned(),
        Ref::Defending => "the defending player".to_owned(),
        Ref::Attacked => "the attacked player".to_owned(),
        Ref::MostLife => "the player with the most life".to_owned(),
        Ref::Bound(name) => format!("\u{201c}{name}\u{201d}"),
        Ref::Paid => "what was paid".to_owned(),
        Ref::ControllerOf(inner) => format!("the controller of {}", who(ctx, inner)),
        Ref::OwnerOf(inner) => format!("the owner of {}", who(ctx, inner)),
        Ref::Each(selector) => format!("each {}", self::selector(ctx, selector)),
    }
}

/// "draws" for "draw": the verb after a subject other than you.
fn third(verb: &str) -> String {
    let consonant_y = verb.ends_with('y')
        && verb
            .chars()
            .rev()
            .nth(1)
            .is_some_and(|c| !"aeiou".contains(c));
    if consonant_y {
        format!("{}ies", &verb[..verb.len() - 1])
    } else if verb.ends_with(['s', 'x', 'z']) || verb.ends_with("ch") || verb.ends_with("sh") {
        format!("{verb}es")
    } else {
        format!("{verb}s")
    }
}

/// A player and what they do: "you draw a card", "each opponent draws a card".
pub(super) fn verb(ctx: &Ctx, player: &Ref, base: &str, rest: &str) -> String {
    let tail = if rest.is_empty() {
        String::new()
    } else {
        format!(" {rest}")
    };
    if player.is_you() {
        format!("you {base}{tail}")
    } else {
        format!("{} {}{tail}", who(ctx, player), third(base))
    }
}

/// "your" or "target player's": a reference as a possessive.
pub(super) fn whose(ctx: &Ctx, reference: &Ref) -> String {
    match reference {
        Ref::You => "your".to_owned(),
        other => format!("{}'s", who(ctx, other)),
    }
}

/// The target the n-th specification asks for, "target 1" when the ability has no such specification.
fn target(ctx: &Ctx, index: u8) -> String {
    match ctx.targets.get(usize::from(index)) {
        Some(spec) if ctx.depth < 3 => spec_text(&ctx.deeper(), spec),
        _ => format!("target {}", u16::from(index) + 1),
    }
}

/// A target specification: "target creature", "up to two target creatures", "any target".
pub(super) fn spec_text(ctx: &Ctx, spec: &TargetSpec) -> String {
    let (min, max) = (&spec.bounds.min, &spec.bounds.max);
    let one = Bounds::default();
    let counted = spec.bounds != one;
    let single = targetable(ctx, &spec.what, false);
    let text = if counted {
        let many = targetable(ctx, &spec.what, true);
        match (min, max) {
            (Amount::Fixed(0), Some(most)) => format!("up to {} {many}", amount(ctx, most)),
            (Amount::Fixed(0), None) => format!("any number of {many}"),
            (Amount::Fixed(1), None) => format!("one or more {many}"),
            (least, Some(most)) if least == most => format!("{} {many}", amount(ctx, least)),
            (least, Some(most)) => {
                format!("{} to {} {many}", amount(ctx, least), amount(ctx, most))
            }
            (least, None) => format!("at least {} {many}", amount(ctx, least)),
        }
    } else {
        single
    };
    match &spec.divide {
        Some(total) => format!("{text}, dividing {} among them", amount(ctx, total)),
        None => text,
    }
}

/// What a target can be, as one thing or as several.
fn targetable(ctx: &Ctx, what: &Targetable, plural: bool) -> String {
    let noun = |text: &str| {
        if plural {
            format!("target {}", pluralize(text))
        } else {
            format!("target {text}")
        }
    };
    match what {
        Targetable::AnyTarget if plural => "targets".to_owned(),
        Targetable::AnyTarget => "any target".to_owned(),
        Targetable::Player => noun("player"),
        Targetable::Opponent => noun("opponent"),
        Targetable::Object(objects) if plural => format!("target {}", selectors(ctx, objects)),
        Targetable::Object(objects) => noun(&selector(ctx, objects)),
        Targetable::Either(options) => options
            .iter()
            .map(|option| targetable(ctx, option, plural))
            .enumerate()
            .map(|(at, text)| match text.strip_prefix("target ") {
                Some(rest) if at > 0 => rest.to_owned(),
                _ => text,
            })
            .collect::<Vec<_>>()
            .join(" or "),
        Targetable::PerPlayer { players, what } => format!(
            "{} controlled by each of {}",
            noun(&selector(ctx, what)),
            who(ctx, players)
        ),
    }
}

/// "creature" to "creatures": the last word takes an s.
fn pluralize(noun: &str) -> String {
    if noun.ends_with('s') {
        noun.to_owned()
    } else {
        format!("{noun}s")
    }
}

/// Where a part of a selector goes in the phrase.
enum Part {
    /// Before the noun ("tapped", "red").
    Before(String),
    /// A subtype, the most specific noun.
    Subtype(String),
    /// A card type.
    Type(String),
    /// What kind of object it is ("spell", "activated ability").
    Kind(String),
    /// After the noun ("you control", "with flying").
    After(String),
}

/// A selector as a noun phrase: "tapped creature you control".
pub(super) fn selector(ctx: &Ctx, objects: &Selector) -> String {
    noun_phrase(ctx, objects, false)
}

/// A selector as a plural noun phrase: "tapped creatures you control".
pub(super) fn selectors(ctx: &Ctx, objects: &Selector) -> String {
    noun_phrase(ctx, objects, true)
}

/// The noun the objects of a selector are called when it names none: a card outside the battlefield, else a permanent.
fn default_noun(objects: &Selector) -> &'static str {
    let elsewhere = objects
        .0
        .iter()
        .any(|pred| matches!(pred, Pred::In(zone) if *zone != ZoneKind::Battlefield));
    if elsewhere { "card" } else { "permanent" }
}

/// A selector as a noun phrase, in the singular or the plural.
fn noun_phrase(ctx: &Ctx, selector: &Selector, plural: bool) -> String {
    let (mut before, mut subtypes, mut types, mut kinds, mut after) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for pred in &selector.0 {
        match part(ctx, pred) {
            Part::Before(text) => before.push(text),
            Part::Subtype(text) => subtypes.push(text),
            Part::Type(text) => types.push(text),
            Part::Kind(text) => kinds.push(text),
            Part::After(text) => after.push(text),
        }
    }
    let mut nouns = subtypes;
    nouns.extend(types);
    nouns.extend(kinds);
    let noun = if nouns.is_empty() {
        default_noun(selector).to_owned()
    } else {
        nouns.join(" ")
    };
    let noun = if plural { pluralize(&noun) } else { noun };
    before
        .into_iter()
        .chain([noun])
        .chain(after)
        .collect::<Vec<_>>()
        .join(" ")
}

/// "a" or "an" before a noun.
fn article(noun: &str) -> &'static str {
    if noun.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    }
}

/// A name of a type or colour from its variant: `Creature` is "creature".
fn lower<T: std::fmt::Debug>(value: &T) -> String {
    generic::words(&format!("{value:?}"))
}

/// One predicate of a selector, placed in the phrase.
fn part(ctx: &Ctx, pred: &Pred) -> Part {
    match pred {
        Pred::This => Part::Kind("this permanent".to_owned()),
        Pred::Other => Part::Before("other".to_owned()),
        Pred::Is(reference) => Part::Kind(who(ctx, reference)),
        Pred::Type(card_type) => Part::Type(lower(card_type)),
        Pred::Supertype(supertype) => Part::Before(lower(supertype)),
        Pred::Subtype(subtype) => Part::Subtype(subtype.0.clone()),
        Pred::Color(color) => Part::Before(lower(color)),
        Pred::Colorless => Part::Before("colorless".to_owned()),
        Pred::Token => Part::Before("token".to_owned()),
        Pred::Commander => Part::Before("commander".to_owned()),
        Pred::Spell => Part::Kind("spell".to_owned()),
        Pred::Ability(kind) => Part::Kind(format!("{} ability", lower(kind))),
        Pred::Tapped => Part::Before("tapped".to_owned()),
        Pred::Untapped => Part::Before("untapped".to_owned()),
        Pred::Attacking => Part::Before("attacking".to_owned()),
        Pred::Blocking => Part::Before("blocking".to_owned()),
        Pred::Unblocked => Part::Before("unblocked".to_owned()),
        Pred::FaceDown => Part::Before("face-down".to_owned()),
        Pred::ControlledBy(Ref::You) => Part::After("you control".to_owned()),
        Pred::ControlledBy(Ref::Opponents) => Part::After("an opponent controls".to_owned()),
        Pred::ControlledBy(owner) => Part::After(format!("controlled by {}", who(ctx, owner))),
        Pred::OwnedBy(Ref::You) => Part::After("you own".to_owned()),
        Pred::OwnedBy(Ref::Opponents) => Part::After("an opponent owns".to_owned()),
        Pred::OwnedBy(owner) => Part::After(format!("owned by {}", who(ctx, owner))),
        Pred::In(zone) => Part::After(format!("in {}", zone_text(*zone))),
        Pred::CastFrom(zone) => Part::After(format!("cast from {}", zone_text(*zone))),
        Pred::HasKeyword(keyword) => Part::After(format!("with {}", parts::keyword(ctx, keyword))),
        Pred::HasCounter(counter) => {
            Part::After(format!("with a {} counter", counter_text(counter)))
        }
        Pred::HasAnyCounter => Part::After("with a counter".to_owned()),
        Pred::Named(name) => Part::After(format!("named {name}")),
        Pred::Compare { stat, op, to } => Part::After(format!(
            "with {} {}",
            stat_text(*stat),
            limit(*op, &amount(ctx, to))
        )),
        Pred::Not(inner) => negated(ctx, inner),
        Pred::AnyOf(options) => Part::Kind(
            options
                .iter()
                .map(|option| selector(ctx, &Selector(vec![option.clone()])))
                .collect::<Vec<_>>()
                .join(" or "),
        ),
        other => Part::After(generic::node(ctx, other)),
    }
}

/// "non" before a type or colour, "not" before anything else.
fn negated(ctx: &Ctx, inner: &Pred) -> Part {
    if *inner == Pred::This {
        return Part::Before("other".to_owned());
    }
    match part(ctx, inner) {
        Part::Type(text) | Part::Before(text) => Part::Before(format!("non{text}")),
        Part::Subtype(text) => Part::Before(format!("non-{text}")),
        Part::Kind(text) | Part::After(text) => Part::After(match text.as_str() {
            "you control" => "you don't control".to_owned(),
            "you own" => "you don't own".to_owned(),
            _ => format!("not {text}"),
        }),
    }
}

/// A sum as words: "a + b", a negative part as "- 1".
fn sum(ctx: &Ctx, parts: &[Amount]) -> String {
    let mut shown = parts.iter().map(|part| match part {
        Amount::Fixed(number) if *number < 0 => (" - ", number.unsigned_abs().to_string()),
        other => (" + ", amount(ctx, other)),
    });
    let first = parts
        .first()
        .map(|part| amount(ctx, part))
        .unwrap_or_default();
    shown.next();
    shown.fold(first, |text, (sign, part)| format!("{text}{sign}{part}"))
}

/// A zone as words.
pub(super) fn zone_text(zone: ZoneKind) -> &'static str {
    match zone {
        ZoneKind::Battlefield => "the battlefield",
        ZoneKind::Hand => "a hand",
        ZoneKind::Library => "a library",
        ZoneKind::Graveyard => "a graveyard",
        ZoneKind::Stack => "the stack",
        ZoneKind::Exile => "exile",
        ZoneKind::Command => "the command zone",
    }
}

/// A kind of counter as words.
pub(super) fn counter_text(counter: &CounterKind) -> String {
    match counter {
        CounterKind::PlusOnePlusOne => "+1/+1".to_owned(),
        CounterKind::MinusOneMinusOne => "-1/-1".to_owned(),
        CounterKind::Loyalty => "loyalty".to_owned(),
        CounterKind::Poison => "poison".to_owned(),
        CounterKind::Charge => "charge".to_owned(),
        CounterKind::Other(name) => name.clone(),
    }
}

/// A characteristic as words.
fn stat_text(stat: StatKind) -> &'static str {
    match stat {
        StatKind::Power => "power",
        StatKind::Toughness => "toughness",
        StatKind::ManaValue => "mana value",
        StatKind::Loyalty => "loyalty",
        StatKind::Colors => "number of colors",
        StatKind::BasicLandTypes => "number of basic land types",
    }
}

/// "2 or less": a bound on a value, as a selector says it.
fn limit(op: Cmp, right: &str) -> String {
    match op {
        Cmp::Eq => right.to_owned(),
        Cmp::Ne => format!("other than {right}"),
        Cmp::Lt => format!("less than {right}"),
        Cmp::Le => format!("{right} or less"),
        Cmp::Gt => format!("more than {right}"),
        Cmp::Ge => format!("{right} or more"),
    }
}

/// "is at least 3": a comparison with its right side.
fn comparison(op: Cmp, right: &str) -> String {
    let word = match op {
        Cmp::Eq => "is",
        Cmp::Ne => "is not",
        Cmp::Lt => "is less than",
        Cmp::Le => "is at most",
        Cmp::Gt => "is more than",
        Cmp::Ge => "is at least",
    };
    format!("{word} {right}")
}

/// An amount as words: a number, X, or what it counts.
pub(super) fn amount(ctx: &Ctx, value: &Amount) -> String {
    match value {
        Amount::Fixed(number) => number.to_string(),
        Amount::X => "X".to_owned(),
        Amount::Count(objects) => format!("the number of {}", selectors(ctx, objects)),
        Amount::Stat { of, stat } => format!("{} of {}", stat_text(*stat), who(ctx, of)),
        Amount::Life(player) => format!("{} life total", whose(ctx, player)),
        Amount::Counters { on, counter } => {
            format!(
                "the number of {} counters on {}",
                counter_text(counter),
                who(ctx, on)
            )
        }
        Amount::AnyCounters(on) => format!("the number of counters on {}", who(ctx, on)),
        Amount::Sum(parts) => sum(ctx, parts),
        Amount::Times { by, of } => format!("{by} times {}", amount(ctx, of)),
        Amount::Half { of, up } => {
            format!(
                "half of {}, rounded {}",
                amount(ctx, of),
                if *up { "up" } else { "down" }
            )
        }
        Amount::Divided => "the share dealt to this target".to_owned(),
        Amount::EvenShare(total) => format!("an even share of {}", amount(ctx, total)),
        Amount::TargetCount(_) => "the number of targets".to_owned(),
        Amount::Greatest { of, stat } => {
            format!(
                "the greatest {} among {}",
                stat_text(*stat),
                selectors(ctx, of)
            )
        }
        Amount::Total { of, stat } => {
            format!("the total {} of {}", stat_text(*stat), selectors(ctx, of))
        }
        Amount::Triggering => "the amount from the triggering event".to_owned(),
        Amount::Players(players) => format!("the number of players in {}", who(ctx, players)),
        other => generic::node(ctx, other),
    }
}

/// "a card" or "3 cards": a count of a noun.
pub(super) fn counted(ctx: &Ctx, count: &Amount, noun: &str) -> String {
    match count {
        Amount::Fixed(1) => format!("a {noun}"),
        Amount::Fixed(number) => format!("{number} {}", pluralize(noun)),
        Amount::X => format!("X {}", pluralize(noun)),
        other => format!("{} equal to {}", pluralize(noun), amount(ctx, other)),
    }
}

/// A condition as words.
pub(super) fn condition(ctx: &Ctx, cond: &Condition) -> String {
    match cond {
        Condition::Not(inner) => format!("not ({})", condition(ctx, inner)),
        Condition::All(parts) => parts
            .iter()
            .map(|part| condition(ctx, part))
            .collect::<Vec<_>>()
            .join(" and "),
        Condition::Any(parts) => parts
            .iter()
            .map(|part| condition(ctx, part))
            .collect::<Vec<_>>()
            .join(" or "),
        Condition::Compare { left, op, right } => {
            format!(
                "{} {}",
                amount(ctx, left),
                comparison(*op, &amount(ctx, right))
            )
        }
        Condition::Matches {
            subject,
            selector: objects,
        } => {
            let noun = selector(ctx, objects);
            format!("{} is {} {noun}", who(ctx, subject), article(&noun))
        }
        Condition::Kicked => "this spell was kicked".to_owned(),
        Condition::AdditionalCostPaid => "the additional cost was paid".to_owned(),
        Condition::YourTurn => "it is your turn".to_owned(),
        Condition::IsDay => "it is day".to_owned(),
        Condition::IsNight => "it is night".to_owned(),
        Condition::InCombat => "it is combat".to_owned(),
        Condition::Monarch(Ref::You) => "you are the monarch".to_owned(),
        Condition::Monarch(player) => format!("{} is the monarch", who(ctx, player)),
        other => generic::node(ctx, other),
    }
}

/// How long an effect lasts.
pub(super) fn duration(value: &Duration) -> String {
    match value {
        Duration::UntilEndOfTurn => "until end of turn".to_owned(),
        Duration::UntilEndOfCombat => "until end of combat".to_owned(),
        Duration::UntilYourNextTurn => "until your next turn".to_owned(),
        Duration::UntilEndOfYourNextTurn => "until the end of your next turn".to_owned(),
        Duration::WhileSourceOnBattlefield => "while this stays on the battlefield".to_owned(),
        Duration::WhileYouControlSource => "while you control this".to_owned(),
        Duration::UntilOpponentBecomesMonarch => "until an opponent becomes the monarch".to_owned(),
        Duration::NextUntapStep => "until the next untap step".to_owned(),
        Duration::WhileHasCounter(counter) => {
            format!("while it has a {} counter", counter_text(counter))
        }
        Duration::Forever => "permanently".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: references, selectors, amounts and conditions read as the English a player expects, whichever of their parts a card uses.
    use rstest::rstest;

    use super::*;

    fn selector_of(json: &str) -> String {
        selector(&Ctx::none(), &serde_json::from_str(json).unwrap())
    }

    #[rstest]
    #[case::empty("[]", "permanent")]
    #[case::creature(r#"[{"Type":"Creature"}]"#, "creature")]
    #[case::yours(
        r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#,
        "creature you control"
    )]
    #[case::other_tapped(r#"["Other","Tapped",{"Type":"Land"}]"#, "other tapped land")]
    #[case::subtype_first(r#"[{"Type":"Creature"},{"Subtype":"Goblin"}]"#, "Goblin creature")]
    #[case::noncreature(
        r#"[{"Not":{"Type":"Creature"}},{"Type":"Artifact"}]"#,
        "noncreature artifact"
    )]
    #[case::spell(r#"[{"Type":"Enchantment"},"Spell"]"#, "enchantment spell")]
    #[case::keyword(
        r#"[{"Type":"Creature"},{"HasKeyword":"Flying"}]"#,
        "creature with flying"
    )]
    #[case::any_of(
        r#"[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]}]"#,
        "artifact or creature"
    )]
    #[case::power(
        r#"[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}]"#,
        "creature with power 2 or less"
    )]
    fn selectors_read_as_noun_phrases(#[case] json: &str, #[case] shown: &str) {
        assert_eq!(selector_of(json), shown);
    }

    #[rstest]
    #[case::any(r#"{"what":"AnyTarget"}"#, "any target")]
    #[case::player(r#"{"what":"Player"}"#, "target player")]
    #[case::creature(r#"{"what":{"Object":[{"Type":"Creature"}]}}"#, "target creature")]
    #[case::up_to(
        r#"{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":2}}"#,
        "up to 2 target creatures"
    )]
    #[case::any_number(
        r#"{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":null}}"#,
        "any number of target creatures"
    )]
    #[case::either(
        r#"{"what":{"Either":[{"Object":[{"Type":"Creature"}]},"Player"]}}"#,
        "target creature or player"
    )]
    fn target_specifications_read_with_their_bounds(#[case] json: &str, #[case] shown: &str) {
        let spec: TargetSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec_text(&Ctx::none(), &spec), shown);
    }

    #[test]
    fn a_target_that_names_itself_does_not_loop() {
        let spec: TargetSpec = serde_json::from_str(
            r#"{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":{"Target":0}}]}}"#,
        )
        .unwrap();
        let specs = [spec];
        let shown = who(&Ctx::new(&specs), &Ref::Target(0));
        assert!(
            shown.starts_with("target creature controlled by"),
            "{shown}"
        );
    }

    #[rstest]
    #[case::fixed("3", "3")]
    #[case::x(r#""X""#, "X")]
    #[case::count(r#"{"Count":[{"Type":"Creature"}]}"#, "the number of creatures")]
    #[case::count_yours(
        r#"{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]}"#,
        "the number of artifacts you control"
    )]
    #[case::count_cards(
        r#"{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"}]}"#,
        "the number of cards in a graveyard you own"
    )]
    #[case::life(r#"{"Life":"You"}"#, "your life total")]
    #[case::sum(r#"{"Sum":[{"TargetCount":0},-1]}"#, "the number of targets - 1")]
    #[case::fallback(r#"{"Devotion":["Black"]}"#, "devotion black")]
    fn amounts_read_as_numbers_or_what_they_count(#[case] json: &str, #[case] shown: &str) {
        let amount_value: Amount = serde_json::from_str(json).unwrap();
        assert_eq!(amount(&Ctx::none(), &amount_value), shown);
    }

    #[test]
    fn a_count_reads_with_its_noun() {
        assert_eq!(counted(&Ctx::none(), &Amount::Fixed(1), "card"), "a card");
        assert_eq!(counted(&Ctx::none(), &Amount::Fixed(3), "card"), "3 cards");
        assert_eq!(counted(&Ctx::none(), &Amount::X, "card"), "X cards");
    }
}
