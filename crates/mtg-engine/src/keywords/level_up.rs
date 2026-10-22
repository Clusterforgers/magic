//! Level up (CR 702.87): "[Cost]: Put a level counter on this permanent. Activate only as a sorcery." The level
//! brackets of the card are static and activated abilities conditioned on the level counters (`docs/cards-dsl.md`).

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::Cost;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::value::{Amount, CounterKind};

use super::KeywordRules;

/// The rules of level up.
#[derive(Debug)]
pub(super) struct LevelUp;

impl KeywordRules for LevelUp {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::LevelUp(cost) = keyword else {
            return Vec::new();
        };
        vec![put_a_counter(
            cost.clone(),
            CounterKind::Other("level".into()),
        )]
    }
}

/// "[Cost]: Put a counter on this permanent. Activate only as a sorcery." (level up and outlast share it).
pub(super) fn put_a_counter(cost: Cost, counter: CounterKind) -> Ability {
    Ability::Activated {
        cost,
        timing: Timing::Sorcery,
        condition: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![Effect::PutCounters {
            on: Ref::This,
            counter,
            count: Amount::one(),
        }],
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Level up adds one sorcery-speed activated ability that costs its level up cost and puts a level counter on its source, whatever the cost, and no other keyword adds it.
    use mtg_cards::ability::Ability;
    use rstest::rstest;

    use super::*;

    fn written(cost: &str) -> Ability {
        serde_json::from_str(&format!(
            r#"{{"Activated":{{"cost":[{{"Mana":"{cost}"}}],"timing":"Sorcery","effects":[{{"PutCounters":{{"on":"This","counter":{{"Other":"level"}}}}}}]}}}}"#
        ))
        .unwrap()
    }

    #[rstest]
    #[case::one("{1}")]
    #[case::colored("{W}")]
    #[case::mixed("{2}{U}")]
    #[case::free("{0}")]
    fn level_up_puts_a_level_counter_for_its_cost(#[case] cost: &str) {
        let keyword = Keyword::LevelUp(Cost::mana(cost.parse().unwrap()));
        assert_eq!(LevelUp.adds_abilities(&keyword), [written(cost)]);
    }

    #[rstest]
    #[case::flying(Keyword::Flying)]
    #[case::outlast(Keyword::Outlast(Cost::mana("{1}".parse().unwrap())))]
    #[case::crew(Keyword::Crew(2))]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(LevelUp.adds_abilities(&keyword), []);
    }
}
