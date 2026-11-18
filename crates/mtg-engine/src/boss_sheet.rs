//! A reworked boss's sheet, a campaign rule's public record: its innate with its level, the fight's encounter
//! modifier, its relics and the words of its phases, each by name and rules text. The emblems in its command zone play
//! these effects; the sheet only names them for every seat, so no rule reads it.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// One effect of a boss sheet: a card's name and rules text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SheetCard {
    /// The card's name.
    pub name: String,
    /// Its rules text.
    pub text: String,
}

/// What a reworked boss seat shows every seat.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct BossSheet {
    /// Its innate at its level.
    pub innate: Option<SheetCard>,
    /// The innate's level, 1 to 5; 0 without an innate.
    pub level: u8,
    /// The fight's encounter modifier.
    pub modifier: Option<SheetCard>,
    /// The relics it brings, copies included.
    pub relics: Vec<SheetCard>,
    /// The words of its phases, in the order they begin.
    pub phases: Vec<SheetCard>,
}

impl BossSheet {
    /// The rules text of the phase named `name`; empty when the sheet does not name it.
    pub fn phase_text(&self, name: &str) -> &str {
        self.phases
            .iter()
            .find(|card| card.name == name)
            .map_or("", |card| card.text.as_str())
    }
}

/// A seat's sheet, if it has one; a seat without one leaves the game's hash as it was.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SheetSlot(pub(crate) Option<Arc<BossSheet>>);

impl Hash for SheetSlot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Some(sheet) = &self.0 {
            sheet.hash(state);
        }
    }
}
