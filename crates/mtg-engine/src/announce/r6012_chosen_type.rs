//! A pick of a spell's cost that names the creature type chosen with the same cost (CR 601.2b: "you may choose a
//! creature type and behold two creatures of that type"): once the type is chosen the pick takes that type; before,
//! it takes objects that share a creature type, so the cost is offered only when some type can pay it.

use mtg_cards::cost::Alike;
use mtg_cards::refs::Pred;

use crate::object::Chosen;
use crate::parts::Part;

/// Writes the chosen creature type into the pick, or asks the objects picked to share one while none is chosen.
pub(super) fn r6012_fix_chosen(part: &mut Part, chosen: Option<&Chosen>) {
    let Part::Pick(pick) = part else {
        return;
    };
    if !pick.filter.0.contains(&Pred::ChosenSubtype) {
        return;
    }
    match chosen {
        Some(Chosen::CreatureType(subtype)) => {
            for pred in &mut pick.filter.0 {
                if *pred == Pred::ChosenSubtype {
                    *pred = Pred::Subtype(subtype.clone());
                }
            }
        }
        Some(_) => {}
        None => {
            pick.filter.0.retain(|pred| *pred != Pred::ChosenSubtype);
            if pick.alike.is_none() {
                pick.alike = Some(Alike::R6012ShareCreatureType);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a pick naming the chosen creature type takes that type once it is chosen, asks for a shared creature type
    //! before, and a pick without it or with another chosen value is left as it is.
    use super::*;
    use crate::parts::{Kind, Pick};
    use mtg_cards::refs::Selector;
    use mtg_core::card::{CardType, Subtype};
    use rstest::rstest;

    fn behold(filter: Vec<Pred>) -> Part {
        Part::Pick(Pick {
            kind: Kind::Behold(false),
            filter: Selector(filter),
            count: 2,
            reads_x: false,
            alike: None,
            up_to: false,
            r6702_total: false,
            r122_crew: false,
        })
    }

    fn elf() -> Chosen {
        Chosen::CreatureType(Subtype("Elf".into()))
    }

    #[rstest]
    #[case::chosen_elf(
        vec![Pred::Type(CardType::Creature), Pred::ChosenSubtype],
        Some(elf()),
        vec![Pred::Type(CardType::Creature), Pred::Subtype(Subtype("Elf".into()))],
        None
    )]
    #[case::none_chosen_yet(
        vec![Pred::Type(CardType::Creature), Pred::ChosenSubtype],
        None,
        vec![Pred::Type(CardType::Creature)],
        Some(Alike::R6012ShareCreatureType)
    )]
    #[case::no_chosen_type_named(vec![Pred::Type(CardType::Creature)], None, vec![Pred::Type(CardType::Creature)], None)]
    #[case::another_value_chosen(
        vec![Pred::ChosenSubtype],
        Some(Chosen::Color(mtg_core::color::Color::Red)),
        vec![Pred::ChosenSubtype],
        None
    )]
    fn r6012_the_pick_reads_the_type_chosen_with_the_cost(
        #[case] filter: Vec<Pred>,
        #[case] chosen: Option<Chosen>,
        #[case] expected: Vec<Pred>,
        #[case] alike: Option<Alike>,
    ) {
        let mut part = behold(filter);
        r6012_fix_chosen(&mut part, chosen.as_ref());
        let Part::Pick(pick) = part else {
            panic!("a pick");
        };
        assert_eq!(pick.filter.0, expected);
        assert_eq!(pick.alike, alike);
    }
}
