//! Equip (CR 702.6): "[Cost]: Attach this permanent to target creature you control. Activate only as a sorcery."

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::Cost;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of equip.
#[derive(Debug)]
pub(super) struct Equip;

impl KeywordRules for Equip {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Equip(cost) = keyword else {
            return Vec::new();
        };
        vec![attach_to_a_creature_you_control(cost, false)]
    }
}

/// "[Cost]: Attach this permanent to target creature you control. Activate only as a sorcery." With `another`, the
/// creature is another one than the permanent itself (reconfigure, CR 702.151a).
pub(super) fn attach_to_a_creature_you_control(cost: &Cost, another: bool) -> Ability {
    let mut host = vec![Pred::Type(CardType::Creature), Pred::ControlledBy(Ref::You)];
    if another {
        host.push(Pred::Other);
    }
    Ability::Activated {
        cost: cost.clone(),
        timing: Timing::Sorcery,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: vec![TargetSpec {
            what: Targetable::Object(Selector(host)),
            bounds: Bounds::default(),
            divide: None,
        }],
        effects: vec![Effect::Attach {
            what: Ref::This,
            to: Ref::Target(0),
        }],
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Equip adds one activated ability that costs its equip cost, may be activated only as a sorcery and attaches its source to the one creature its controller controls that it targets, the attach ability that reconfigure shares leaves the source out of the creatures it may target, and no other keyword adds it.
    use mtg_cards::ability::Ability;
    use rstest::rstest;

    use super::*;

    fn written(cost: &str, targets: &str) -> Ability {
        serde_json::from_str(&format!(
            r#"{{"Activated":{{"cost":"{cost}","timing":"Sorcery","targets":[{{"what":{{"Object":[{targets}]}}}}],"effects":[{{"Attach":{{"what":"This","to":{{"Target":0}}}}}}]}}}}"#
        ))
        .unwrap()
    }

    const YOURS: &str = r#"{"Type":"Creature"},{"ControlledBy":"You"}"#;

    #[rstest]
    #[case::one("{1}")]
    #[case::five("{5}")]
    #[case::colored("{R}")]
    #[case::mixed("{2}{B}{B}")]
    fn equip_attaches_the_source_to_a_creature_you_control_at_sorcery_speed(#[case] cost: &str) {
        let keyword = Keyword::Equip(Cost::mana(cost.parse().unwrap()));
        assert_eq!(Equip.adds_abilities(&keyword), [written(cost, YOURS)]);
    }

    #[test]
    fn another_creature_is_every_creature_you_control_but_the_source() {
        let cost = Cost::mana("{2}".parse().unwrap());
        assert_eq!(
            attach_to_a_creature_you_control(&cost, true),
            written("{2}", &format!(r#"{YOURS},"Other""#))
        );
    }

    #[rstest]
    #[case::flying(Keyword::Flying)]
    #[case::ward(Keyword::Ward(Cost::mana("{1}".parse().unwrap())))]
    #[case::enchant(Keyword::Enchant(mtg_cards::refs::Selector::default()))]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Equip.adds_abilities(&keyword), []);
    }
}
