//! A cost change of the abilities a player activates, written back: "Equip abilities you activate cost {1} less to
//! activate.", "Equip costs you pay cost {1} less.", "Cycling abilities you activate cost {2} less to activate." and
//! "Equip abilities you activate of other Equipment cost {1} less to activate.". Read from `Ability::CostModifier` of the
//! kind `Equip` or `Cycling` on the sources of the player, at one use and without a floor.

use mtg_core::card::Subtype;

use crate::ability::{ActivationKind, CostChange};
use crate::mana::ManaCostX;
use crate::refs::{Pred, Ref, Selector};
use crate::value::Amount;

/// The sentences of the change; none for another kind, source, count or condition.
pub fn g13g_activation_texts(
    kind: ActivationKind,
    applies_to: &Selector,
    change: CostChange,
    mana: &ManaCostX,
    per: &Amount,
) -> Vec<String> {
    if !per.is_one() {
        return Vec::new();
    }
    if let Some(text) =
        super::r4702_names::r4702_named_activation_text(kind, applies_to, change, mana)
    {
        return vec![text];
    }
    let word = match kind {
        ActivationKind::Equip => "Equip",
        ActivationKind::Cycling => "Cycling",
        ActivationKind::Ninjutsu => "Ninjutsu",
        ActivationKind::Unlock => "Unlock",
        ActivationKind::Morph => "Morph",
        _ => return Vec::new(),
    };
    let more = match change {
        CostChange::Increase => "more",
        CostChange::Reduce => "less",
    };
    let you = Pred::ControlledBy(Ref::You);
    let other_equipment = [
        Pred::Subtype(Subtype("Equipment".into())),
        Pred::Other,
        you.clone(),
    ];
    let mut out = Vec::new();
    if kind == ActivationKind::Morph {
        if applies_to.0.is_empty() {
            out.push(format!("All morph costs cost {mana} {more}"));
        }
        return out;
    }
    if kind == ActivationKind::Unlock {
        // A special action, not an ability: only the cost a player pays is named.
        if applies_to.0 == [you] {
            out.push(format!("Unlock costs you pay cost {mana} {more}"));
        }
        return out;
    }
    if applies_to.0 == [you] {
        out.push(format!(
            "{word} abilities you activate cost {mana} {more} to activate"
        ));
        if kind == ActivationKind::Equip {
            out.push(format!("Equip costs you pay cost {mana} {more}"));
        }
    } else if kind == ActivationKind::Equip && applies_to.0 == other_equipment {
        out.push(format!(
            "Equip abilities you activate of other Equipment cost {mana} {more} to activate"
        ));
    }
    out
}
