//! "As an additional cost to cast this spell, choose a creature type." (`CostItem::R110Choose`, CR 601.2b): the caster
//! chooses the value as the spell is announced, with the parts of its costs, and the spell keeps it in
//! `CostTaken.r110_chosen`; the spell and the permanent it becomes read it as the value their source chose.

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::CostItem;
use mtg_cards::trigger::EntersChoice;

use super::{Ctx, Next, What, Work};
use crate::choose::{ChoiceOption, OptionOptions, values};
use crate::decision::DecisionKind;
use crate::object::Chosen;

/// The kind of value the spell's required additional cost has its caster choose; `None` when it has none.
pub(crate) fn r110_kind(card: &CardDefinition) -> Option<EntersChoice> {
    crate::ability_reader::printed_rules(card).find_map(|rule| match &**rule {
        Ability::AdditionalCost {
            cost,
            required: true,
            ..
        } => cost.0.iter().find_map(|item| match item {
            CostItem::R110Choose(kind) => Some(*kind),
            _ => None,
        }),
        _ => None,
    })
}

/// The values a caster may choose of the kind.
fn options(kind: EntersChoice) -> Vec<ChoiceOption> {
    match kind {
        EntersChoice::CreatureType => values::creature_types(),
        _ => Vec::new(),
    }
}

/// The value an option stands for.
fn chosen(kind: EntersChoice, option: &ChoiceOption) -> Option<Chosen> {
    match (kind, option) {
        (EntersChoice::CreatureType, ChoiceOption::Label(name)) => {
            Some(Chosen::CreatureType(mtg_core::card::Subtype(name.clone())))
        }
        _ => None,
    }
}

/// Settles the value the spell has its caster choose, or asks for it with `ChooseOption`; `None` once it is chosen or the
/// spell has none to choose. A kind with nothing to offer cannot be cast.
pub(super) fn r110_run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    let What::Spell(spell) = ctx.what else {
        return None;
    };
    if work.choices.r110_chosen().is_some() {
        return None;
    }
    let kind = r110_kind(ctx.state.object(*spell)?.card())?;
    let offered = options(kind);
    if offered.is_empty() {
        return Some(Next::Drop);
    }
    // The answer is the one to this question: the picks of later questions of the parts start unset.
    work.done.w2f_kind_picked = None;
    if let Some(index) = work.done.r110_picked.take() {
        work.choices
            .r110_set_chosen(chosen(kind, offered.get(index)?));
        return None;
    }
    Some(ctx.ask(DecisionKind::ChooseOption(OptionOptions {
        source: ctx.source,
        options: offered,
        text: "Choose a creature type.".to_owned(),
        can_cancel: ctx.can_cancel(),
    })))
}
