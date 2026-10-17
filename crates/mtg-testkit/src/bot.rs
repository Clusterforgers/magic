//! A bot that always picks uniformly at random among the legal options it is offered.

use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaType;
use mtg_engine::Decision;
use mtg_engine::decision::{
    Answer, DamageOptions, DecisionKind, MulliganChoice, PayChoice, PayOptions, PriorityAction,
    PriorityOptions,
};
use mtg_engine::mana::{ManaSource, Payment};
use mtg_engine::rng::GameRng;

/// Picks random legal answers from a seeded generator, so a bot game is reproducible from its seed.
#[derive(Debug, Clone)]
pub struct RandomBot {
    rng: GameRng,
}

/// A way to produce one mana while paying: floating mana or a tap.
enum Supply {
    Floating(ManaType),
    Tap(ManaSource, ManaType),
}

impl Supply {
    fn mana(&self) -> ManaType {
        match self {
            Supply::Floating(m) | Supply::Tap(_, m) => *m,
        }
    }
}

impl RandomBot {
    /// A bot using substream `stream` of the generator keyed by `seed`.
    pub fn new(seed: u64, stream: u64) -> Self {
        RandomBot {
            rng: GameRng::substream(seed, stream),
        }
    }

    /// Answers `decision` with a uniformly random legal choice.
    pub fn choose(&mut self, decision: &Decision) -> Answer {
        match &decision.kind {
            DecisionKind::Mulligan(options) => {
                let mulligan = options.can_mulligan && self.rng.below(2) == 0;
                Answer::Mulligan(if mulligan {
                    MulliganChoice::Mulligan
                } else {
                    MulliganChoice::Keep
                })
            }
            DecisionKind::BottomCards(options) => {
                Answer::BottomCards(self.subset(&options.hand, options.count))
            }
            DecisionKind::Discard(options) => {
                Answer::Discard(self.subset(&options.hand, options.count))
            }
            DecisionKind::Priority(options) => Answer::Priority(self.priority(options)),
            DecisionKind::PayMana(options) => Answer::PayMana(self.pay(options)),
            DecisionKind::DeclareAttackers(options) => {
                let mut attackers = Vec::new();
                for option in &options.attackers {
                    if self.rng.below(2) == 0 && !option.defenders.is_empty() {
                        let target = option.defenders[self.rng.index(option.defenders.len())];
                        attackers.push((option.object, target));
                    }
                }
                Answer::DeclareAttackers(attackers)
            }
            DecisionKind::DeclareBlockers(options) => {
                let mut blocks = Vec::new();
                for option in &options.blockers {
                    // One extra slot stands for "do not block".
                    let pick = self.rng.index(option.can_block.len() + 1);
                    if let Some(attacker) = option.can_block.get(pick) {
                        blocks.push((option.object, *attacker));
                    }
                }
                Answer::DeclareBlockers(blocks)
            }
            DecisionKind::AssignDamage(options) => Answer::AssignDamage(self.divide(options)),
        }
    }

    fn subset(&mut self, from: &[ObjectId], count: usize) -> Vec<ObjectId> {
        let mut cards = from.to_vec();
        self.rng.shuffle(&mut cards);
        cards.truncate(count);
        cards
    }

    fn priority(&mut self, options: &PriorityOptions) -> PriorityAction {
        let mut actions = vec![PriorityAction::Pass];
        actions.extend(
            options
                .playable_lands
                .iter()
                .map(|o| PriorityAction::PlayLand(*o)),
        );
        actions.extend(
            options
                .castable
                .iter()
                .map(|o| PriorityAction::CastSpell(*o)),
        );
        actions.extend(
            options
                .mana_abilities
                .iter()
                .map(|m| PriorityAction::ActivateMana(m.source)),
        );
        actions[self.rng.index(actions.len())]
    }

    fn pay(&mut self, options: &PayOptions) -> PayChoice {
        let manual = self.random_payment(options);
        let mut choices = vec![PayChoice::Cancel];
        if options.auto_available {
            choices.push(PayChoice::Auto);
        }
        choices.extend(manual.map(PayChoice::Manual));
        let pick = self.rng.index(choices.len());
        choices.swap_remove(pick)
    }

    /// A random exact payment: required colors first, then random mana for the generic part.
    fn random_payment(&mut self, options: &PayOptions) -> Option<Payment> {
        let mut supply: Vec<Supply> = options
            .sources
            .iter()
            .map(|o| Supply::Tap(o.source, o.produces))
            .collect();
        for mana in ManaType::ALL {
            supply.extend((0..options.pool.get(mana)).map(|_| Supply::Floating(mana)));
        }
        self.rng.shuffle(&mut supply);
        let mut chosen = Vec::new();
        for mana in ManaType::ALL {
            for _ in 0..options.cost.required(mana) {
                let pos = supply.iter().position(|s| s.mana() == mana)?;
                chosen.push(supply.remove(pos));
            }
        }
        let generic = usize::try_from(options.cost.generic()).ok()?;
        if supply.len() < generic {
            return None;
        }
        chosen.extend(supply.drain(..generic));
        let mut payment = Payment::default();
        for item in chosen {
            match item {
                Supply::Floating(m) => payment.pool.push(m),
                Supply::Tap(source, _) => payment.taps.push(source),
            }
        }
        Some(payment)
    }

    /// Splits combat damage randomly among blockers, one point at a time.
    fn divide(&mut self, options: &DamageOptions) -> Vec<(ObjectId, u32)> {
        let mut shares: Vec<(ObjectId, u32)> = options.blockers.iter().map(|b| (*b, 0)).collect();
        for _ in 0..options.power {
            let pick = self.rng.index(shares.len());
            shares[pick].1 += 1;
        }
        shares
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every answer the bot gives is legal for the decision, across all decision kinds.
    use super::*;
    use crate::cards::{bear, forest, free_creature};
    use crate::scenario::{Scenario, ready};
    use mtg_engine::turn::Step;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn bot_answers_are_always_accepted(seed in any::<u64>(), step in 0usize..4) {
            let step = [Step::Main1, Step::DeclareAttackers, Step::Main2, Step::Cleanup][step];
            let mut engine = Scenario::new(3)
                .at(2, 0, step)
                .hand(0, std::iter::repeat_n(forest(), 10).chain([bear(), bear()]))
                .battlefield(0, [ready(forest()), ready(forest()), ready(free_creature("A", 2, 2)), ready(free_creature("B", 1, 3))])
                .battlefield(1, [ready(free_creature("C", 2, 2)), ready(free_creature("D", 3, 3))])
                .build()
                .unwrap();
            let mut bot = RandomBot::new(seed, 0);
            for _ in 0..200 {
                let Some(decision) = engine.decision().cloned() else { break };
                let answer = bot.choose(&decision);
                prop_assert!(engine.apply(answer).is_ok(), "bot gave an illegal answer to {decision:?}");
            }
        }
    }
}
