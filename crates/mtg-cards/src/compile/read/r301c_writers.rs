//! Wordings of a keyword cost followed by the spell's own reduction when it is cast that way, one clause of the card
//! ("Flashback {8}{G}{G}. This spell costs {X} less to cast this way, where X is the greatest mana value of a commander
//! you own on the battlefield or in the command zone."), and of a change to the cost of the spells cast a way ("Flashback
//! costs you pay cost {2} less.").

use mtg_core::mana::{ManaCost, ManaSymbol};

use super::defining::value_texts;
use crate::ability::{Ability, CostChange};
use crate::keyword::Keyword;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, CastWay, Condition, StatKind};

/// Every wording of `keyword` and then `modifier` as one clause; empty unless `modifier` reduces this spell's cost by
/// `{1}` per an amount only when it is cast for the cost `keyword` gives.
pub fn keyword_cost_texts(keyword: &Ability, modifier: &Ability) -> Vec<String> {
    let Ability::Keyword(Keyword::Flashback(cost)) = keyword else {
        return Vec::new();
    };
    let Some(cost) = super::costs::text(cost) else {
        return Vec::new();
    };
    let Ability::CostModifier {
        applies_to,
        change: CostChange::Reduce,
        mana,
        per,
        condition: Some(Condition::CastWith(CastWay::Flashback)),
        activating: None,
        targeting: None,
        least: 0,
        life: 0,
    } = modifier
    else {
        return Vec::new();
    };
    let one = ManaCost::from_symbols(vec![ManaSymbol::Generic(1)]);
    if applies_to.0.as_slice() != [Pred::This] || mana.rest() != &one || mana.has_x() {
        return Vec::new();
    }
    let mut amounts = value_texts(per);
    if *per == commander_mana_value() {
        amounts.push(
            "the greatest mana value of a commander you own on the battlefield or in the command zone"
                .to_owned(),
        );
    }
    // A cost of mana alone follows the keyword after a space, any other after a dash (CR 702.34a).
    let mana_only = cost.starts_with('{') && !cost.contains(',');
    let head = if mana_only {
        format!("Flashback {cost}")
    } else {
        format!("Flashback\u{2014}{cost}")
    };
    amounts
        .iter()
        .map(|amount| {
            format!("{head}. This spell costs {{X}} less to cast this way, where X is {amount}")
        })
        .collect()
}

/// Adds "Flashback costs you pay cost {2} less" and "Dash costs your opponents pay cost {2} more" for a fixed change to
/// the cost of the spells you or your opponents cast that way.
pub(super) fn way_cost_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::CostModifier {
        applies_to,
        change,
        mana,
        per,
        condition: None,
        activating: None,
        targeting: None,
        least: 0,
        life: 0,
    } = ability
    else {
        return;
    };
    let [Pred::Spell, Pred::CastWith(way), Pred::ControlledBy(who)] = applies_to.0.as_slice()
    else {
        return;
    };
    if mana.has_x() || !per.is_one() {
        return;
    }
    let Some(name) = way_name(*way) else {
        return;
    };
    let payer = match who {
        Ref::You => "you",
        Ref::Opponents => "your opponents",
        _ => return,
    };
    let direction = match change {
        CostChange::Reduce => "less",
        CostChange::Increase => "more",
    };
    let text = format!("{name} costs {payer} pay cost {mana} {direction}");
    if !texts.contains(&text) {
        texts.push(text);
    }
}

/// The keyword of a way to cast a spell whose cost a card may change.
fn way_name(way: CastWay) -> Option<&'static str> {
    Some(match way {
        CastWay::Flashback => "Flashback",
        CastWay::Dash => "Dash",
        CastWay::Blitz => "Blitz",
        CastWay::Escape => "Escape",
        _ => return None,
    })
}

/// The greatest mana value among the commanders you own on the battlefield or in the command zone.
fn commander_mana_value() -> Amount {
    Amount::Greatest {
        of: Selector(vec![
            Pred::Commander,
            Pred::OwnedBy(Ref::You),
            Pred::AnyOf(vec![
                Pred::In(ZoneKind::Battlefield),
                Pred::In(ZoneKind::Command),
            ]),
        ]),
        stat: StatKind::ManaValue,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a flashback cost and the spell's reduction by its owner's commanders when cast that way read as one clause, and near misses (another way, another amount, an increase, another spell) have no such wording.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::keyword_cost_texts;
    use crate::ability::Ability;
    use rstest::rstest;

    const VISIONS: &str = "Flashback {8}{G}{G}. This spell costs {X} less to cast this way, where X is the greatest mana value of a commander you own on the battlefield or in the command zone";

    fn ability(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    fn modifier(change: &str, applies_to: &str, way: &str, of: &str) -> Ability {
        ability(&format!(
            r#"{{"CostModifier":{{"applies_to":{applies_to},"change":"{change}","mana":"{{1}}","per":{{"Greatest":{{"of":{of},"stat":"ManaValue"}}}},"condition":{{"CastWith":"{way}"}}}}}}"#
        ))
    }

    const COMMANDERS: &str =
        r#"["Commander",{"OwnedBy":"You"},{"AnyOf":[{"In":"Battlefield"},{"In":"Command"}]}]"#;

    #[test]
    fn the_flashback_reduction_reads_as_the_card() {
        let flashback = ability(r#"{"Keyword":{"Flashback":"{8}{G}{G}"}}"#);
        let texts = keyword_cost_texts(
            &flashback,
            &modifier("Reduce", r#"["This"]"#, "Flashback", COMMANDERS),
        );
        assert!(texts.iter().any(|text| text == VISIONS), "{texts:?}");
    }

    #[rstest]
    #[case::another_way("Reduce", r#"["This"]"#, "Escape", COMMANDERS)]
    #[case::an_increase("Increase", r#"["This"]"#, "Flashback", COMMANDERS)]
    #[case::other_spells("Reduce", r#"[{"Type":"Instant"}]"#, "Flashback", COMMANDERS)]
    #[case::creatures_you_control(
        "Reduce",
        r#"["This"]"#,
        "Flashback",
        r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#
    )]
    fn near_misses_do_not_read_as_the_card(
        #[case] change: &str,
        #[case] applies_to: &str,
        #[case] way: &str,
        #[case] of: &str,
    ) {
        let flashback = ability(r#"{"Keyword":{"Flashback":"{8}{G}{G}"}}"#);
        let texts = keyword_cost_texts(&flashback, &modifier(change, applies_to, way, of));
        assert!(!texts.iter().any(|text| text == VISIONS), "{texts:?}");
    }
}
