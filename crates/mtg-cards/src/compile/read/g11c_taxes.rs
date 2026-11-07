//! The cost of blocking and the cost of attacking with one creature, written back: "Creatures can't block unless their
//! controller pays {1} for each of those creatures.", "~ can't block creatures with power 3 or greater unless you pay
//! {1}.", "Enchanted creature can't be blocked unless defending player pays {3} for each creature they control that's
//! blocking it." and "~ can't attack or block unless you pay {2}." (the attack and the block tax of one creature as one
//! sentence). Read from `GameRule::BlockTax` and `GameRule::AttackTax` of `who: EachPlayer`.

use super::amounts::for_each_text;
use super::render::capitalize;
use super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::mana::ManaCostX;
use crate::modify::{Attacked, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;
use mtg_core::card::CardType;

/// The creature one tax is about and who pays for it: "~" and "you", the creature an Aura or Equipment is attached to
/// and "its controller".
fn one_creature(selector: &Selector) -> Vec<(&'static str, &'static str)> {
    match selector.0.as_slice() {
        [Pred::This] | [Pred::Is(Ref::This)] => vec![("~", "you pay")],
        [Pred::Is(Ref::Attached)] => vec![
            ("enchanted creature", "its controller pays"),
            ("equipped creature", "its controller pays"),
        ],
        _ => Vec::new(),
    }
}

/// What one taxed creature costs: "{2}", "1 life", "{1} for each +1/+1 counter on it".
fn prices(mana: &ManaCostX, life: u32, per: &Amount) -> Vec<String> {
    let no_mana = mana.rest().symbols().is_empty() && !mana.has_x();
    if mana.has_x() || (life > 0) == !no_mana {
        return Vec::new();
    }
    match per {
        Amount::Fixed(1) if life > 0 => vec![format!("{life} life")],
        Amount::Fixed(1) => vec![mana.to_string()],
        counted if life == 0 && mana.to_string() == "{1}" => {
            let Some(each) = for_each_text(counted) else {
                return Vec::new();
            };
            let mut found = vec![format!("{{1}} for each {each}")];
            if let Some(it) = each.strip_suffix(" on ~") {
                found.push(format!("{{1}} for each {it} on it"));
            }
            found
        }
        _ => Vec::new(),
    }
}

/// The attack tax of one creature, which taxes it whoever it attacks; empty for any other rule.
fn self_attack(rule: &GameRule) -> Option<(&Selector, Vec<String>)> {
    let GameRule::AttackTax {
        who: Ref::EachPlayer,
        attacked: Attacked::Both,
        attackers,
        mana,
        life,
        per,
    } = rule
    else {
        return None;
    };
    Some((attackers, prices(mana, *life, per)))
}

/// "~ can't attack unless you pay {2}".
pub fn g11c_self_attack_texts(rule: &GameRule) -> Vec<String> {
    let Some((attackers, prices)) = self_attack(rule) else {
        return Vec::new();
    };
    let mut written = Vec::new();
    for (creature, payer) in one_creature(attackers) {
        for price in &prices {
            written.push(capitalize(&format!(
                "{creature} can't attack unless {payer} {price}"
            )));
        }
    }
    written
}

/// "creatures", "nonblue creatures": the group of creatures a tax is about, every creature when empty.
fn group(selector: &Selector) -> Option<String> {
    if selector.is_empty() {
        return Some("creatures".to_owned());
    }
    selector
        .0
        .contains(&Pred::Type(CardType::Creature))
        .then(|| phrase_text(selector, Form::All))?
}

/// Every wording of a block tax, without a period.
pub fn g11c_block_texts(rule: &GameRule) -> Vec<String> {
    let GameRule::BlockTax {
        blockers,
        attackers,
        mana,
        life,
        per,
    } = rule
    else {
        return Vec::new();
    };
    let prices = prices(mana, *life, per);
    let mut written = Vec::new();
    // "~ can't block unless you pay {2}", "~ can't block creatures with power 3 or greater unless you pay {1}".
    let blocked = if attackers.is_empty() {
        Some(String::new())
    } else {
        group(attackers).map(|attackers| format!(" {attackers}"))
    };
    for (creature, payer) in one_creature(blockers) {
        for price in &prices {
            if let Some(blocked) = &blocked {
                written.push(format!(
                    "{creature} can't block{blocked} unless {payer} {price}"
                ));
            }
        }
    }
    // "Enchanted creature can't be blocked unless defending player pays {3} for each creature they control that's
    // blocking it."
    if blockers.is_empty()
        && attackers.0 == [Pred::Is(Ref::Attached)]
        && matches!(per, Amount::Fixed(1))
    {
        for price in &prices {
            for attached in ["enchanted creature", "equipped creature"] {
                written.push(format!(
                    "{attached} can't be blocked unless defending player pays {price} for each creature they control that's blocking it"
                ));
            }
        }
    }
    // "Creatures can't block unless their controller pays {1} for each of those creatures."
    if let (Some(group), Some(blocked), Amount::Fixed(1)) = (group(blockers), &blocked, per) {
        for price in &prices {
            for each in ["of those creatures", "blocking creature they control"] {
                written.push(format!(
                    "{group} can't block{blocked} unless their controller pays {price} for each {each}"
                ));
            }
        }
    }
    written.into_iter().map(|text| capitalize(&text)).collect()
}

/// The attack and the block tax of one creature as one sentence, with a capital and a period: "~ can't attack or block
/// unless you pay {2}."; empty for any other two abilities. Selectors are also tried in the readers' order.
pub fn g11c_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let written = pair_as_given(first, second);
    if !written.is_empty() {
        return written;
    }
    let first = super::r5a_order::canonical(first).unwrap_or_else(|| first.clone());
    let second = super::r5a_order::canonical(second).unwrap_or_else(|| second.clone());
    pair_as_given(&first, &second)
}

/// [`g11c_pair_texts`] with the selectors in the order given.
fn pair_as_given(first: &Ability, second: &Ability) -> Vec<String> {
    let (
        Ability::Rule {
            rule: attack,
            condition: None,
            from: ZoneKind::Battlefield,
        },
        Ability::Rule {
            rule:
                GameRule::BlockTax {
                    blockers,
                    attackers,
                    mana,
                    life,
                    per,
                },
            condition: None,
            from: ZoneKind::Battlefield,
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    let Some((attacking, prices)) = self_attack(attack) else {
        return Vec::new();
    };
    let GameRule::AttackTax {
        mana: attack_mana,
        life: attack_life,
        per: attack_per,
        ..
    } = attack
    else {
        return Vec::new();
    };
    let same = attack_mana == mana && attack_life == life && attack_per == per;
    let same_creature = match (attacking.0.as_slice(), blockers.0.as_slice()) {
        ([Pred::Is(Ref::This)], [Pred::This]) => true,
        (one, other) => one == other,
    };
    if !same || !same_creature || !attackers.is_empty() {
        return Vec::new();
    }
    let mut written = Vec::new();
    for (creature, payer) in one_creature(attacking) {
        for price in &prices {
            written.push(capitalize(&format!(
                "{creature} can't attack or block unless {payer} {price}."
            )));
        }
    }
    written
}
