//! Any amount of mana paid in addition (`Rule::R6012fPayAnyAmount`, CR 601.2f, the Chorus of the Conclave rulings): each
//! rule whose spells the spell being cast is offers {1} that may be paid any number of times, chosen as a number like
//! multikicker; the spell keeps the indices paid in `CostTaken.r6012f_paid`, and the permanent it becomes enters with a
//! +1/+1 counter for each (`r6012f_entering`).

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaSymbol};

use super::optional::Optional;
use super::{Ctx, Work};
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::keywords::OptionalKind;
use crate::object::ObjectRef;
use crate::state::GameState;
use crate::zones::Zone;

/// The label of the cost, as the repeated question names it.
const LABEL: &str = "{1} for a +1/+1 counter";

/// The optional costs the rules give `caster`'s spell `spell`: one {1}, paid any number of times, for each rule whose
/// spells it is, in timestamp order.
pub(super) fn r6012f_offered(
    state: &GameState,
    spell: ObjectId,
    caster: PlayerId,
) -> Vec<Optional> {
    let Some(object) = state.object(spell) else {
        return Vec::new();
    };
    crate::rules::r6012f_any_amounts(state, caster)
        .into_iter()
        .filter(|(source, spells)| {
            let scope = crate::resolve::Scope {
                casting: Some((spell, caster)),
                ..crate::resolve::Scope::announcing(state, caster, *source)
            };
            scope.selects(spells, object.reference()).unwrap_or(false)
        })
        .map(|_| Optional {
            label: LABEL.to_owned(),
            mana: ManaCost::from_symbols([ManaSymbol::Generic(1)]),
            parts: Vec::new(),
            repeatable: true,
            kind: OptionalKind::R6012fPayAnyAmount,
            offerable: true,
            x: 0,
            least_x: 0,
        })
        .collect()
}

/// Keeps the indices of the costs paid for any amount of mana among the optional costs chosen, once for each {1}.
pub(super) fn r6012f_note(ctx: &Ctx<'_>, work: &mut Work<'_>) {
    let paid: Vec<usize> = work
        .choices
        .optional_costs
        .iter()
        .copied()
        .filter(|index| {
            ctx.needs
                .optional
                .get(*index)
                .is_some_and(|cost| cost.kind == OptionalKind::R6012fPayAnyAmount)
        })
        .collect();
    work.choices.r6012f_set_paid(paid);
}

/// The +1/+1 counters the permanent spell `id` enters with for the mana paid in addition for it, proposed with the
/// event that moves it, so a rule that changes what enters with counters reaches them; `None` when none was paid.
pub(crate) fn r6012f_entering(state: &GameState, id: ObjectId) -> Option<Event> {
    let object = state.object(id)?;
    if object.zone() != Zone::Stack {
        return None;
    }
    let paid = i32::try_from(object.choices().r6012f_paid().len()).ok()?;
    (paid > 0).then(|| {
        Event::CountersChanged(CountersChanged {
            on: Target::Object(ObjectRef {
                id,
                counter: object.counter() + 1,
            }),
            counter: mtg_cards::value::CounterKind::PlusOnePlusOne,
            delta: paid,
        })
    })
}
