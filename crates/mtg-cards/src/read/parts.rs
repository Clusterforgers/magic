//! The parts an ability is made of besides its effects: triggers, costs, mana, rules, modifications and replacements.

use super::generic;
use super::phrase::{self, Ctx};
use super::{Step, effects, steps};
use crate::cost::{Cost, CostItem};
use crate::effect::Produce;
use crate::keyword::Keyword;
use crate::modify::{GameRule, Modification};
use crate::refs::{Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen, Step as TurnStep, TriggerPattern};
use crate::value::Amount;
use mtg_core::mana::ManaType;

/// "{W}{U}": mana types as symbols.
fn symbols(types: &[ManaType]) -> String {
    types.iter().fold(String::new(), |mut text, kind| {
        text.push('{');
        text.push(kind.symbol());
        text.push('}');
        text
    })
}

/// Mana an effect adds.
pub(super) fn mana(ctx: &Ctx, produce: &Produce) -> String {
    match produce {
        Produce::Fixed(types) => symbols(types),
        Produce::AnyColor(count) => {
            format!("{} mana of any one color", phrase::amount(ctx, count))
        }
        Produce::OneOf(types) => format!(
            "one mana of {}",
            types
                .iter()
                .map(|kind| format!("{{{}}}", kind.symbol()))
                .collect::<Vec<_>>()
                .join(" or ")
        ),
        other => generic::node(ctx, other),
    }
}

/// A cost: its parts joined by commas.
pub(super) fn cost(ctx: &Ctx, cost: &Cost) -> String {
    if cost.0.is_empty() {
        return "no cost".to_owned();
    }
    cost.0
        .iter()
        .map(|item| cost_item(ctx, item))
        .collect::<Vec<_>>()
        .join(", ")
}

/// "2 cards" or "a card" of a filter: what a cost takes.
fn taken(ctx: &Ctx, count: &Amount, filter: &Selector) -> String {
    let what = phrase::selector(ctx, filter);
    match count {
        Amount::Fixed(1) => format!("a {what}"),
        other => format!("{} {what}s", phrase::amount(ctx, other)),
    }
}

/// One part of a cost.
fn cost_item(ctx: &Ctx, item: &CostItem) -> String {
    match item {
        CostItem::Mana(mana) => mana.to_string(),
        CostItem::Tap => "{T}".to_owned(),
        CostItem::Untap => "{Q}".to_owned(),
        CostItem::PayLife(life) => format!("pay {} life", phrase::amount(ctx, life)),
        CostItem::SacrificeSelf => "sacrifice this permanent".to_owned(),
        CostItem::ExileSelf => "exile this card".to_owned(),
        CostItem::DiscardSelf => "discard this card".to_owned(),
        CostItem::Sacrifice { filter, count } => format!("sacrifice {}", taken(ctx, count, filter)),
        CostItem::Discard { count, filter } => format!(
            "discard {}",
            filter.as_ref().map_or_else(
                || phrase::counted(ctx, count, "card"),
                |filter| taken(ctx, count, filter)
            )
        ),
        CostItem::Mill(count) => format!("mill {}", phrase::counted(ctx, count, "card")),
        CostItem::PayEnergy(count) => format!("pay {} energy", phrase::amount(ctx, count)),
        CostItem::TapPermanents { filter, count } => {
            format!("tap {} you control", taken(ctx, count, filter))
        }
        CostItem::ExileFromGraveyard { filter, count } => {
            format!("exile {} from your graveyard", taken(ctx, count, filter))
        }
        CostItem::ReturnToHand { filter, count } => {
            format!(
                "return {} you control to its owner's hand",
                taken(ctx, count, filter)
            )
        }
        CostItem::RemoveCounters {
            counter,
            count,
            from,
        } => format!(
            "remove {} {} counter{} from {}",
            phrase::amount(ctx, count),
            counter
                .as_ref()
                .map_or_else(|| "any".to_owned(), phrase::counter_text),
            if *count == Amount::one() { "" } else { "s" },
            from.as_ref().map_or_else(
                || "this permanent".to_owned(),
                |objects| phrase::selector(ctx, objects)
            )
        ),
        CostItem::OneOf(options) => options
            .iter()
            .map(|option| cost(ctx, option))
            .collect::<Vec<_>>()
            .join(" or "),
        other => generic::node(ctx, other),
    }
}

/// "your upkeep", "combat on your turn": the moment a step of the turn begins.
fn step_text(ctx: &Ctx, step: TurnStep, whose: &Ref) -> String {
    let owner = phrase::whose(ctx, whose);
    let name = match step {
        TurnStep::Untap => "untap step",
        TurnStep::Upkeep => "upkeep",
        TurnStep::Draw => "draw step",
        TurnStep::Main1 => "precombat main phase",
        TurnStep::Main2 => "postcombat main phase",
        TurnStep::End => "end step",
        TurnStep::Cleanup => "cleanup step",
        TurnStep::BeginCombat => {
            return format!(
                "combat on {} turn",
                if whose.is_you() {
                    "your".to_owned()
                } else {
                    owner
                }
            );
        }
        TurnStep::DeclareAttackers => "declare attackers step",
        TurnStep::DeclareBlockers => "declare blockers step",
        TurnStep::FirstStrikeDamage => "first-strike damage step",
        TurnStep::CombatDamage => "combat damage step",
        TurnStep::EndCombat => "end of combat step",
    };
    format!("{owner} {name}")
}

/// The event a triggered ability waits for.
pub(super) fn trigger(ctx: &Ctx, when: &TriggerPattern) -> String {
    match when {
        TriggerPattern::ZoneChange { what, from, to } => {
            zone_change(ctx, &phrase::selector(ctx, what), *from, *to)
        }
        TriggerPattern::Attacks { what } => format!("{} attacks", phrase::selector(ctx, what)),
        TriggerPattern::AttacksAlone { what } => {
            format!("{} attacks alone", phrase::selector(ctx, what))
        }
        TriggerPattern::Blocks { what } => format!("{} blocks", phrase::selector(ctx, what)),
        TriggerPattern::BecomesBlocked { what } => {
            format!("{} becomes blocked", phrase::selector(ctx, what))
        }
        TriggerPattern::SpellCast { what, by } => {
            let spell = phrase::selector(ctx, what);
            let spell = if what.0.is_empty() {
                "a spell".to_owned()
            } else if spell.ends_with("spell") {
                spell
            } else {
                format!("{spell} spell")
            };
            format!(
                "{} {} {spell}",
                phrase::who(ctx, by),
                if by.is_you() { "cast" } else { "casts" }
            )
        }
        TriggerPattern::DealsDamage { source, to, combat } => format!(
            "{} deals {}damage{}",
            phrase::selector(ctx, source),
            match combat {
                Some(true) => "combat ",
                Some(false) => "noncombat ",
                None => "",
            },
            to.as_ref()
                .map_or_else(String::new, |to| format!(" to {}", phrase::who(ctx, to)))
        ),
        TriggerPattern::LifeGained { who } => phrase::verb(ctx, who, "gain", "life"),
        TriggerPattern::LifeLost { who } => phrase::verb(ctx, who, "lose", "life"),
        TriggerPattern::Drew { who, .. } => phrase::verb(ctx, who, "draw", "a card"),
        TriggerPattern::GroupAttacks { what, at_least } => {
            format!("{at_least} or more {} attack", phrase::selectors(ctx, what))
        }
        TriggerPattern::Tapped { what } => {
            format!("{} becomes tapped", phrase::selector(ctx, what))
        }
        TriggerPattern::Untapped { what } => {
            format!("{} becomes untapped", phrase::selector(ctx, what))
        }
        TriggerPattern::Chapter { number } => format!("chapter {number}"),
        TriggerPattern::StepBegins { step, whose } => {
            format!("the beginning of {}", step_text(ctx, *step, whose))
        }
        TriggerPattern::State { condition } => phrase::condition(ctx, condition),
        TriggerPattern::AnyOf(options) => options
            .iter()
            .map(|option| trigger(ctx, option))
            .collect::<Vec<_>>()
            .join(" or "),
        TriggerPattern::Batch(inner) => format!("one or more times: {}", trigger(ctx, inner)),
        TriggerPattern::Nth { event, nth } => {
            format!("{} for the {nth}th time each turn", trigger(ctx, event))
        }
        other => generic::node(ctx, other),
    }
}

/// An object changing zones, by where it came from and where it went.
fn zone_change(_: &Ctx, what: &str, from: Option<ZoneKind>, to: Option<ZoneKind>) -> String {
    match (from, to) {
        (None, Some(ZoneKind::Battlefield)) => format!("{what} enters the battlefield"),
        (Some(ZoneKind::Battlefield), Some(ZoneKind::Graveyard)) => format!("{what} dies"),
        (Some(ZoneKind::Battlefield), None) => format!("{what} leaves the battlefield"),
        (None, Some(ZoneKind::Graveyard)) => format!("{what} is put into a graveyard"),
        (Some(ZoneKind::Battlefield), Some(ZoneKind::Exile)) => {
            format!("{what} is exiled from the battlefield")
        }
        (Some(from), Some(to)) => format!(
            "{what} moves from {} to {}",
            phrase::zone_text(from),
            phrase::zone_text(to)
        ),
        (Some(from), None) => format!("{what} leaves {}", phrase::zone_text(from)),
        (None, Some(to)) => format!("{what} moves to {}", phrase::zone_text(to)),
        (None, None) => format!("{what} changes zones"),
    }
}

/// A rule of the game a static ability or an effect sets.
pub(super) fn rule(ctx: &Ctx, rule: &GameRule) -> String {
    match rule {
        GameRule::MustAttack(who) => {
            format!("{} must attack each combat if able", phrase::who(ctx, who))
        }
        GameRule::MustBlock(who) => format!("{} must block if able", phrase::who(ctx, who)),
        GameRule::CantBeCountered(who) => format!("{} can't be countered", phrase::who(ctx, who)),
        GameRule::CantGainLife(who) => format!("{} can't gain life", phrase::who(ctx, who)),
        GameRule::CantDraw(who) => format!("{} can't draw cards", phrase::who(ctx, who)),
        GameRule::CantSearch(who) => format!("{} can't search libraries", phrase::who(ctx, who)),
        GameRule::NoMaxHandSize(who) => {
            format!("{} has no maximum hand size", phrase::who(ctx, who))
        }
        GameRule::SkipDrawStep(who) => format!("{} skips the draw step", phrase::who(ctx, who)),
        GameRule::MayCast(permission) => {
            let from = permission.from.map_or_else(String::new, |zone| {
                format!(" from {}", phrase::zone_text(zone))
            });
            let free = if permission.free {
                " without paying its mana cost"
            } else {
                ""
            };
            format!(
                "{} may {} {}{from}{free}",
                phrase::who(ctx, &permission.who),
                if permission.play { "play" } else { "cast" },
                phrase::selectors(ctx, &permission.what)
            )
        }
        GameRule::AttackLimit(most) => {
            format!("no more than {most} creatures can attack each combat")
        }
        GameRule::BlockLimit(most) => {
            format!("no more than {most} creatures can block each combat")
        }
        other => generic::node(ctx, other),
    }
}

/// A keyword with its parameter: "flying", "ward {2}", "protection from red".
pub(super) fn keyword(ctx: &Ctx, keyword: &Keyword) -> String {
    let Ok(serde_json::Value::Object(map)) = serde_json::to_value(keyword) else {
        return generic::node(ctx, keyword);
    };
    let Some((name, inner)) = map.iter().next().filter(|_| map.len() == 1) else {
        return generic::node(ctx, keyword);
    };
    let head = generic::words(name);
    if let Ok(objects) = serde_json::from_value::<Selector>(inner.clone()) {
        let shown = phrase::selector(ctx, &objects);
        let from = if name == "Protection" { " from" } else { "" };
        format!(
            "{head}{from} {}",
            shown.strip_suffix(" permanent").unwrap_or(&shown)
        )
    } else if let Ok(paid) = serde_json::from_value::<Cost>(inner.clone()) {
        format!("{head} {}", cost(ctx, &paid))
    } else {
        generic::node(ctx, keyword)
    }
}

/// A signed number for a power or toughness change: `+2`, `-1`, `X`.
fn signed(ctx: &Ctx, amount: &Amount) -> String {
    match amount {
        Amount::Fixed(number) if *number >= 0 => format!("+{number}"),
        Amount::Fixed(number) => number.to_string(),
        other => phrase::amount(ctx, other),
    }
}

/// The changes a continuous effect or a static ability makes, one step each.
pub(super) fn modifications(ctx: &Ctx, mods: &[Modification], depth: u8, out: &mut Vec<Step>) {
    for change in mods {
        match change {
            Modification::GrantAbility(ability) => {
                out.push(Step::new(depth, "gains this ability:"));
                steps::ability_into(ability, depth + 1, out);
            }
            other => out.push(Step::new(depth, modification(ctx, other))),
        }
    }
}

/// One change as a line.
fn modification(ctx: &Ctx, change: &Modification) -> String {
    match change {
        Modification::ModifyPT { power, toughness } => {
            let plain = |amount: &Amount| matches!(amount, Amount::Fixed(_) | Amount::X);
            if plain(power) && plain(toughness) {
                format!("gets {}/{}", signed(ctx, power), signed(ctx, toughness))
            } else if power == toughness {
                format!("gets +N/+N, where N is {}", phrase::amount(ctx, power))
            } else {
                format!(
                    "gets +({})/+({})",
                    phrase::amount(ctx, power),
                    phrase::amount(ctx, toughness)
                )
            }
        }
        Modification::SetPT { power, toughness } => format!(
            "has base power and toughness {}/{}",
            phrase::amount(ctx, power),
            phrase::amount(ctx, toughness)
        ),
        Modification::AddKeyword(keyword_of) => format!("gains {}", keyword(ctx, keyword_of)),
        Modification::RemoveKeyword(keyword_of) => format!("loses {}", keyword(ctx, keyword_of)),
        Modification::AddType(kind) => {
            format!("is also a {}", generic::words(&format!("{kind:?}")))
        }
        Modification::AddSubtype(kind) => format!("is also a {}", kind.0),
        Modification::LoseAbilities => "loses all abilities".to_owned(),
        Modification::CantAttack => "can't attack".to_owned(),
        Modification::CantBlock => "can't block".to_owned(),
        Modification::CantBeBlocked => "can't be blocked".to_owned(),
        Modification::DoesntUntap => "doesn't untap during its controller's untap step".to_owned(),
        Modification::SetController(player) => {
            format!("is controlled by {}", phrase::who(ctx, player))
        }
        other => generic::node(ctx, other),
    }
}

/// What a replacement effect waits for.
pub(super) fn replace_when(ctx: &Ctx, when: &ReplaceWhen) -> String {
    match when {
        ReplaceWhen::TokenCreation { controller, what } => format!(
            "{} would be created under {} control",
            phrase::selectors(ctx, what).replace("permanents", "tokens"),
            phrase::whose(ctx, controller)
        ),
        ReplaceWhen::CountersPut { on, counter } => format!(
            "{} counters would be put on {}",
            counter
                .as_ref()
                .map_or_else(|| "any".to_owned(), phrase::counter_text),
            on.as_ref()
                .map_or_else(|| "a permanent".to_owned(), |on| phrase::who(ctx, on))
        ),
        ReplaceWhen::Enters { what } => format!(
            "{} would enter the battlefield",
            phrase::selector(ctx, what)
        ),
        ReplaceWhen::Destroyed { what } => {
            format!("{} would be destroyed", phrase::selector(ctx, what))
        }
        ReplaceWhen::Discarded { what } => {
            format!("{} would be discarded", phrase::selector(ctx, what))
        }
        ReplaceWhen::LifeGain { who } => format!("{} would gain life", phrase::who(ctx, who)),
        ReplaceWhen::Draw { who, .. } => format!("{} would draw a card", phrase::who(ctx, who)),
        other => generic::node(ctx, other),
    }
}

/// What happens instead.
pub(super) fn replace_by(ctx: &Ctx, by: &ReplaceBy, depth: u8, out: &mut Vec<Step>) {
    match by {
        ReplaceBy::Prevent => out.push(Step::new(depth, "prevent it")),
        ReplaceBy::WithCounters {
            counter,
            count,
            tapped,
        } => out.push(Step::new(
            depth,
            format!(
                "it enters with {} {} counters{}",
                phrase::amount(ctx, count),
                phrase::counter_text(counter),
                if *tapped { ", tapped" } else { "" }
            ),
        )),
        ReplaceBy::Tapped => out.push(Step::new(depth, "it enters tapped")),
        ReplaceBy::Multiply(factor) => {
            out.push(Step::new(depth, format!("{factor} times as many")));
        }
        ReplaceBy::Add(extra) => out.push(Step::new(depth, format!("{extra} more are added"))),
        ReplaceBy::MoveTo(zone) => out.push(Step::new(
            depth,
            format!("it goes to {} instead", phrase::zone_text(*zone)),
        )),
        ReplaceBy::Instead(list) => {
            out.push(Step::new(depth, "instead:"));
            effects::list(ctx, list, depth + 1, out);
        }
        ReplaceBy::PreventAnd(list) => {
            out.push(Step::new(depth, "prevent it, then:"));
            effects::list(ctx, list, depth + 1, out);
        }
        other => out.push(Step::new(depth, generic::node(ctx, other))),
    }
}

/// "you": a reference of a static ability's subject.
pub(super) fn affected(ctx: &Ctx, subject: &Ref) -> String {
    phrase::who(ctx, subject)
}

#[cfg(test)]
mod tests {
    //! Goal: triggers, costs, rules and changes read as English, and a part with no arm of its own still shows its variant.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::enters(
        r#"{"ZoneChange":{"what":[{"Type":"Creature"}],"to":"Battlefield"}}"#,
        "creature enters the battlefield"
    )]
    #[case::dies(
        r#"{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}"#,
        "this permanent dies"
    )]
    #[case::cast(
        r#"{"SpellCast":{"what":[{"Type":"Enchantment"},"Spell"],"by":"You"}}"#,
        "you cast enchantment spell"
    )]
    #[case::upkeep(
        r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#,
        "the beginning of your upkeep"
    )]
    #[case::beginning_of_combat(
        r#"{"StepBegins":{"step":"BeginCombat","whose":"You"}}"#,
        "the beginning of combat on your turn"
    )]
    #[case::you_gain(r#"{"LifeGained":{"who":"You"}}"#, "you gain life")]
    #[case::they_draw(
        r#"{"Drew":{"who":"Opponents","what":[]}}"#,
        "each opponent draws a card"
    )]
    #[case::chapter(r#"{"Chapter":{"number":2}}"#, "chapter 2")]
    #[case::unworded(r#"{"Explored":{"what":["This"]}}"#, "explored (this permanent)")]
    fn triggers_read_as_events(#[case] json: &str, #[case] shown: &str) {
        let when: TriggerPattern = serde_json::from_str(json).unwrap();
        assert_eq!(trigger(&Ctx::none(), &when), shown);
    }

    #[rstest]
    #[case::mana(r#""{2}{R}""#, "{2}{R}")]
    #[case::tap(r#"["Tap"]"#, "{T}")]
    #[case::tap_and_sacrifice(r#"["Tap","SacrificeSelf"]"#, "{T}, sacrifice this permanent")]
    #[case::life(r#"[{"PayLife":2}]"#, "pay 2 life")]
    #[case::discard(r#"[{"Discard":{"count":1}}]"#, "discard a card")]
    fn costs_read_part_by_part(#[case] json: &str, #[case] shown: &str) {
        let parsed: Cost = serde_json::from_str(json).unwrap();
        assert_eq!(cost(&Ctx::none(), &parsed), shown);
    }

    #[rstest]
    #[case::bare(r#""Flying""#, "flying")]
    #[case::protection(r#"{"Protection":[{"Color":"Red"}]}"#, "protection from red")]
    #[case::ward(r#"{"Ward":"{2}"}"#, "ward {2}")]
    #[case::number(r#"{"Annihilator":2}"#, "annihilator 2")]
    fn keywords_read_with_their_parameters(#[case] json: &str, #[case] shown: &str) {
        let parsed: Keyword = serde_json::from_str(json).unwrap();
        assert_eq!(keyword(&Ctx::none(), &parsed), shown);
    }

    #[rstest]
    #[case::counters(
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":"X"}}}}"#,
        "it enters with X +1/+1 counters"
    )]
    #[case::prevent(
        r#"{"Replacement":{"when":{"Destroyed":{"what":["This"]}},"by":"Prevent"}}"#,
        "prevent it"
    )]
    fn replacements_say_what_happens_instead(#[case] json: &str, #[case] shown: &str) {
        let ability: crate::ability::Ability = serde_json::from_str(json).unwrap();
        let steps = steps::ability(&ability);
        assert_eq!(steps.last().map(|step| step.text.as_str()), Some(shown));
    }

    #[test]
    fn a_free_cost_is_not_blank() {
        assert_eq!(cost(&Ctx::none(), &Cost::default()), "no cost");
    }
}
