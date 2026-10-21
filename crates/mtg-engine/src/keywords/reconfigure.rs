//! Reconfigure (CR 702.151): two activated abilities, "[Cost]: Attach this permanent to another target creature you
//! control" and "[Cost]: Unattach this permanent", each only as a sorcery and the first only while it is attached to no
//! creature and the second only while it is attached to one, and "while this permanent is attached to a creature, it
//! isn't a creature", a layer 4 effect.

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Modification;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::value::{Amount, Cmp, Condition};
use mtg_core::card::CardType;

use super::KeywordRules;
use super::equip::attach_to_a_creature_you_control;

/// The rules of reconfigure.
#[derive(Debug)]
pub(super) struct Reconfigure;

impl KeywordRules for Reconfigure {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Reconfigure(cost) = keyword else {
            return Vec::new();
        };
        let attached = attached_to_a_creature();
        vec![
            attach_to_a_creature_you_control(
                cost,
                true,
                Some(Condition::Not(Box::new(attached.clone()))),
            ),
            Ability::Activated {
                cost: cost.clone(),
                timing: Timing::Sorcery,
                condition: Some(attached.clone()),
                limit: None,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: vec![Effect::Unattach { what: Ref::This }],
            },
            Ability::Static {
                affected: Ref::This,
                mods: vec![Modification::RemoveType(CardType::Creature)],
                condition: Some(attached),
                defining: false,
                from: ZoneKind::Battlefield,
            },
        ]
    }
}

/// The source is attached to a creature: the permanent it is attached to, if any, is a creature.
fn attached_to_a_creature() -> Condition {
    Condition::Compare {
        left: Amount::Count(Selector(vec![
            Pred::Is(Ref::Attached),
            Pred::Type(CardType::Creature),
        ])),
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Reconfigure adds the attach ability for another creature while the source is attached to no creature, the unattach ability while it is attached to one, both at sorcery speed for the reconfigure cost, and the static ability that makes the source no creature while it is attached to one; no other keyword adds them.
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;

    fn written(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    const ATTACHED: &str = r#"{"Compare":{"left":{"Count":[{"Is":"Attached"},{"Type":"Creature"}]},"op":"Ge","right":1}}"#;

    #[rstest]
    #[case::two("{2}")]
    #[case::red("{R}")]
    #[case::five("{5}")]
    fn reconfigure_adds_two_activated_abilities_and_one_static_ability(#[case] cost: &str) {
        let keyword = Keyword::Reconfigure(Cost::mana(cost.parse().unwrap()));
        let added = Reconfigure.adds_abilities(&keyword);
        let expected = [
            format!(
                r#"{{"Activated":{{"cost":"{cost}","timing":"Sorcery","condition":{{"Not":{ATTACHED}}},"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"You"}},"Other"]}}}}],"effects":[{{"Attach":{{"what":"This","to":{{"Target":0}}}}}}]}}}}"#
            ),
            format!(
                r#"{{"Activated":{{"cost":"{cost}","timing":"Sorcery","condition":{ATTACHED},"effects":[{{"Unattach":{{"what":"This"}}}}]}}}}"#
            ),
            format!(
                r#"{{"Static":{{"affected":"This","mods":[{{"RemoveType":"Creature"}}],"condition":{ATTACHED}}}}}"#
            ),
        ];
        assert_eq!(added, expected.map(|json| written(&json)));
    }

    #[rstest]
    #[case::equip(Keyword::Equip(Cost::mana("{1}".parse().unwrap())))]
    #[case::flying(Keyword::Flying)]
    fn no_other_keyword_adds_them(#[case] keyword: Keyword) {
        assert_eq!(Reconfigure.adds_abilities(&keyword), []);
    }
}
