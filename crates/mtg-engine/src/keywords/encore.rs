//! Encore (CR 702.141): "[Cost], Exile this card from your graveyard: For each opponent, create a token that's a copy of
//! it that attacks that opponent this turn if able. They gain haste. Sacrifice them at the beginning of the next end step.
//! Activate only as a sorcery."
//!
//! An activated ability that works from a graveyard, like embalm (`embalm.rs`): the card is exiled as a cost, so the copies
//! are made from what the card was (last known information). One effect makes the tokens, one for each opponent, and
//! names them; haste, the requirement to attack and the sacrifice at the end step work on the named tokens. The tokens
//! and the opponents are paired in order (`GameRule::MustAttackPlayers`).

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::{Effect, TokenRef};
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, GameRule, Modification};
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::Amount;

use super::KeywordRules;

/// The name the tokens are bound to.
const ENCORED: &str = "encored";

/// The rules of encore.
#[derive(Debug)]
pub(super) struct Encore;

impl KeywordRules for Encore {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Encore(cost) = keyword else {
            return Vec::new();
        };
        let mut paid = cost.0.clone();
        paid.push(CostItem::ExileSelf);
        vec![Ability::Activated {
            cost: Cost(paid),
            timing: Timing::Sorcery,
            condition: None,
            limit: None,
            from: ZoneKind::Graveyard,
            targets: Vec::new(),
            effects: vec![
                one_copy_for_each_opponent(),
                Effect::Continuous {
                    affected: encored(),
                    mods: vec![Modification::AddKeyword(Keyword::Haste)],
                    duration: Duration::UntilEndOfTurn,
                },
                Effect::Rule {
                    rule: GameRule::MustAttackPlayers {
                        who: encored(),
                        players: Ref::Opponents,
                    },
                    duration: Duration::UntilEndOfTurn,
                },
                Effect::DelayedTrigger {
                    when: TriggerPattern::StepBegins {
                        step: Step::End,
                        whose: Ref::EachPlayer,
                    },
                    once: true,
                    this_turn: false,
                    effects: vec![sacrifice_them()],
                },
            ],
        }]
    }
}

/// The tokens the ability made.
fn encored() -> Ref {
    Ref::Bound(ENCORED.to_owned())
}

/// "For each opponent, create a token that's a copy of it": the copy is of the card as it was in the graveyard.
fn one_copy_for_each_opponent() -> Effect {
    Effect::CreateToken {
        who: Ref::You,
        token: TokenRef::CopyOf {
            of: Ref::This,
            except: Vec::new(),
            no_mana_cost: false,
        },
        count: Amount::Players(Ref::Opponents),
        tapped: false,
        attacking: false,
        bind: Some(ENCORED.to_owned()),
    }
}

/// "Sacrifice them": the player sacrifices every one of the tokens they still control.
fn sacrifice_them() -> Effect {
    let them = Selector(vec![Pred::Is(encored())]);
    Effect::Sacrifice {
        who: Ref::You,
        count: Amount::Count(them.clone()),
        filter: them,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: encore adds one sorcery-speed ability from a graveyard whose cost is the keyword cost and the exile of the card, which makes a token copy for each opponent and names them, gives them haste, binds each to its opponent for the combat of the turn and sacrifices them at the next end step; no other keyword adds it.
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;

    fn mana(text: &str) -> Cost {
        Cost::mana(text.parse().unwrap())
    }

    #[rstest]
    #[case::cheap("{2}{W}")]
    #[case::mid("{3}{B}")]
    #[case::dear("{5}{U}{U}")]
    fn the_ability_exiles_the_card_for_the_cost_and_goes_through_the_four_steps(
        #[case] text: &str,
    ) {
        let added = Encore.adds_abilities(&Keyword::Encore(mana(text)));
        let [
            Ability::Activated {
                cost,
                timing,
                from,
                targets,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one activated ability: {added:?}");
        };
        assert_eq!(
            cost.0,
            [CostItem::Mana(text.parse().unwrap()), CostItem::ExileSelf]
        );
        assert_eq!(*timing, Timing::Sorcery);
        assert_eq!(*from, ZoneKind::Graveyard);
        assert_eq!(*targets, Vec::new());
        let [copies, haste, attack, sacrifice] = effects.as_slice() else {
            panic!("four effects: {effects:?}");
        };
        assert!(matches!(
            copies,
            Effect::CreateToken {
                token: TokenRef::CopyOf { of: Ref::This, no_mana_cost: false, .. },
                count: Amount::Players(Ref::Opponents),
                bind: Some(name),
                ..
            } if name == ENCORED
        ));
        assert!(matches!(
            haste,
            Effect::Continuous { affected, mods, .. }
                if *affected == encored() && *mods == [Modification::AddKeyword(Keyword::Haste)]
        ));
        assert!(matches!(
            attack,
            Effect::Rule {
                rule: GameRule::MustAttackPlayers { who, players: Ref::Opponents },
                duration: Duration::UntilEndOfTurn,
            } if *who == encored()
        ));
        assert!(matches!(
            sacrifice,
            Effect::DelayedTrigger { once: true, effects, .. }
                if *effects == [sacrifice_them()]
        ));
    }

    #[rstest]
    #[case::other_keyword(Keyword::Flying)]
    #[case::embalm(Keyword::Embalm(mana("{1}")))]
    #[case::unearth(Keyword::Unearth(mana("{1}")))]
    fn encore_adds_nothing_for_another_keyword(#[case] keyword: Keyword) {
        assert_eq!(Encore.adds_abilities(&keyword), Vec::new());
    }
}
