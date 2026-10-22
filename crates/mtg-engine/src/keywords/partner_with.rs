//! Partner with (CR 702.124): two commanders that name each other pair, which `CardDefinition::partners_with` reads
//! when the format checks a seat (`format/commander.rs`); and "When this permanent enters, target player may put
//! [name] into their hand from their library, then shuffle".

use mtg_cards::ability::Ability;
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_cards::trigger::TriggerPattern;

use super::KeywordRules;

/// The rules of partner with.
#[derive(Debug)]
pub(super) struct PartnerWith;

impl KeywordRules for PartnerWith {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::PartnerWith(name) = keyword else {
            return Vec::new();
        };
        let player = || Ref::Target(0);
        vec![Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector::this(),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: vec![TargetSpec {
                what: Targetable::Player,
                bounds: Bounds::default(),
                divide: None,
            }],
            effects: vec![
                Effect::May {
                    who: player(),
                    effects: vec![
                        Effect::Search {
                            who: player(),
                            zone: ZoneKind::Library,
                            filter: Selector(vec![Pred::Named(name.clone())]),
                            count: Bounds::default(),
                            reveal: false,
                            bind: "partner".into(),
                        },
                        Effect::Move {
                            what: Ref::Bound("partner".into()),
                            to: Destination::Hand,
                        },
                    ],
                },
                Effect::Shuffle { who: player() },
            ],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: partner with adds one triggered ability that waits for its source to enter the battlefield and lets a target player put the named card from their library into their hand, then shuffle; any other keyword adds none.
    use super::*;

    #[test]
    fn it_lets_a_target_player_fetch_the_named_card_and_shuffle() {
        let written: Ability = serde_json::from_str(
            r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[
                {"May":{"who":{"Target":0},"effects":[
                    {"Search":{"who":{"Target":0},"filter":[{"Named":"Pir, Imaginative Rascal"}],"as":"partner"}},
                    {"Move":{"what":{"Bound":"partner"},"to":"Hand"}}]}},
                {"Shuffle":{"who":{"Target":0}}}]}}"#,
        )
        .unwrap();
        let keyword = Keyword::PartnerWith("Pir, Imaginative Rascal".into());
        assert_eq!(PartnerWith.adds_abilities(&keyword), [written]);
        assert!(PartnerWith.adds_abilities(&Keyword::Partner).is_empty());
    }
}
