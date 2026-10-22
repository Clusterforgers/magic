//! Outlast (CR 702.107): "[Cost], {T}: Put a +1/+1 counter on this creature. Activate only as a sorcery." The source
//! taps itself, so a creature that came under its controller's control this turn cannot outlast (CR 302.6).

use mtg_cards::ability::Ability;
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;

use super::KeywordRules;
use super::level_up::put_a_counter;

/// The rules of outlast.
#[derive(Debug)]
pub(super) struct Outlast;

impl KeywordRules for Outlast {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Outlast(cost) = keyword else {
            return Vec::new();
        };
        let mut items = cost.0.clone();
        items.push(CostItem::Tap);
        vec![put_a_counter(Cost(items), CounterKind::PlusOnePlusOne)]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Outlast adds one sorcery-speed activated ability that costs its outlast cost and the tap of its source and puts a +1/+1 counter on it, whatever the cost, and no other keyword adds it.
    use mtg_cards::ability::Ability;
    use rstest::rstest;

    use super::*;

    fn written(cost: &str) -> Ability {
        serde_json::from_str(&format!(
            r#"{{"Activated":{{"cost":[{{"Mana":"{cost}"}},"Tap"],"timing":"Sorcery","effects":[{{"PutCounters":{{"on":"This","counter":"+1/+1"}}}}]}}}}"#
        ))
        .unwrap()
    }

    #[rstest]
    #[case::one("{1}")]
    #[case::colored("{W}")]
    #[case::mixed("{1}{W}")]
    fn outlast_taps_and_puts_a_counter_for_its_cost(#[case] cost: &str) {
        let keyword = Keyword::Outlast(Cost::mana(cost.parse().unwrap()));
        assert_eq!(Outlast.adds_abilities(&keyword), [written(cost)]);
    }

    #[rstest]
    #[case::flying(Keyword::Flying)]
    #[case::level_up(Keyword::LevelUp(Cost::mana("{1}".parse().unwrap())))]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Outlast.adds_abilities(&keyword), []);
    }
}
