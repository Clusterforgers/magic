//! Convoke, improvise and delve (CR 702.51, CR 702.126, CR 702.66): creatures and artifacts tapped and graveyard cards
//! exiled to pay part of a spell's cost.
//!
//! The helpers pay after the total cost is fixed (CR 601.2f to 601.2h). A creature that convokes pays for {1} or for one
//! mana of a colour it is, an artifact that improvises and a card that delves pay for {1} of generic mana, so what mana
//! must pay is the plain cost with those symbols taken out. A creature or artifact taps for it whether or not it is
//! summoning sick, and it is not tapped for mana as well.

use std::collections::BTreeMap;

use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaSymbol, ManaType};
use serde::{Deserialize, Serialize};

use crate::characteristics::{Characteristics, characteristics, controller_of};
use crate::error::PaymentError;
use crate::keywords::has_keyword;
use crate::mana::ManaOption;
use crate::state::GameState;
use crate::zones::Zone;

/// A creature that may be tapped to help pay, and what it can pay besides {1}.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ConvokeOption {
    /// The creature.
    pub creature: ObjectId,
    /// The colours it is: it pays one mana of any of them.
    pub colors: Vec<Color>,
}

/// One creature tapped to help pay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Convoked {
    /// The creature.
    pub creature: ObjectId,
    /// The colour of the mana it pays for, which pays generic mana once the cost has no more of that colour; `None` means {1} of generic mana only.
    pub pays: Option<Color>,
}

/// The helpers a payment names: creatures and artifacts tapped and graveyard cards exiled.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Used {
    /// Creatures tapped, each with what it pays.
    pub convoke: Vec<Convoked>,
    /// Artifacts tapped, each paying {1}.
    pub improvise: Vec<ObjectId>,
    /// Cards exiled from the graveyard, each paying {1}.
    pub delve: Vec<ObjectId>,
}

impl Used {
    /// The helpers the three lists name.
    pub fn new(convoke: &[Convoked], improvise: &[ObjectId], delve: &[ObjectId]) -> Self {
        Used {
            convoke: convoke.to_vec(),
            improvise: improvise.to_vec(),
            delve: delve.to_vec(),
        }
    }

    /// How many creatures, artifacts and cards pay.
    pub fn count(&self) -> usize {
        self.convoke.len() + self.improvise.len() + self.delve.len()
    }

    /// Whether `object` is tapped for convoke or improvise.
    pub fn taps(&self, object: ObjectId) -> bool {
        self.convoke.iter().any(|c| c.creature == object) || self.improvise.contains(&object)
    }

    fn joined(&self, more: &Used) -> Used {
        Used {
            convoke: [self.convoke.as_slice(), more.convoke.as_slice()].concat(),
            improvise: [self.improvise.as_slice(), more.improvise.as_slice()].concat(),
            delve: [self.delve.as_slice(), more.delve.as_slice()].concat(),
        }
    }
}

/// The helpers a payment may use, owned: the creatures that could convoke, the artifacts that could improvise and the
/// cards that could delve.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Offered {
    /// Creatures that may be tapped for part of the cost, each with the colours it can pay beside {1} (convoke); left out means none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ConvokeOption>>", optional))]
    pub convoke: Vec<ConvokeOption>,
    /// Artifacts the payer controls that may be tapped, each paying {1} of the generic part of the cost (improvise); left out means none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ObjectId>>", optional))]
    pub improvise: Vec<ObjectId>,
    /// Cards in the payer's graveyard that may be exiled, each paying {1} of the generic part of the cost (delve); left out means none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ObjectId>>", optional))]
    pub delve: Vec<ObjectId>,
}

impl Offered {
    /// Whether nothing is offered.
    pub fn is_empty(&self) -> bool {
        self.convoke.is_empty() && self.improvise.is_empty() && self.delve.is_empty()
    }

    /// The offer as the payment functions read it.
    pub fn help(&self) -> Help<'_> {
        Help {
            convoke: &self.convoke,
            improvise: &self.improvise,
            delve: &self.delve,
        }
    }
}

/// The helpers a payment may use, borrowed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Help<'a> {
    /// Creatures that may be tapped.
    pub convoke: &'a [ConvokeOption],
    /// Artifacts that may be tapped.
    pub improvise: &'a [ObjectId],
    /// Cards in the graveyard that may be exiled.
    pub delve: &'a [ObjectId],
}

impl Help<'_> {
    /// No helpers: only mana pays.
    pub const NONE: Help<'static> = Help {
        convoke: &[],
        improvise: &[],
        delve: &[],
    };

    /// Whether nothing can help.
    pub fn is_empty(&self) -> bool {
        self.convoke.is_empty() && self.improvise.is_empty() && self.delve.is_empty()
    }
}

/// The untapped permanents `player` controls that `pick` takes, each as `pick` gives it, by object id.
fn untapped<T>(
    state: &GameState,
    player: PlayerId,
    pick: impl Fn(ObjectId, &Characteristics) -> Option<T>,
) -> Vec<T> {
    let mut found: Vec<(ObjectId, T)> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|&id| {
            let untapped = state
                .object(id)
                .and_then(|object| object.permanent())
                .is_some_and(|permanent| !permanent.tapped());
            if !untapped || controller_of(state, id) != Some(player) {
                return None;
            }
            let chars = characteristics(state, id)?;
            pick(id, &chars).map(|found| (id, found))
        })
        .collect();
    found.sort_by_key(|(id, _)| *id);
    found.into_iter().map(|(_, found)| found).collect()
}

/// What `player` may use to help pay for casting `spell`: the untapped creatures they control when it has convoke, the
/// untapped artifacts when it has improvise and the cards in their graveyard when it has delve (the spell itself is on
/// its way to the stack, not a card to exile).
pub(crate) fn offered(state: &GameState, player: PlayerId, spell: ObjectId) -> Offered {
    let mut offer = Offered::default();
    if has_keyword(state, spell, &Keyword::Convoke) {
        offer.convoke = untapped(state, player, |creature, chars| {
            chars.is_creature().then(|| ConvokeOption {
                creature,
                colors: chars.colors.iter().collect(),
            })
        });
    }
    if has_keyword(state, spell, &Keyword::Improvise) {
        offer.improvise = untapped(state, player, |artifact, chars| {
            chars.types.contains(CardType::Artifact).then_some(artifact)
        });
    }
    if has_keyword(state, spell, &Keyword::Delve) {
        offer.delve = state
            .zone(Zone::Graveyard(player))
            .iter()
            .copied()
            .filter(|id| *id != spell)
            .collect();
    }
    offer
}

/// Checks the helpers a payment names against the offer: each creature, artifact and card is offered once, no longer tapped or gone
/// (`unusable`), a creature pays only a colour it is, and no creature is also tapped for mana (`tapped_for_mana`).
pub(crate) fn check(
    used: &Used,
    help: Help<'_>,
    unusable: &[ObjectId],
    tapped_for_mana: &[ObjectId],
) -> Result<(), PaymentError> {
    for (at, convoked) in used.convoke.iter().enumerate() {
        let creature = convoked.creature;
        let offer = help
            .convoke
            .iter()
            .find(|o| o.creature == creature)
            .ok_or(PaymentError::NotOffered(creature))?;
        if used.convoke[..at].iter().any(|c| c.creature == creature) {
            return Err(PaymentError::HelpedTwice(creature));
        }
        if unusable.contains(&creature) {
            return Err(PaymentError::AlreadyTapped(creature));
        }
        if tapped_for_mana.contains(&creature) {
            return Err(PaymentError::ObjectTappedTwice(creature));
        }
        if let Some(color) = convoked.pays
            && !offer.colors.contains(&color)
        {
            return Err(PaymentError::WrongColor { creature, color });
        }
    }
    check_improvise(used, help, unusable, tapped_for_mana)?;
    for (at, card) in used.delve.iter().enumerate() {
        if !help.delve.contains(card) || unusable.contains(card) {
            return Err(PaymentError::NotInGraveyard(*card));
        }
        if used.delve[..at].contains(card) {
            return Err(PaymentError::HelpedTwice(*card));
        }
    }
    Ok(())
}

/// The artifacts of a payment are offered, tapped once (not also for convoke) and still untapped.
fn check_improvise(
    used: &Used,
    help: Help<'_>,
    unusable: &[ObjectId],
    tapped_for_mana: &[ObjectId],
) -> Result<(), PaymentError> {
    for (at, &artifact) in used.improvise.iter().enumerate() {
        if !help.improvise.contains(&artifact) {
            return Err(PaymentError::NotOffered(artifact));
        }
        let convoked = used.convoke.iter().any(|c| c.creature == artifact);
        if convoked || used.improvise[..at].contains(&artifact) {
            return Err(PaymentError::HelpedTwice(artifact));
        }
        if unusable.contains(&artifact) {
            return Err(PaymentError::AlreadyTapped(artifact));
        }
        if tapped_for_mana.contains(&artifact) {
            return Err(PaymentError::ObjectTappedTwice(artifact));
        }
    }
    Ok(())
}

/// What a plain cost needs of mana: each type's requirement, then the generic part.
type Left = ([u32; 6], u32);

fn left_of(cost: &ManaCost) -> Left {
    let mut needs = [0; 6];
    for mana in ManaType::ALL {
        needs[mana.index()] = cost.required(mana);
    }
    (needs, cost.generic())
}

fn cost_of((needs, generic): Left) -> ManaCost {
    let symbols = ManaType::ALL.into_iter().flat_map(|mana| {
        let symbol = mana
            .color()
            .map_or(ManaSymbol::Colorless, ManaSymbol::Colored);
        std::iter::repeat_n(symbol, needs[mana.index()] as usize)
    });
    ManaCost::from_symbols(std::iter::once(ManaSymbol::Generic(generic)).chain(symbols))
}

/// The plain `cost` with what `used` pays taken out, or why `used` pays for more than the cost has.
pub fn reduce(cost: &ManaCost, used: &Used) -> Result<ManaCost, PaymentError> {
    if used.count() == 0 {
        return Ok(cost.clone());
    }
    let (mut needs, mut generic) = left_of(cost);
    for convoked in &used.convoke {
        // Mana of a colour pays that colour first and generic mana when the cost has no more of it, as any mana does.
        let slot = match convoked.pays.map(|color| ManaType::from(color).index()) {
            Some(at) if needs[at] > 0 => &mut needs[at],
            _ => &mut generic,
        };
        *slot = slot.checked_sub(1).ok_or(PaymentError::HelpedTooMuch)?;
    }
    let generic_helpers = used.improvise.len() + used.delve.len();
    let generic_helpers = u32::try_from(generic_helpers).unwrap_or(u32::MAX);
    generic = generic
        .checked_sub(generic_helpers)
        .ok_or(PaymentError::HelpedTooMuch)?;
    Ok(cost_of((needs, generic)))
}

/// One way helpers pay for part of a plain cost: what they pay and what mana must pay besides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Variant {
    /// What mana must still pay.
    pub cost: ManaCost,
    /// The helpers beyond the ones already chosen.
    pub used: Used,
}

/// The states the search tells apart: what is still to pay and which mana sources it spared by tapping their permanent.
type Key = (Left, Vec<ObjectId>);

/// A permanent that may be tapped to help pay: what it can pay besides {1}, and whether only improvise offers it.
struct Tapper {
    object: ObjectId,
    colors: Vec<Color>,
    improvise: bool,
}

/// The permanents the search may tap: the creatures that convoke, and the artifacts that improvise besides them. An
/// artifact creature is offered by convoke, which pays at least what improvise does.
fn tappers(help: Help<'_>, kept: &Used, occupied: &[ObjectId]) -> Vec<Tapper> {
    let convoking = help.convoke.iter().map(|option| Tapper {
        object: option.creature,
        colors: option.colors.clone(),
        improvise: false,
    });
    let improvising = help
        .improvise
        .iter()
        .filter(|artifact| {
            !help
                .convoke
                .iter()
                .any(|option| option.creature == **artifact)
        })
        .map(|&object| Tapper {
            object,
            colors: Vec::new(),
            improvise: true,
        });
    convoking
        .chain(improvising)
        .filter(|tapper| !kept.taps(tapper.object) && !occupied.contains(&tapper.object))
        .collect()
}

/// What `tapper` can pay of `left`: {1} of generic mana, or one mana of a colour it is that the cost still needs.
fn ways(left: &Left, tapper: &Tapper) -> Vec<(Option<Color>, Left)> {
    let mut ways = Vec::new();
    if left.1 > 0 {
        ways.push((None, (left.0, left.1 - 1)));
    }
    for &color in &tapper.colors {
        let at = ManaType::from(color).index();
        if left.0[at] > 0 {
            let mut needs = left.0;
            needs[at] -= 1;
            ways.push((Some(color), (needs, left.1)));
        }
    }
    ways
}

/// The states `tappers` reach from `start`, each with the fewest permanents that reach it.
fn reach(
    start: Left,
    tappers: &[Tapper],
    sources: &[ManaOption],
) -> BTreeMap<Key, (usize, Vec<Convoked>)> {
    let mut reached: BTreeMap<Key, (usize, Vec<Convoked>)> = BTreeMap::new();
    reached.insert((start, Vec::new()), (0, Vec::new()));
    for tapper in tappers {
        let creature = tapper.object;
        let dork = sources.iter().any(|o| o.source.object == creature);
        let mut next = reached.clone();
        for ((left, spared), (count, chosen)) in &reached {
            for (pays, after) in ways(left, tapper) {
                let mut spared = spared.clone();
                if dork {
                    spared.push(creature);
                    spared.sort();
                }
                let mut chosen = chosen.clone();
                chosen.push(Convoked { creature, pays });
                let key = (after, spared);
                if next.get(&key).is_none_or(|(held, _)| count + 1 < *held) {
                    next.insert(key, (count + 1, chosen));
                }
            }
        }
        reached = next;
    }
    reached
}

/// Every way helpers can pay for part of the plain `cost`, fewest helpers first; the first is always "no more helpers".
///
/// `kept` is what the payment already names (the creatures, artifacts and cards are not offered again, and `cost` is
/// what is left after them), `occupied` the permanents already tapped for mana. A creature or artifact that is also a
/// mana source in `sources` is tapped for one or the other, so the states keep which of them were used. Each state is
/// reached by its fewest helpers, so a variant is a way to pay, not an order to tap in.
pub(crate) fn variants(
    cost: &ManaCost,
    help: Help<'_>,
    kept: &Used,
    occupied: &[ObjectId],
    sources: &[ManaOption],
) -> Vec<Variant> {
    if help.is_empty() {
        return vec![Variant {
            cost: cost.clone(),
            used: Used::default(),
        }];
    }
    let tappers = tappers(help, kept, occupied);
    let reached = reach(left_of(cost), &tappers, sources);
    let improvising: Vec<ObjectId> = tappers
        .iter()
        .filter(|tapper| tapper.improvise)
        .map(|tapper| tapper.object)
        .collect();
    let cards: Vec<ObjectId> = help
        .delve
        .iter()
        .copied()
        .filter(|card| !kept.delve.contains(card))
        .collect();
    let mut found: Vec<(usize, usize, Variant)> = Vec::new();
    for ((left, _), (count, chosen)) in &reached {
        let (artifacts, creatures): (Vec<Convoked>, Vec<Convoked>) = chosen
            .iter()
            .partition(|tapped| improvising.contains(&tapped.creature));
        let most = cards.len().min(left.1 as usize);
        for delved in 0..=most {
            let after = (left.0, left.1 - u32::try_from(delved).unwrap_or(0));
            found.push((
                count + delved,
                delved,
                Variant {
                    cost: cost_of(after),
                    used: Used {
                        convoke: creatures.clone(),
                        improvise: artifacts.iter().map(|tapped| tapped.creature).collect(),
                        delve: cards[..delved].to_vec(),
                    },
                },
            ));
        }
    }
    found.sort_by_key(|(total, delved, _)| (*total, *delved));
    found.into_iter().map(|(.., variant)| variant).collect()
}

/// The sources that are not permanents tapped for convoke or improvise, and what their costs may take without those
/// permanents.
pub fn without_tapped(sources: &[ManaOption], used: &Used, kept: &Used) -> Vec<ManaOption> {
    if used.convoke.is_empty()
        && used.improvise.is_empty()
        && kept.convoke.is_empty()
        && kept.improvise.is_empty()
    {
        return sources.to_vec();
    }
    let left: Vec<ManaOption> = sources
        .iter()
        .filter(|o| {
            let object = crate::mana_repeat::real(o.source.object);
            !used.taps(object) && !kept.taps(object)
        })
        .cloned()
        .collect();
    crate::mana_takes::without_taken(&left, &|id| used.taps(id) || kept.taps(id))
}

/// The two lists `kept` and `more` as one.
pub(crate) fn together(kept: &Used, more: &Used) -> Used {
    kept.joined(more)
}

#[cfg(test)]
mod tests {
    //! Goal: the helpers of a plain cost are offered fewest first, each pays only what the cost has, a creature taps for mana or for convoke and not both, and what is offered follows the keyword.
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;
    use crate::mana::ManaSource;

    fn cost(text: &str) -> ManaCost {
        ManaCost::parse(text).unwrap()
    }

    fn creature(id: u64, colors: &[Color]) -> ConvokeOption {
        ConvokeOption {
            creature: ObjectId(id),
            colors: colors.to_vec(),
        }
    }

    fn cards(ids: &[u64]) -> Vec<ObjectId> {
        ids.iter().copied().map(ObjectId).collect()
    }

    fn costs(variants: &[Variant]) -> Vec<String> {
        variants.iter().map(|v| v.cost.to_string()).collect()
    }

    #[rstest]
    #[case::nothing_offered("{2}{R}", vec![], &[], vec!["{2}{R}"])]
    #[case::a_card_pays_generic("{2}{R}", vec![], &[1], vec!["{2}{R}", "{1}{R}"])]
    #[case::cards_stop_at_the_generic_part("{1}{R}", vec![], &[1, 2, 3], vec!["{1}{R}", "{R}"])]
    #[case::a_red_creature_pays_red_or_one("{1}{R}", vec![(10, vec![Color::Red])], &[], vec!["{1}{R}", "{R}", "{1}"])]
    #[case::a_creature_pays_only_its_colour("{1}{R}", vec![(10, vec![Color::Green])], &[], vec!["{1}{R}", "{R}"])]
    #[case::a_colourless_creature_pays_one("{1}{R}", vec![(10, vec![])], &[], vec!["{1}{R}", "{R}"])]
    #[case::a_creature_and_a_card("{1}{R}", vec![(10, vec![Color::Red])], &[1], vec!["{1}{R}", "{R}", "{R}", "{1}", "{0}"])]
    fn helpers_pay_part_of_a_plain_cost_fewest_first(
        #[case] text: &str,
        #[case] creatures: Vec<(u64, Vec<Color>)>,
        #[case] graveyard: &[u64],
        #[case] expected: Vec<&str>,
    ) {
        let offer: Vec<ConvokeOption> = creatures.iter().map(|(id, c)| creature(*id, c)).collect();
        let help = Help {
            convoke: &offer,
            improvise: &[],
            delve: &cards(graveyard),
        };
        let found = variants(&cost(text), help, &Used::default(), &[], &[]);
        let mut got = costs(&found);
        // The order within one number of helpers is not part of the contract.
        let counts: Vec<usize> = found.iter().map(|v| v.used.count()).collect();
        assert!(
            counts.windows(2).all(|pair| pair[0] <= pair[1]),
            "{counts:?}"
        );
        got.sort();
        let mut want: Vec<String> = expected.iter().map(ToString::to_string).collect();
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn a_creature_that_is_also_a_mana_source_is_used_for_one_of_them() {
        let source = ManaOption::new(
            ManaSource {
                object: ObjectId(10),
                ability: 0,
            },
            ManaType::Green,
        );
        let offer = [creature(10, &[Color::Green]), creature(11, &[Color::Green])];
        let help = Help {
            convoke: &offer,
            improvise: &[],
            delve: &[],
        };
        let found = variants(&cost("{G}{G}"), help, &Used::default(), &[], &[source]);
        // {G}{G} with two green creatures: the dork or the other one stays untapped, and both states are told apart.
        let paid_two: Vec<&Variant> = found.iter().filter(|v| v.used.count() == 2).collect();
        assert_eq!(paid_two.len(), 1);
        assert!(paid_two[0].used.taps(ObjectId(10)));
        let paid_one: Vec<&Variant> = found.iter().filter(|v| v.used.count() == 1).collect();
        assert_eq!(paid_one.len(), 2);
    }

    #[rstest]
    #[case::an_artifact_pays_generic("{2}{R}", vec![], &[1], vec!["{2}{R}", "{1}{R}"])]
    #[case::artifacts_stop_at_the_generic_part("{1}{R}", vec![], &[1, 2, 3], vec!["{1}{R}", "{R}"])]
    #[case::artifacts_never_pay_a_colour("{R}", vec![], &[1], vec!["{R}"])]
    #[case::an_artifact_creature_is_offered_once("{1}{R}", vec![(10, vec![Color::Red])], &[10], vec!["{1}{R}", "{R}", "{1}"])]
    #[case::a_creature_and_an_artifact("{1}{R}", vec![(10, vec![Color::Red])], &[11], vec!["{1}{R}", "{R}", "{1}", "{0}"])]
    #[case::artifacts_and_cards_share_the_generic_part("{2}", vec![], &[1, 2], vec!["{2}", "{1}", "{0}"])]
    fn artifacts_that_improvise_pay_the_generic_part_of_a_plain_cost_fewest_first(
        #[case] text: &str,
        #[case] creatures: Vec<(u64, Vec<Color>)>,
        #[case] artifacts: &[u64],
        #[case] expected: Vec<&str>,
    ) {
        let offer: Vec<ConvokeOption> = creatures.iter().map(|(id, c)| creature(*id, c)).collect();
        let help = Help {
            convoke: &offer,
            improvise: &cards(artifacts),
            delve: &[],
        };
        let found = variants(&cost(text), help, &Used::default(), &[], &[]);
        let counts: Vec<usize> = found.iter().map(|v| v.used.count()).collect();
        assert!(
            counts.windows(2).all(|pair| pair[0] <= pair[1]),
            "{counts:?}"
        );
        let mut got = costs(&found);
        got.sort();
        let mut want: Vec<String> = expected.iter().map(ToString::to_string).collect();
        want.sort();
        assert_eq!(got, want);
        // An artifact creature that convokes is named under convoke, and never twice.
        for variant in &found {
            let convoked: Vec<ObjectId> = variant.used.convoke.iter().map(|c| c.creature).collect();
            assert!(variant.used.improvise.iter().all(|a| !convoked.contains(a)));
        }
    }

    #[test]
    fn an_artifact_that_is_also_a_mana_source_improvises_or_taps_for_mana_and_not_both() {
        let source = ManaOption::new(
            ManaSource {
                object: ObjectId(10),
                ability: 0,
            },
            ManaType::Colorless,
        );
        let help = Help {
            convoke: &[],
            improvise: &cards(&[10, 11]),
            delve: &[],
        };
        let found = variants(&cost("{2}"), help, &Used::default(), &[], &[source]);
        // Both artifacts improvise (the source stays untapped for nothing), or one of them does and the other is a source.
        let both: Vec<&Variant> = found.iter().filter(|v| v.used.count() == 2).collect();
        assert_eq!(both.len(), 1);
        assert!(both[0].used.taps(ObjectId(10)) && both[0].used.taps(ObjectId(11)));
        let one: Vec<&Variant> = found.iter().filter(|v| v.used.count() == 1).collect();
        assert_eq!(one.len(), 2, "the source or the plain artifact improvises");
    }

    #[rstest]
    #[case::one_artifact(&[1], &[], Some("{R}"))]
    #[case::two_artifacts_for_one_generic(&[1, 2], &[], None)]
    #[case::an_artifact_and_a_card_for_one_generic(&[1], &[7], None)]
    #[case::an_artifact_is_not_a_colour(&[], &[], Some("{1}{R}"))]
    fn reducing_a_cost_takes_the_artifacts_off_the_generic_part(
        #[case] improvise: &[u64],
        #[case] delve: &[u64],
        #[case] expected: Option<&str>,
    ) {
        let used = Used {
            improvise: cards(improvise),
            delve: cards(delve),
            ..Used::default()
        };
        let reduced = reduce(&cost("{1}{R}"), &used).ok().map(|c| c.to_string());
        assert_eq!(reduced.as_deref(), expected);
    }

    #[rstest]
    #[case::fine(&[1], &[], &[], Ok(()))]
    #[case::not_offered(&[9], &[], &[], Err(PaymentError::NotOffered(ObjectId(9))))]
    #[case::twice(&[1, 1], &[], &[], Err(PaymentError::HelpedTwice(ObjectId(1))))]
    #[case::tapped(&[1], &[1], &[], Err(PaymentError::AlreadyTapped(ObjectId(1))))]
    #[case::also_tapped_for_mana(&[1], &[], &[1], Err(PaymentError::ObjectTappedTwice(ObjectId(1))))]
    fn the_artifacts_named_are_checked_against_the_offer(
        #[case] improvise: &[u64],
        #[case] unusable: &[u64],
        #[case] for_mana: &[u64],
        #[case] expected: Result<(), PaymentError>,
    ) {
        let help = Help {
            convoke: &[],
            improvise: &cards(&[1]),
            delve: &[],
        };
        let used = Used {
            improvise: cards(improvise),
            ..Used::default()
        };
        assert_eq!(
            check(&used, help, &cards(unusable), &cards(for_mana)),
            expected
        );
    }

    #[test]
    fn an_artifact_creature_named_for_convoke_and_improvise_helps_twice() {
        let offer = [creature(1, &[Color::Red])];
        let help = Help {
            convoke: &offer,
            improvise: &cards(&[1]),
            delve: &[],
        };
        let used = Used {
            convoke: vec![Convoked {
                creature: ObjectId(1),
                pays: None,
            }],
            improvise: cards(&[1]),
            ..Used::default()
        };
        assert_eq!(
            check(&used, help, &[], &[]),
            Err(PaymentError::HelpedTwice(ObjectId(1)))
        );
    }

    #[rstest]
    #[case::exact(&[(1, None)], &[], Some("{R}"))]
    #[case::a_colour(&[(1, Some(Color::Red))], &[], Some("{1}"))]
    #[case::a_colour_the_cost_lacks_pays_generic(&[(1, Some(Color::Green))], &[], Some("{R}"))]
    #[case::the_colour_twice(&[(1, Some(Color::Red)), (2, Some(Color::Red))], &[], Some("{0}"))]
    #[case::the_colour_three_times(&[(1, Some(Color::Red)), (2, Some(Color::Red)), (3, Some(Color::Red))], &[], None)]
    #[case::two_for_one_generic(&[(1, None), (2, None)], &[], None)]
    #[case::a_card_and_a_creature(&[(1, None)], &[7], None)]
    #[case::more_cards_than_generic(&[], &[7, 8], None)]
    #[case::one_card(&[], &[7], Some("{R}"))]
    fn reducing_a_cost_refuses_what_pays_more_than_it_has(
        #[case] convoke: &[(u64, Option<Color>)],
        #[case] delve: &[u64],
        #[case] expected: Option<&str>,
    ) {
        let used = Used {
            convoke: convoke
                .iter()
                .map(|(id, pays)| Convoked {
                    creature: ObjectId(*id),
                    pays: *pays,
                })
                .collect(),
            delve: cards(delve),
            ..Used::default()
        };
        let reduced = reduce(&cost("{1}{R}"), &used).ok().map(|c| c.to_string());
        assert_eq!(reduced.as_deref(), expected);
    }

    #[rstest]
    #[case::fine(&[(1, None)], &[7], &[], &[], Ok(()))]
    #[case::not_offered(&[(9, None)], &[], &[], &[], Err(PaymentError::NotOffered(ObjectId(9))))]
    #[case::twice(&[(1, None), (1, None)], &[], &[], &[], Err(PaymentError::HelpedTwice(ObjectId(1))))]
    #[case::tapped(&[(1, None)], &[], &[1], &[], Err(PaymentError::AlreadyTapped(ObjectId(1))))]
    #[case::also_tapped_for_mana(&[(1, None)], &[], &[], &[1], Err(PaymentError::ObjectTappedTwice(ObjectId(1))))]
    #[case::a_colour_it_is_not(&[(1, Some(Color::Blue))], &[], &[], &[], Err(PaymentError::WrongColor { creature: ObjectId(1), color: Color::Blue }))]
    #[case::a_card_not_in_the_graveyard(&[], &[8], &[], &[], Err(PaymentError::NotInGraveyard(ObjectId(8))))]
    #[case::a_card_gone(&[], &[7], &[7], &[], Err(PaymentError::NotInGraveyard(ObjectId(7))))]
    #[case::a_card_twice(&[], &[7, 7], &[], &[], Err(PaymentError::HelpedTwice(ObjectId(7))))]
    fn the_helpers_named_are_checked_against_the_offer(
        #[case] convoke: &[(u64, Option<Color>)],
        #[case] delve: &[u64],
        #[case] unusable: &[u64],
        #[case] for_mana: &[u64],
        #[case] expected: Result<(), PaymentError>,
    ) {
        let offer = [creature(1, &[Color::Red])];
        let help = Help {
            convoke: &offer,
            improvise: &[],
            delve: &cards(&[7]),
        };
        let used = Used {
            convoke: convoke
                .iter()
                .map(|(id, pays)| Convoked {
                    creature: ObjectId(*id),
                    pays: *pays,
                })
                .collect(),
            delve: cards(delve),
            ..Used::default()
        };
        assert_eq!(
            check(&used, help, &cards(unusable), &cards(for_mana)),
            expected
        );
    }
}
