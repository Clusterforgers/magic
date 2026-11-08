//! The Licids' ability: "{W}, {T}: ~ loses this ability and becomes an Aura enchantment with enchant creature. Attach it
//! to target creature. You may pay {W} to end this effect." The attachment is written by the plain writer of the ability
//! without the change, which the words of the change and of its end go around.

use mtg_core::card::{CardType, Subtype};

use crate::ability::Ability;
use crate::effect::Effect;
use crate::keyword::Keyword;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Selector};

/// Whether the change is exactly the Licid's: an Aura enchantment with enchant creature that loses this ability.
fn licid_mods(mods: &[Modification]) -> bool {
    let wanted = [
        Modification::AddSubtype(Subtype("Aura".to_owned())),
        Modification::SetTypes(vec![CardType::Enchantment]),
        Modification::AddKeyword(Keyword::Enchant(Selector(vec![Pred::Type(
            CardType::Creature,
        )]))),
        Modification::LoseThisAbility,
    ];
    mods.len() == wanted.len() && wanted.iter().all(|one| mods.contains(one))
}

/// The wordings of a Licid's ability, written by `write` for the ability that only attaches; none for any other ability.
pub(super) fn e15g_licid_texts(
    ability: &Ability,
    write: impl Fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    let Ability::Activated {
        cost,
        timing,
        condition,
        limit,
        by,
        from,
        targets,
        effects,
    } = ability
    else {
        return None;
    };
    let [
        Effect::Continuous {
            affected: Ref::This,
            mods,
            duration: Duration::UntilPaid(price),
        },
        attach @ Effect::Attach {
            what: Ref::This,
            to: Ref::Target(0),
            bind: None,
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    if !licid_mods(mods) {
        return None;
    }
    let plain = Ability::Activated {
        cost: cost.clone(),
        timing: *timing,
        condition: condition.clone(),
        limit: *limit,
        by: by.clone(),
        from: *from,
        targets: targets.clone(),
        effects: vec![attach.clone()],
    };
    let change =
        "~ loses this ability and becomes an Aura enchantment with enchant creature. Attach it to ";
    let end = format!(" You may pay {price} to end this effect.");
    let texts: Vec<String> = write(&plain)
        .iter()
        .filter_map(|text| {
            let (head, tail) = text.split_once(": Attach ~ to ")?;
            let (body, rest) = tail.split_once('.')?;
            Some(format!("{head}: {change}{body}.{end}{rest}"))
        })
        .collect();
    Some(texts)
}
