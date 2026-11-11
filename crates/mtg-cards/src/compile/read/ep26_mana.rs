//! Writers of ep26: mana of the colours of a card the source exiled, the order of a cost's parts and a next spell that
//! can't be countered.
//! The mana reads "any of the exiled card's colors" (an imprint) and "any of the exiled cards' colors" (every card exiled with it).

use crate::refs::{EXILED_SLOT, Pred, Ref, Selector, ZoneKind};

/// The slot an imprint ability remembers its card in.
const IMPRINTED: &str = "imprinted";

/// The words of the mana of the colours among the cards the source exiled, `None` for any other objects.
pub(super) fn ep26_exiled_colors_text(among: &Selector) -> Option<&'static str> {
    let [first, Pred::In(ZoneKind::Exile)] = among.0.as_slice() else {
        return None;
    };
    match first {
        Pred::Is(Ref::Remembered { on, slot }) if **on == Ref::This && slot == IMPRINTED => {
            Some("one mana of any of the exiled card's colors")
        }
        Pred::ExiledWith(Ref::This) => Some("one mana of any of the exiled cards' colors"),
        Pred::Is(Ref::Remembered { on, slot }) if **on == Ref::This && slot == EXILED_SLOT => {
            Some("one mana of any of the exiled cards' colors")
        }
        _ => None,
    }
}

/// The cost with "Sacrifice ~" moved last, as cards print it ("Discard your hand, Sacrifice ~"); `None` when it is last
/// already or absent.
pub(super) fn ep26_sacrifice_last(cost: &crate::cost::Cost) -> Option<crate::cost::Cost> {
    use crate::cost::CostItem;
    let at = cost
        .0
        .iter()
        .position(|item| *item == CostItem::SacrificeSelf)?;
    if at + 1 == cost.0.len() {
        return None;
    }
    let mut items = cost.0.clone();
    let sacrifice = items.remove(at);
    items.push(sacrifice);
    Some(crate::cost::Cost(items))
}

/// "The next spell you cast this turn can't be countered", of a creature spell or of an instant or sorcery spell: a
/// `NextSpell` rule that keeps the spell from being countered, until the end of the turn.
pub(super) fn ep26_next_spell_text(effect: &crate::effect::Effect) -> Option<String> {
    use crate::modify::{Duration, GameRule, SpellBenefit};
    use mtg_core::card::CardType;
    let crate::effect::Effect::Rule {
        rule:
            GameRule::NextSpell {
                what,
                benefit: SpellBenefit::Uncounterable,
            },
        duration: Duration::UntilEndOfTurn,
    } = effect
    else {
        return None;
    };
    let instant_or_sorcery = Pred::AnyOf(vec![
        Pred::Type(CardType::Instant),
        Pred::Type(CardType::Sorcery),
    ]);
    let kind = match what.0.as_slice() {
        [] => "spell",
        [Pred::Type(CardType::Creature)] => "creature spell",
        [Pred::Type(CardType::Artifact)] => "artifact spell",
        [Pred::Type(CardType::Instant)] => "instant spell",
        [either] if *either == instant_or_sorcery => "instant or sorcery spell",
        _ => return None,
    };
    Some(format!(
        "the next {kind} you cast this turn can't be countered"
    ))
}
