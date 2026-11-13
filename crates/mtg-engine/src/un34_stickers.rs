//! Stickers (CR 123): the sticker sheets a seat brings, three of them chosen at random as the game begins; putting a
//! sticker on a permanent or card its player owns for the sticker's ticket cost; and the stickers each card has, which
//! stay on it in every zone. While it is on the battlefield each sticker's change applies as a continuous effect
//! derived with the layers (a name sticker in layer 3, an ability sticker in layer 6, a power/toughness sticker in
//! layer 7b), timestamped when the sticker was put on or the permanent entered, whichever is later.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Sticker, StickerKind};
use mtg_cards::value::CounterKind;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use crate::effects::{ContinuousEffect, Modification, Selector};
use crate::error::EngineError;
use crate::events::Event;
use crate::fold::invalid;
use crate::object::ObjectRef;
use crate::state::GameState;

/// The name under which a resolution keeps the unique vowels of the last name sticker it put on (`Amount::StickerVowels`).
pub(crate) const VOWELS: &str = "~sticker-vowels";

/// The number a trigger on putting a sticker on carries as its amount for the sticker's kind
/// (`Condition::TriggeringSticker`).
pub(crate) const fn kind_code(kind: StickerKind) -> u32 {
    match kind {
        StickerKind::Name => 1,
        StickerKind::Ability => 2,
        StickerKind::PowerToughness => 3,
        StickerKind::Art => 4,
    }
}

/// How many sticker sheets a seat plays with (CR 123).
pub const SHEETS_IN_PLAY: usize = 3;

/// The kind of counter a ticket is (`{TK}`).
pub(crate) fn ticket() -> CounterKind {
    CounterKind::Other("ticket".to_owned())
}

/// One sticker of a sheet a seat plays with: the sheet (an index into the sheets the seat brought) and the sticker
/// (an index among the stickers of that sheet).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StickerAt {
    /// The seat whose sheet it is.
    pub player: PlayerId,
    /// The sheet, in the order the seat brought its sheets.
    pub sheet: usize,
    /// The sticker, in the order the sheet lists them.
    pub sticker: usize,
}

/// The stickers of a game: the sheets each seat brought, the ones it plays with, the stickers used and the stickers
/// on each permanent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stickers {
    pub(crate) brought: BTreeMap<PlayerId, Vec<Arc<CardDefinition>>>,
    pub(crate) chosen: BTreeMap<PlayerId, Vec<usize>>,
    pub(crate) used: BTreeSet<StickerAt>,
    pub(crate) on: BTreeMap<ObjectId, Vec<StickerAt>>,
    pub(crate) put_at: BTreeMap<StickerAt, u64>,
}

impl Hash for Stickers {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for (player, sheets) in &self.brought {
            player.hash(state);
            for sheet in sheets {
                sheet.name.hash(state);
            }
        }
        self.chosen.hash(state);
        self.used.hash(state);
        self.on.hash(state);
        self.put_at.hash(state);
    }
}

impl Stickers {
    /// Whether no seat brought a sticker sheet.
    pub fn is_empty(&self) -> bool {
        self.brought.values().all(Vec::is_empty)
    }

    /// The sheets a seat brings (setup): with no more than three it plays with all of them at once, else the opening
    /// chooses three at random (`choices`), and a game that skips the opening plays with the first three.
    pub(crate) fn bring(&mut self, player: PlayerId, sheets: Vec<Arc<CardDefinition>>) {
        if sheets.is_empty() {
            return;
        }
        self.chosen
            .insert(player, (0..sheets.len().min(SHEETS_IN_PLAY)).collect());
        self.brought.insert(player, sheets);
    }
}

/// Whether the object, as the reference names it, has a sticker on it, of the kind when one is given; a card keeps its
/// stickers in every zone, and an object that has changed zones since the reference is another object.
pub(crate) fn has_sticker(
    state: &GameState,
    reference: ObjectRef,
    kind: Option<StickerKind>,
) -> bool {
    stickers_of_kind(state, reference, kind) > 0
}

/// How many stickers are on the object, of the kind when one is given; none on an object the reference no longer names.
pub(crate) fn stickers_of_kind(
    state: &GameState,
    reference: ObjectRef,
    kind: Option<StickerKind>,
) -> u32 {
    if state.un34.on.is_empty()
        || state
            .object(reference.id)
            .is_none_or(|object| object.counter() != reference.counter)
    {
        return 0;
    }
    let count = state
        .stickers_on(reference.id)
        .iter()
        .filter_map(|at| state.sticker(*at))
        .filter(|sticker| kind.is_none_or(|kind| sticker.kind() == kind))
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// The stickers a sheet lists, in order.
pub fn stickers_of(sheet: &CardDefinition) -> impl Iterator<Item = &Sticker> {
    sheet.rules.iter().filter_map(|rule| match &**rule {
        Ability::Sticker(sticker) => Some(sticker),
        _ => None,
    })
}

impl GameState {
    /// The sticker sheets the player plays with, as indexes into the sheets they brought, with the sheets.
    pub fn sticker_sheets(&self, player: PlayerId) -> Vec<(usize, &Arc<CardDefinition>)> {
        let Some(brought) = self.un34.brought.get(&player) else {
            return Vec::new();
        };
        self.un34
            .chosen
            .get(&player)
            .map(|chosen| {
                chosen
                    .iter()
                    .filter_map(|at| brought.get(*at).map(|sheet| (*at, sheet)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The sticker at `at`, if the seat brought it.
    pub fn sticker(&self, at: StickerAt) -> Option<&Sticker> {
        let sheet = self.un34.brought.get(&at.player)?.get(at.sheet)?;
        stickers_of(sheet).nth(at.sticker)
    }

    /// Whether the sticker has been put on a permanent; each is used once.
    pub fn sticker_used(&self, at: StickerAt) -> bool {
        self.un34.used.contains(&at)
    }

    /// The stickers on the permanent, in the order they were put on.
    pub fn stickers_on(&self, id: ObjectId) -> &[StickerAt] {
        self.un34.on.get(&id).map_or(&[], Vec::as_slice)
    }

    /// The object's name as the game sees it: the printed name with the words of its name stickers before it
    /// (`Characteristics::name_words`, layer 3).
    pub fn name_of(&self, id: ObjectId) -> Option<String> {
        let object = self.object(id)?;
        let printed = object.card().name.clone();
        if self.un34.on.is_empty() {
            return Some(printed);
        }
        let words = crate::characteristics::seen_as(self, id)
            .map(|(chars, _)| chars.name_words)
            .unwrap_or_default();
        Some(name_with(&words, &printed))
    }
}

/// A printed name with the words name stickers gave it before it.
pub fn name_with(words: &[String], printed: &str) -> String {
    let mut parts: Vec<&str> = words.iter().map(String::as_str).collect();
    parts.push(printed);
    parts.join(" ")
}

/// The stickers the player can put on now: unused stickers of the sheets they play with whose ticket cost they can
/// pay.
pub(crate) fn affordable(state: &GameState, player: PlayerId) -> Vec<(StickerAt, &Sticker)> {
    let tickets = state
        .player(player)
        .map_or(0, |seat| seat.counters_of(&ticket()));
    state
        .sticker_sheets(player)
        .into_iter()
        .flat_map(|(sheet, card)| {
            stickers_of(card).enumerate().map(move |(sticker, found)| {
                (
                    StickerAt {
                        player,
                        sheet,
                        sticker,
                    },
                    found,
                )
            })
        })
        .filter(|(at, sticker)| !state.sticker_used(*at) && sticker.tickets() <= tickets)
        .collect()
}

/// The words of a sticker, as the table sees it beside its sheet: the name words, the ability or the power and
/// toughness, and its ticket cost.
pub fn sticker_text(sticker: &Sticker) -> String {
    let what = match sticker {
        Sticker::Name { words, .. } => format!("\u{201c}{words}\u{201d}"),
        Sticker::Ability { ability, .. } => {
            mtg_cards::compile::render_ability(ability).unwrap_or_else(|| "an ability".to_owned())
        }
        Sticker::PowerToughness {
            power, toughness, ..
        } => format!("{power}/{toughness}"),
        Sticker::Art {} => "art".to_owned(),
    };
    match sticker.tickets() {
        0 => what,
        count => format!(
            "{what} ({})",
            "{TK}".repeat(usize::try_from(count).unwrap_or(0))
        ),
    }
}

/// The change a sticker makes to the permanent it is on; an art sticker makes none.
fn modification(sticker: &Sticker) -> Option<Modification> {
    match sticker {
        Sticker::Name { words, .. } => Some(Modification::AddNameWords(words.clone())),
        Sticker::Ability { ability, .. } => {
            Some(Modification::GrantAbility(Arc::new((**ability).clone())))
        }
        Sticker::PowerToughness {
            power, toughness, ..
        } => Some(Modification::SetPowerToughness {
            power: *power,
            toughness: *toughness,
        }),
        Sticker::Art {} => None,
    }
}

/// The events of putting the sticker `at` on `on`: the tickets paid and the sticker put on.
pub(crate) fn put_events(state: &GameState, at: StickerAt, on: ObjectRef) -> Vec<Event> {
    let Some(sticker) = state.sticker(at) else {
        return Vec::new();
    };
    let mut events = Vec::new();
    let tickets = sticker.tickets();
    if tickets > 0 {
        events.push(Event::CountersChanged(crate::counters::CountersChanged {
            on: crate::choose::Target::Player(at.player),
            counter: ticket(),
            delta: -i32::try_from(tickets).unwrap_or(i32::MAX),
        }));
    }
    events.push(Event::StickerPut {
        player: at.player,
        object: on.id,
        sheet: at.sheet,
        sticker: at.sticker,
    });
    events
}

/// The random choice of three sheets for every seat that brought more (CR 103.2d), `offset` being the first substream
/// of the batch left to it.
pub(crate) fn choices(state: &GameState, offset: u64) -> Vec<Event> {
    state
        .un34
        .brought
        .iter()
        .filter(|(_, sheets)| sheets.len() > SHEETS_IN_PLAY)
        .zip(offset..)
        .map(|((player, sheets), offset)| {
            let order: Vec<ObjectId> = (0..sheets.len())
                .map(|at| ObjectId(u64::try_from(at).unwrap_or(u64::MAX)))
                .collect();
            let mut chosen: Vec<usize> = crate::mulligan::shuffled(state, offset, order)
                .into_iter()
                .take(SHEETS_IN_PLAY)
                .filter_map(|id| usize::try_from(id.0).ok())
                .collect();
            chosen.sort_unstable();
            Event::StickerSheetsChosen {
                player: *player,
                sheets: chosen,
            }
        })
        .collect()
}

/// Folds the choice of a seat's sheets: three distinct sheets it brought.
pub(crate) fn fold_chosen(
    state: &mut GameState,
    player: PlayerId,
    sheets: &[usize],
) -> Result<(), EngineError> {
    let brought = state.un34.brought.get(&player).map_or(0, Vec::len);
    let distinct: BTreeSet<usize> = sheets.iter().copied().collect();
    if sheets.len() != brought.min(SHEETS_IN_PLAY)
        || distinct.len() != sheets.len()
        || sheets.iter().any(|at| *at >= brought)
    {
        return Err(invalid(format!(
            "{player:?} plays with sticker sheets they did not bring: {sheets:?}"
        )));
    }
    state.un34.chosen.insert(player, sheets.to_vec());
    Ok(())
}

/// Folds a sticker put on a permanent: the sticker is used and the permanent has it.
pub(crate) fn fold_put(
    state: &mut GameState,
    at: StickerAt,
    object: ObjectId,
) -> Result<(), EngineError> {
    let playing = state
        .un34
        .chosen
        .get(&at.player)
        .is_some_and(|chosen| chosen.contains(&at.sheet));
    if !playing || state.sticker(at).is_none() || !state.un34.used.insert(at) {
        return Err(invalid(format!(
            "{:?} has no unused sticker {at:?} to put on",
            at.player
        )));
    }
    state.un34.on.entry(object).or_default().push(at);
    state.un34.put_at.insert(at, state.next_timestamp);
    state.next_timestamp += 1;
    Ok(())
}

/// A token that leaves the battlefield ceases to exist with its stickers; a card keeps them in every zone.
pub(crate) fn fold_left(state: &mut GameState, object: ObjectId) {
    if !state.un34.on.is_empty()
        && state
            .object(object)
            .is_some_and(|found| found.kind() == crate::object::ObjectKind::Token)
    {
        state.un34.on.remove(&object);
    }
}

/// The continuous effects of the stickers on permanents (CR 123): one per sticker that changes something, on that
/// permanent as it is now, timestamped when the sticker was put on or the permanent entered, whichever is later.
pub(crate) fn effects(state: &GameState) -> Vec<ContinuousEffect> {
    let mut found = Vec::new();
    for (id, stickers) in &state.un34.on {
        let Some(object) = state
            .object(*id)
            .filter(|object| object.zone() == crate::zones::Zone::Battlefield)
        else {
            continue;
        };
        let entered = object
            .permanent()
            .map_or(0, crate::object::PermanentState::timestamp);
        for (index, at) in stickers.iter().enumerate() {
            let Some(modification) = state.sticker(*at).and_then(modification) else {
                continue;
            };
            let put = state.un34.put_at.get(at).copied().unwrap_or(0);
            // An id far past any the game hands out keeps these apart from the registered effects and from each other.
            let offset = id.0.saturating_mul(64).saturating_add(index as u64);
            found.push(
                ContinuousEffect::new(
                    EffectId((u64::MAX / 2).saturating_sub(offset)),
                    Selector::Objects(vec![object.reference()]),
                    modification,
                    mtg_cards::modify::Duration::Forever,
                    put.max(entered),
                )
                .begun(at.player, 0),
            );
        }
    }
    found
}

#[cfg(test)]
mod tests {
    //! Goal: a seat with more than three sticker sheets plays with three distinct ones chosen at random by the seed, one with three or fewer plays with all of them from the start, and the fold refuses a choice of sheets the seat did not bring.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::{Ability, Sticker};
    use rstest::rstest;

    use super::*;
    use crate::fixtures::setup;

    fn sheet(at: usize) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new(format!("Sheet {at}")).with_rules(Ability::Sticker(
                Sticker::Name {
                    words: format!("Word{at}"),
                    tickets: 0,
                },
            )),
        )
    }

    fn game(sheets: usize, seed: u64) -> GameState {
        let mut setup = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
        setup.players[0].stickers = (0..sheets).map(sheet).collect();
        let mut game = GameState::from_setup(&setup).unwrap();
        game.seed = seed;
        game
    }

    #[rstest]
    #[case::none(0, 0)]
    #[case::two(2, 2)]
    #[case::three(3, 3)]
    fn three_or_fewer_sheets_are_all_in_play(#[case] sheets: usize, #[case] playing: usize) {
        let game = game(sheets, 1);
        assert_eq!(choices(&game, 1), []);
        assert_eq!(game.sticker_sheets(PlayerId(0)).len(), playing);
        assert_eq!(game.sticker_sheets(PlayerId(1)), []);
    }

    #[rstest]
    fn more_sheets_are_chosen_three_at_random(#[values(4, 5, 9)] sheets: usize) {
        let mut seen = BTreeSet::new();
        for seed in 0..32 {
            let mut game = game(sheets, seed);
            let events = choices(&game, 1);
            let [
                Event::StickerSheetsChosen {
                    player,
                    sheets: chosen,
                },
            ] = events.as_slice()
            else {
                panic!("{events:?}");
            };
            assert_eq!(*player, PlayerId(0));
            crate::fold::apply_event(&mut game, &events[0]).unwrap();
            let playing: Vec<usize> = game
                .sticker_sheets(PlayerId(0))
                .into_iter()
                .map(|(at, _)| at)
                .collect();
            assert_eq!(&playing, chosen);
            assert_eq!(playing.len(), SHEETS_IN_PLAY);
            assert!(playing.windows(2).all(|pair| pair[0] < pair[1]));
            seen.extend(playing);
        }
        assert_eq!(seen.len(), sheets, "every sheet is chosen for some seed");
    }

    #[rstest]
    #[case::too_few(vec![0, 1])]
    #[case::twice(vec![0, 0, 1])]
    #[case::not_brought(vec![0, 1, 7])]
    fn a_choice_of_sheets_not_brought_is_refused(#[case] sheets: Vec<usize>) {
        let mut game = game(5, 1);
        let event = Event::StickerSheetsChosen {
            player: PlayerId(0),
            sheets,
        };
        assert!(crate::fold::apply_event(&mut game, &event).is_err());
    }
}
