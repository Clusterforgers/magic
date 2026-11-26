//! A choice as an additional cost (CR 601.2b): "choose a creature you control or a warped creature card you own in
//! exile". The payer picks one permanent they control or one card they own in exile; nothing happens to it, and the
//! spell reads it as `Ref::Paid`.

use mtg_cards::refs::{Pred, Selector, ZoneKind};
use mtg_core::ids::ObjectId;

use super::{Kind, Part, Pick, Payer};
use crate::characteristics::controller_of;
use crate::zones::Zone;

/// The one pick of the cost: its filter holds both halves, each kept to its zone, so one selector judges a candidate.
pub(super) fn r6012_pick(filter: &Selector, exiled: &Selector) -> (Part, bool) {
    let side = |zone: ZoneKind, of: &Selector| {
        let mut all = vec![Pred::In(zone)];
        all.extend(of.0.iter().cloned());
        Pred::R117AllOf(all)
    };
    let both = Selector(vec![Pred::AnyOf(vec![
        side(ZoneKind::Battlefield, filter),
        side(ZoneKind::Exile, exiled),
    ])]);
    (
        Part::Pick(Pick {
            kind: Kind::R6012Choose,
            filter: both,
            count: 1,
            reads_x: false,
            alike: None,
            up_to: false,
            r6702_total: false,
            r122_crew: false,
        }),
        false,
    )
}

/// The two halves of the pick's filter: what may be chosen on the battlefield and in exile.
fn halves(filter: &Selector) -> (Selector, Selector) {
    let half = |zone: ZoneKind| match filter.0.as_slice() {
        [Pred::AnyOf(sides)] => sides
            .iter()
            .find_map(|side| match side {
                Pred::R117AllOf(all) if all.first() == Some(&Pred::In(zone)) => {
                    Some(Selector(all[1..].to_vec()))
                }
                _ => None,
            })
            .unwrap_or_default(),
        _ => Selector::default(),
    };
    (half(ZoneKind::Battlefield), half(ZoneKind::Exile))
}

/// The permanents the payer controls and the cards they own in exile that the filter selects.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    let mut found = payer.matching(
        payer.state.zone(Zone::Battlefield).iter().copied(),
        filter,
        |id| controller_of(payer.state, id) == Some(payer.player),
    );
    found.extend(payer.matching(
        payer.state.zone(Zone::Exile).iter().copied(),
        filter,
        |id| {
            payer
                .state
                .object(id)
                .is_some_and(|card| card.owner() == payer.player)
        },
    ));
    found
}

/// "Choose a creature you control or a warped creature card you own in exile".
pub(super) fn text(filter: &Selector) -> String {
    let (permanent, exiled) = halves(filter);
    format!(
        "Choose {} you control or {} you own in exile",
        crate::wording::subject(&permanent),
        crate::wording::subject(&exiled)
    )
}

#[cfg(test)]
mod tests {
    //! Goal: the pick of a choice between a permanent and an exiled card keeps each half to its zone and words both.
    use super::*;
    use mtg_core::card::CardType;

    #[test]
    fn r6012_the_filter_splits_back_into_its_halves() {
        let creature = Selector(vec![Pred::Type(CardType::Creature)]);
        let warped = Selector(vec![Pred::R702185Warped, Pred::Type(CardType::Creature)]);
        let (Part::Pick(pick), false) = r6012_pick(&creature, &warped) else {
            panic!("a pick");
        };
        assert_eq!(halves(&pick.filter), (creature, warped));
        assert_eq!(pick.count, 1);
    }
}
