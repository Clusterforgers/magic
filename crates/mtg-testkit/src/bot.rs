//! A bot that always picks uniformly at random among the legal options it is offered.

use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaType, PHYREXIAN_LIFE, Resolution};
use mtg_engine::Decision;
use mtg_engine::choose::{
    CardsChoice, CardsOptions, CostsChoice, CostsOptions, DivideChoice, DivideOptions, ModeChoice,
    NumberChoice, NumberOptions, OptionChoice, OrderChoice, ReplacementChoice, Target,
    TargetChoice, TriggersChoice, YesNoChoice, YesNoOptions,
};
use mtg_engine::decision::{
    Answer, DamageOptions, DecisionKind, MulliganChoice, PayChoice, PayOptions, PriorityAction,
    PriorityOptions,
};
use mtg_engine::help::{Convoked, Used, reduce, without_tapped};
use mtg_engine::mana::{ManaOption, ManaSource, Payment, check_payment};
use mtg_engine::rng::GameRng;
use mtg_engine::spend::Restricted;
use mtg_engine::turn::Step;

use crate::windows::{Mode, SeatWindows, Side, Stop};

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
    Restricted(Restricted),
    Tap(ManaSource, ManaType),
}

/// The restricted mana in the pool that the payment may spend.
fn allowed_restricted(options: &PayOptions) -> impl Iterator<Item = Restricted> + '_ {
    options
        .pool
        .restricted()
        .iter()
        .filter(|unit| options.spend.allows(&unit.only))
        .cloned()
}

impl Supply {
    fn mana(&self) -> ManaType {
        match self {
            Supply::Floating(m) | Supply::Tap(_, m) => *m,
            Supply::Restricted(unit) => unit.mana,
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

    /// Chooses the stops and the Full Control mode this bot plays a game with.
    ///
    /// The mode is `Off` in two games out of five, `NextWindow` in one and `Held` in two. Each of the eleven steps
    /// where a player gets priority holds a stop with chance one in four on each side, and half the stops are kept.
    /// This takes from the bot's own random stream, so a bot that calls it before its first answer, in a game and in
    /// the one it is compared with, plays the same.
    pub fn windows(&mut self) -> SeatWindows {
        let mode = match self.rng.below(5) {
            0 | 1 => Mode::Off,
            2 => Mode::NextWindow,
            _ => Mode::Held,
        };
        let mut stops = Vec::new();
        for side in [Side::Own, Side::Opponent] {
            for step in Step::ALL.into_iter().filter(|step| step.grants_priority()) {
                if self.rng.below(4) == 0 {
                    let keep = self.rng.below(2) == 0;
                    stops.push(Stop { side, step, keep });
                }
            }
        }
        SeatWindows { stops, mode }
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
                    // A creature a rule requires to attack always does; the others at random.
                    let required = options.required.contains(&option.object);
                    if (self.rng.below(2) == 0 || required) && !option.defenders.is_empty() {
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
                // An attacker that needs several blockers (menace) is left alone unless it got that many.
                for (attacker, needed) in &options.min_blockers {
                    let got = blocks.iter().filter(|(_, a)| a == attacker).count();
                    if got < *needed {
                        blocks.retain(|(_, a)| a != attacker);
                    }
                }
                // A creature a rule requires to block that the random choice left out: the bot blocks as the rules
                // require and no more.
                if options
                    .required
                    .iter()
                    .any(|required| !blocks.iter().any(|(blocker, _)| blocker == required))
                {
                    blocks = options.required_blocks();
                }
                Answer::DeclareBlockers(blocks)
            }
            DecisionKind::AssignDamage(options) => Answer::AssignDamage(self.divide(options)),
            DecisionKind::ChooseTargets(options) => {
                // Any number between the bounds; a bot never backs out of a cast it began.
                let spread =
                    usize::try_from(options.max.saturating_sub(options.min)).unwrap_or(0) + 1;
                let wanted = options.min as usize + self.rng.index(spread);
                Answer::ChooseTargets(TargetChoice::Targets(self.pick(&options.legal, wanted)))
            }
            DecisionKind::ChooseMode(options) => {
                // Any number of the modes offered between the bounds, in no particular order, a mode more than
                // once only where it may repeat; a bot never backs out of a cast it began.
                let offered: Vec<usize> = options.modes.iter().map(|mode| mode.index).collect();
                let spread =
                    usize::try_from(options.max.saturating_sub(options.min)).unwrap_or(0) + 1;
                let wanted = options.min as usize + self.rng.index(spread);
                let chosen = if offered.is_empty() {
                    Vec::new()
                } else if options.repeat {
                    (0..wanted)
                        .map(|_| offered[self.rng.index(offered.len())])
                        .collect()
                } else {
                    self.pick(&offered, wanted)
                };
                Answer::ChooseMode(ModeChoice::Modes(chosen))
            }
            DecisionKind::ChooseNumber(options) => self.number(options),
            DecisionKind::DivideAmount(options) => {
                Answer::DivideAmount(DivideChoice::Division(self.share_out(options)))
            }
            DecisionKind::ChooseCards(options) => self.cards(options),
            DecisionKind::OrderCards(options) => {
                // Every card once, in a random order.
                Answer::OrderCards(OrderChoice(
                    self.subset(&options.cards, options.cards.len()),
                ))
            }
            DecisionKind::YesNo(options) => self.yes_no(options),
            DecisionKind::ChooseOption(options) => {
                Answer::ChooseOption(OptionChoice(self.rng.index(options.options.len().max(1))))
            }
            DecisionKind::ChooseCosts(options) => self.costs(options),
            DecisionKind::OrderTriggers(options) => {
                Answer::OrderTriggers(TriggersChoice((0..options.triggers.len()).collect()))
            }
            DecisionKind::ChooseReplacement(_) => Answer::ChooseReplacement(ReplacementChoice(0)),
        }
    }

    fn pick<T: Copy>(&mut self, from: &[T], count: usize) -> Vec<T> {
        let mut items = from.to_vec();
        self.rng.shuffle(&mut items);
        items.truncate(count);
        items
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
        // A land face of a modal double-faced card is played as that face, the back one included.
        actions.extend(
            options
                .faces
                .iter()
                .filter(|option| option.land)
                .map(|option| PriorityAction::PlayLandFace(option.card, option.face)),
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
        actions.extend(
            options
                .abilities
                .iter()
                .map(|a| PriorityAction::Activate(*a)),
        );
        actions.extend(
            options
                .suspendable
                .iter()
                .map(|o| PriorityAction::Suspend(*o)),
        );
        actions.extend(
            options
                .foretellable
                .iter()
                .map(|o| PriorityAction::Foretell(*o)),
        );
        actions.swap_remove(self.rng.index(actions.len()))
    }

    fn pay(&mut self, options: &PayOptions) -> PayChoice {
        let manual = if !options.help.is_empty() {
            self.random_helped_payment(options)
        } else if options.cost.is_plain() {
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
        let restricted_keep = keep(payment.restricted.len());
        let tap_keep = keep(payment.taps.len());
        let convoke_keep = keep(payment.convoke.len());
        let improvise_keep = keep(payment.improvise.len());
        let delve_keep = keep(payment.delve.len());
        Payment {
            pool: payment
                .pool
                .iter()
                .zip(pool_keep)
                .filter_map(|(m, k)| k.then_some(*m))
                .collect(),
            restricted: payment
                .restricted
                .iter()
                .zip(restricted_keep)
                .filter(|(_, k)| *k)
                .map(|(unit, _)| unit.clone())
                .collect(),
            taps: payment
                .taps
                .iter()
                .zip(tap_keep)
                .filter_map(|(t, k)| k.then_some(*t))
                .collect(),
            life: self.some_life(payment.life, life),
            convoke: payment
                .convoke
                .iter()
                .zip(convoke_keep)
                .filter_map(|(c, k)| k.then_some(*c))
                .collect(),
            improvise: payment
                .improvise
                .iter()
                .zip(improvise_keep)
                .filter_map(|(c, k)| k.then_some(*c))
                .collect(),
            delve: payment
                .delve
                .iter()
                .zip(delve_keep)
                .filter_map(|(c, k)| k.then_some(*c))
                .collect(),
        }
    }

    /// A random payment that taps some of the creatures and exiles some of the cards offered to help, and pays the rest
    /// of a random way to pay the cost with mana.
    fn random_helped_payment(&mut self, options: &PayOptions) -> Option<Payment> {
        if !options.cost.is_plain() && !self.symbol_choices {
            return None;
        }
        let mut ways: Vec<Resolution> = options
            .cost
            .resolutions()
            .into_iter()
            .filter(|r| r.life <= options.max_life)
            .collect();
        self.rng.shuffle(&mut ways);
        ways.into_iter().find_map(|way| {
            let used = self.random_help(&way.cost, options);
            let rest = reduce(&way.cost, &used).ok()?;
            let mut open = options.clone();
            open.sources = without_tapped(&options.sources, &used, &Used::default());
            let mut payment = self.random_payment(&rest, &open)?;
            payment.life = way.life;
            payment.convoke = used.convoke;
            payment.improvise = used.improvise;
            payment.delve = used.delve;
            Some(payment)
        })
    }

    /// Some of the creatures and cards offered, each creature paying {1} or a colour the plain `cost` still needs.
    fn random_help(&mut self, cost: &ManaCost, options: &PayOptions) -> Used {
        let mut needs = ManaType::ALL.map(|mana| cost.required(mana));
        let mut generic = cost.generic();
        let mut creatures = options.help.convoke.clone();
        self.rng.shuffle(&mut creatures);
        let mut convoke = Vec::new();
        for creature in creatures {
            if self.rng.below(2) == 0 {
                continue;
            }
            let mut ways: Vec<Option<Color>> = Vec::new();
            if generic > 0 {
                ways.push(None);
            }
            ways.extend(
                creature
                    .colors
                    .iter()
                    .filter(|c| needs[ManaType::from(**c).index()] > 0)
                    .map(|c| Some(*c)),
            );
            if ways.is_empty() {
                continue;
            }
            let pays = ways[self.rng.index(ways.len())];
            match pays {
                Some(color) => needs[ManaType::from(color).index()] -= 1,
                None => generic -= 1,
            }
            convoke.push(Convoked {
                creature: creature.creature,
                pays,
            });
        }
        let mut artifacts = options.help.improvise.clone();
        self.rng.shuffle(&mut artifacts);
        let mut improvise = Vec::new();
        for artifact in artifacts {
            let convoked = convoke.iter().any(|c: &Convoked| c.creature == artifact);
            if generic > 0 && !convoked && self.rng.below(2) == 0 {
                generic -= 1;
                improvise.push(artifact);
            }
        }
        let most = options.help.delve.len().min(generic as usize);
        let cards = usize::try_from(self.rng.below(most as u64 + 1)).unwrap_or(0);
        Used {
            convoke,
            improvise,
            delve: options.help.delve[..cards].to_vec(),
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

    /// A random payment of the plain `cost` when some source adds several mana or is sacrificed: sources and floating
    /// mana in random order, each taken while it still helps; `None` when what was taken has a part to spare.
    fn random_planned_payment(&mut self, cost: &ManaCost, options: &PayOptions) -> Option<Payment> {
        let mut supply: Vec<(Supply, u32)> = options
            .sources
            .iter()
            .filter(|o| options.spend.allows(&o.only))
            .map(|o| (Supply::Tap(o.source, o.produces), o.count))
            .collect();
        for mana in ManaType::ALL {
            supply.extend((0..options.pool.get(mana)).map(|_| (Supply::Floating(mana), 1)));
        }
        supply.extend(allowed_restricted(options).map(|unit| (Supply::Restricted(unit), 1)));
        self.rng.shuffle(&mut supply);
        let mut needs = ManaType::ALL.map(|mana| cost.required(mana));
        let mut generic = cost.generic();
        let mut payment = Payment::default();
        let mut tapped = Vec::new();
        for (item, count) in supply {
            if needs.iter().all(|n| *n == 0) && generic == 0 {
                break;
            }
            let mana = item.mana();
            if let Supply::Tap(source, _) = item
                && tapped.contains(&source.object)
            {
                continue;
            }
            let slot = &mut needs[mana.index()];
            let used = (*slot).min(count);
            if used == 0 && generic == 0 {
                continue;
            }
            *slot -= used;
            generic = generic.saturating_sub(count - used);
            match item {
                Supply::Floating(m) => payment.pool.push(m),
                Supply::Restricted(unit) => payment.restricted.push(unit),
                Supply::Tap(source, _) => {
                    tapped.push(source.object);
                    payment.taps.push(source);
                }
            }
        }
        check_payment(
            cost,
            &options.pool,
            &options.sources,
            &[],
            options.life,
            &options.spend,
            &payment,
        )
        .ok()
        .map(|_| payment)
    }

    /// A random exact payment of the plain `cost`: required colors first, then random mana for the generic part.
    fn random_payment(&mut self, cost: &ManaCost, options: &PayOptions) -> Option<Payment> {
        if !options.sources.iter().all(ManaOption::is_simple) {
            return self.random_planned_payment(cost, options);
        }
        let mut supply: Vec<Supply> = options
            .sources
            .iter()
            .filter(|o| options.spend.allows(&o.only))
            .map(|o| Supply::Tap(o.source, o.produces))
            .collect();
        for mana in ManaType::ALL {
            supply.extend((0..options.pool.get(mana)).map(|_| Supply::Floating(mana)));
        }
        supply.extend(allowed_restricted(options).map(Supply::Restricted));
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
                Supply::Restricted(unit) => payment.restricted.push(unit),
                Supply::Tap(source, _) => payment.taps.push(source),
            }
        }
        Some(payment)
    }

    /// Yes or no at random, No when it is the only answer. The die is rolled either way, so a seed plays the same
    /// whether or not a payment can be made.
    fn yes_no(&mut self, options: &YesNoOptions) -> Answer {
        let yes = self.rng.below(2) == 0;
        Answer::YesNo(YesNoChoice(yes && !options.only_no))
    }

    /// Any cards among those offered, as many as the bounds allow. A cost that takes any number of creatures for their
    /// power (crew) is the one question with room between the bounds that a bot may not answer short, and it cannot
    /// check the power here: it takes every creature offered, as the engine asked only because together they have enough.
    fn cards(&mut self, options: &CardsOptions) -> Answer {
        if options.can_cancel && options.max > options.min {
            return Answer::ChooseCards(CardsChoice(options.from.clone()));
        }
        let spread = usize::try_from(options.max.saturating_sub(options.min)).unwrap_or(0) + 1;
        let wanted = options.min as usize + self.rng.index(spread);
        Answer::ChooseCards(CardsChoice(self.subset(&options.from, wanted)))
    }

    /// Any number in range that is allowed; with no upper bound, a few above the lowest. A bot never backs out of a
    /// cast.
    fn number(&mut self, options: &NumberOptions) -> Answer {
        let top = options.max.unwrap_or_else(|| options.min.saturating_add(3));
        let allowed: Vec<u32> = (options.min..=top)
            .filter(|number| !options.except.contains(number))
            .collect();
        let at = self.rng.index(allowed.len().max(1));
        let picked = allowed.get(at).copied().unwrap_or(options.min);
        Answer::ChooseNumber(NumberChoice::Number(picked))
    }

    /// Any of the optional costs on offer, none included.
    fn costs(&mut self, options: &CostsOptions) -> Answer {
        let indices: Vec<usize> = options.optional.iter().map(|cost| cost.index).collect();
        let wanted = self.rng.index(indices.len() + 1);
        let mut chosen = self.pick(&indices, wanted);
        chosen.sort_unstable();
        Answer::ChooseCosts(CostsChoice::Costs(chosen))
    }

    /// Shares an amount among the targets of a spell: one each, then the rest a point at a time at random.
    fn share_out(&mut self, options: &DivideOptions) -> Vec<(Target, u32)> {
        let mut shares: Vec<(Target, u32)> = options.among.iter().map(|t| (*t, 1)).collect();
        let spare = options
            .total
            .saturating_sub(u32::try_from(shares.len()).unwrap_or(u32::MAX));
        for _ in 0..if shares.is_empty() { 0 } else { spare } {
            let pick = self.rng.index(shares.len());
            shares[pick].1 += 1;
        }
        shares
    }

    /// Splits combat damage randomly among blockers, one point at a time.
    ///
    /// With trample half the divisions give every blocker its lethal damage first (when the power covers it) and
    /// leave a random part of the rest for the player or planeswalker attacked.
    fn divide(&mut self, options: &DamageOptions) -> Vec<(ObjectId, u32)> {
        let mut shares: Vec<(ObjectId, u32)> = options.blockers.iter().map(|b| (*b, 0)).collect();
        let needed: u32 = options.lethal.iter().sum();
        let past_the_blockers = options.excess.is_some() && needed <= options.power;
        let mut to_spread = options.power;
        if past_the_blockers && self.rng.below(2) == 0 {
            for (share, lethal) in shares.iter_mut().zip(&options.lethal) {
                share.1 = *lethal;
            }
            let spare = options.power - needed;
            let kept = u32::try_from(self.rng.index(spare as usize + 1)).unwrap_or(0);
            to_spread = spare - kept;
        }
        for _ in 0..to_spread {
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
