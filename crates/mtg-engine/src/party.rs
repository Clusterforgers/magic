//! The party (CR 700.8): up to one each of Cleric, Rogue, Warrior and Wizard among a player's creatures, each creature
//! filling one role it has, chosen so that the party is as large as it can be.

use mtg_core::card::Subtype;

use crate::characteristics::Characteristics;

/// The roles of a party in bit order: bit 0 is a Cleric, bit 3 a Wizard.
const ROLES: [&str; 4] = ["Cleric", "Rogue", "Warrior", "Wizard"];

/// The roles a creature can fill as a bit set: the creature types it has now, every role for a changeling.
pub(crate) fn roles(chars: &Characteristics, printed: &[Subtype]) -> u8 {
    ROLES
        .iter()
        .enumerate()
        .filter(|(_, name)| chars.has_subtype(printed, &Subtype((**name).to_owned())))
        .fold(0, |mask, (bit, _)| mask | (1 << bit))
}

/// The size of the largest party the creatures can fill, each given as the set of roles it has.
pub(crate) fn size(masks: &[u8]) -> u32 {
    // Bit `set` of `reach` says the role set `set` can be filled by distinct creatures among those seen so far.
    let mut reach: u16 = 1;
    for mask in masks {
        let mut next = reach;
        for set in 0..16u16 {
            if reach & (1 << set) == 0 {
                continue;
            }
            for role in 0..4 {
                if u16::from(*mask) & (1 << role) != 0 && set & (1 << role) == 0 {
                    next |= 1 << (set | (1 << role));
                }
            }
        }
        reach = next;
    }
    (0..16u16)
        .filter(|set| reach & (1 << set) != 0)
        .map(u16::count_ones)
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    //! Goal: the party is the most roles distinct creatures can fill, each creature filling one: none for no roles, one creature fills one role however many it has, a creature with every role (a changeling) fills any gap, and an assignment that gives a many-role creature the wrong role does not lose a member.
    use rstest::rstest;

    use super::*;

    const CLERIC: u8 = 1;
    const ROGUE: u8 = 2;
    const WARRIOR: u8 = 4;
    const WIZARD: u8 = 8;
    const ALL: u8 = 15;

    #[rstest]
    #[case::nobody(&[], 0)]
    #[case::no_roles(&[0, 0], 0)]
    #[case::one_cleric(&[CLERIC], 1)]
    #[case::two_clerics_fill_one_role(&[CLERIC, CLERIC], 1)]
    #[case::one_creature_with_every_role(&[ALL], 1)]
    #[case::two_roles(&[CLERIC, WIZARD], 2)]
    #[case::three_roles(&[CLERIC, ROGUE | WARRIOR, WIZARD], 3)]
    #[case::a_changeling_fills_the_gap(&[CLERIC, ROGUE, WARRIOR, ALL], 4)]
    #[case::the_changeling_alone_with_three(&[CLERIC, ROGUE, ALL], 3)]
    #[case::a_full_party(&[CLERIC, ROGUE, WARRIOR, WIZARD], 4)]
    #[case::many_roles_must_not_take_the_only_role_of_another(&[CLERIC | ROGUE, CLERIC], 2)]
    #[case::four_creatures_with_two_roles_each(&[CLERIC | ROGUE, ROGUE | WARRIOR, WARRIOR | WIZARD, WIZARD | CLERIC], 4)]
    #[case::more_creatures_than_roles(&[ALL, ALL, ALL, ALL, ALL, ALL], 4)]
    fn the_party_is_the_most_roles_distinct_creatures_fill(
        #[case] masks: &[u8],
        #[case] expected: u32,
    ) {
        assert_eq!(size(masks), expected);
    }

    #[rstest]
    #[case::cleric("Cleric", CLERIC)]
    #[case::rogue("Rogue", ROGUE)]
    #[case::warrior("Warrior", WARRIOR)]
    #[case::wizard("Wizard", WIZARD)]
    #[case::elf("Elf", 0)]
    fn a_creature_type_is_its_role(#[case] subtype: &str, #[case] expected: u8) {
        let mut chars = Characteristics::empty();
        chars.subtypes.push(Subtype(subtype.to_owned()));
        assert_eq!(roles(&chars, &[]), expected);
    }

    #[test]
    fn a_changeling_has_every_role() {
        let mut chars = Characteristics::empty();
        chars.all_creature_types = true;
        assert_eq!(roles(&chars, &[]), ALL);
    }

    #[test]
    fn a_printed_role_an_effect_replaced_is_lost() {
        let mut chars = Characteristics::empty();
        chars.creature_types_set = true;
        assert_eq!(roles(&chars, &[Subtype("Cleric".to_owned())]), 0);
    }
}
