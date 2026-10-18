//! A bot that always picks uniformly at random among the legal options it is offered.

use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaType, PHYREXIAN_LIFE, Resolution};
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
    partial_payments: bool,
    symbol_choices: bool,
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
    ///
    /// Its choices stay fixed (cancel, automatic and exact payments) because seeded fixtures, such as the
    /// transcripts the web tests pin, replay its games; new kinds of answer are opt-in.
    pub fn new(seed: u64, stream: u64) -> Self {
        RandomBot {
            rng: GameRng::substream(seed, stream),
            partial_payments: false,
            symbol_choices: false,
        }
    }

    /// Also pays by choosing part of a payment and letting the engine finish it (`PayChoice::Complete`).
    #[must_use]
    pub const fn with_partial_payments(mut self) -> Self {
        self.partial_payments = true;
        self
    }

    /// Also chooses how to pay hybrid and Phyrexian symbols itself, paying life in place of mana at random.
    ///
    /// Without it a cost with such a symbol is paid automatically or cancelled, and a plain cost is
    /// answered exactly as before.
    #[must_use]
    pub const fn with_symbol_choices(mut self) -> Self {
        self.symbol_choices = true;
        self
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
        let manual = if options.cost.is_plain() {
            self.random_payment(&options.cost, options)
        } else if self.symbol_choices {
            self.random_symbol_payment(options)
        } else {
            None
        };
        let mut choices = Vec::new();
        if options.can_cancel {
            choices.push(PayChoice::Cancel);
        }
        if options.auto_available {
            choices.push(PayChoice::Auto);
        }
        if let Some(payment) = manual {
            if self.partial_payments {
                choices.push(PayChoice::Complete(self.some_of(&payment, options.life)));
            }
            choices.push(PayChoice::Manual(payment));
        }
        // A payment is always possible when a cast is offered, so only a cancel-less decision can end up empty.
        if choices.is_empty() {
            return PayChoice::Auto;
        }
        let pick = self.rng.index(choices.len());
        choices.swap_remove(pick)
    }

    /// A random part of `payment` (possibly all or none of it), the way a player who has paid part by hand leaves it.
    ///
    /// `life` is the payer's life total, which decides how much of the life paid can be left to the engine.
    fn some_of(&mut self, payment: &Payment, life: i32) -> Payment {
        let mut keep =
            |len: usize| -> Vec<bool> { (0..len).map(|_| self.rng.below(2) == 0).collect() };
        let pool_keep = keep(payment.pool.len());
        let tap_keep = keep(payment.taps.len());
        Payment {
            pool: payment
                .pool
                .iter()
                .zip(pool_keep)
                .filter_map(|(m, k)| k.then_some(*m))
                .collect(),
            taps: payment
                .taps
                .iter()
                .zip(tap_keep)
                .filter_map(|(t, k)| k.then_some(*t))
                .collect(),
            life: self.some_life(payment.life, life),
        }
    }

    /// A random part of the `paid` life, in steps of one symbol; all of it when taking the rest would be the engine's call to make alone.
    ///
    /// A completion adds life only where it leaves some, so a payment that takes the last life keeps it whole.
    fn some_life(&mut self, paid: u32, life: i32) -> u32 {
        if paid == 0 || i64::from(life) - i64::from(paid) < 1 {
            return paid;
        }
        let steps = u64::from(paid / PHYREXIAN_LIFE);
        u32::try_from(self.rng.below(steps + 1)).unwrap_or(0) * PHYREXIAN_LIFE
    }

    /// A random exact payment of a cost with hybrid or Phyrexian symbols: a random way to pay each, life included.
    fn random_symbol_payment(&mut self, options: &PayOptions) -> Option<Payment> {
        let mut ways: Vec<Resolution> = options
            .cost
            .resolutions()
            .into_iter()
            .filter(|r| r.life <= options.max_life)
            .collect();
        self.rng.shuffle(&mut ways);
        ways.into_iter().find_map(|way| {
            let mut payment = self.random_payment(&way.cost, options)?;
            payment.life = way.life;
            Some(payment)
        })
    }

    /// A random exact payment of the plain `cost`: required colors first, then random mana for the generic part.
    fn random_payment(&mut self, cost: &ManaCost, options: &PayOptions) -> Option<Payment> {
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
        // A permanent is tapped once, so choosing one of its abilities drops the others from the supply.
        let take = |supply: &mut Vec<Supply>, pos: usize, chosen: &mut Vec<Supply>| {
            let item = supply.remove(pos);
            if let Supply::Tap(source, _) = item {
                supply.retain(
                    |s| !matches!(s, Supply::Tap(other, _) if other.object == source.object),
                );
            }
            chosen.push(item);
        };
        for mana in ManaType::ALL {
            for _ in 0..cost.required(mana) {
                let pos = supply.iter().position(|s| s.mana() == mana)?;
                take(&mut supply, pos, &mut chosen);
            }
        }
        for _ in 0..cost.generic() {
            if supply.is_empty() {
                return None;
            }
            take(&mut supply, 0, &mut chosen);
        }
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
    use crate::cards::{
        bear, colorless_hybrid_elf, colorless_phyrexian_elf, double_phyrexian_ogre, elf, forest,
        free_creature, giant, hybrid_bear, hybrid_phyrexian_elf, island, mono_hybrid_bear,
        phyrexian_bear, swamp, wastes,
    };
    use crate::scenario::{Scenario, ready};
    use mtg_engine::turn::Step;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn bot_answers_are_always_accepted(seed in any::<u64>(), step in 0usize..4, partial in any::<bool>()) {
            let step = [Step::Main1, Step::DeclareAttackers, Step::Main2, Step::Cleanup][step];
            let mut engine = Scenario::new(3)
                .at(2, 0, step)
                .hand(0, std::iter::repeat_n(forest(), 10).chain([bear(), bear()]))
                .battlefield(0, [ready(forest()), ready(forest()), ready(free_creature("A", 2, 2)), ready(free_creature("B", 1, 3))])
                .battlefield(1, [ready(free_creature("C", 2, 2)), ready(free_creature("D", 3, 3))])
                .build()
                .unwrap();
            let mut bot = RandomBot::new(seed, 0);
            if partial {
                bot = bot.with_partial_payments();
            }
            for _ in 0..200 {
                let Some(decision) = engine.decision().cloned() else { break };
                let answer = bot.choose(&decision);
                prop_assert!(engine.apply(answer).is_ok(), "bot gave an illegal answer to {decision:?}");
            }
        }

        #[test]
        fn bot_answers_to_hybrid_and_phyrexian_costs_are_always_accepted(
            seed in any::<u64>(), partial in any::<bool>(), symbol_choices in any::<bool>(), life in 1i32..9
        ) {
            let hand = [hybrid_bear(), mono_hybrid_bear(), phyrexian_bear(), double_phyrexian_ogre(), hybrid_phyrexian_elf(), colorless_hybrid_elf(), colorless_phyrexian_elf()];
            let mut engine = Scenario::new(2)
                .hand(0, hand.into_iter().cycle().take(14))
                .battlefield(0, [ready(forest()), ready(island()), ready(swamp()), ready(wastes())])
                .life(0, life)
                .build()
                .unwrap();
            let mut bot = RandomBot::new(seed, 0);
            if partial {
                bot = bot.with_partial_payments();
            }
            if symbol_choices {
                bot = bot.with_symbol_choices();
            }
            for _ in 0..120 {
                let Some(decision) = engine.decision().cloned() else { break };
                let answer = bot.choose(&decision);
                prop_assert!(engine.apply(answer).is_ok(), "bot gave an illegal answer to {decision:?}");
            }
        }
    }

    #[test]
    fn plain_costs_get_the_same_answers_whether_or_not_symbols_are_chosen() {
        // Seeded fixtures replay the plain bot, so the opt-in must not change a single draw of it.
        let build = || {
            Scenario::new(2)
                .hand(0, [bear(), giant(), elf()])
                .battlefield(0, std::iter::repeat_n(ready(forest()), 4))
                .build()
                .unwrap()
        };
        for seed in 0..40 {
            let (mut first, mut second) = (build(), build());
            let mut plain = RandomBot::new(seed, 0);
            let mut choosing = RandomBot::new(seed, 0).with_symbol_choices();
            for _ in 0..60 {
                let (Some(a), Some(b)) = (first.decision().cloned(), second.decision().cloned())
                else {
                    break;
                };
                let (answer_a, answer_b) = (plain.choose(&a), choosing.choose(&b));
                assert_eq!(answer_a, answer_b, "seed {seed}");
                first.apply(answer_a).unwrap();
                second.apply(answer_b).unwrap();
            }
        }
    }
}
