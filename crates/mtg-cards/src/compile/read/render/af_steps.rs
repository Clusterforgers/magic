//! Steps that need a sentence of their own: a defending player who exiles permanents, a player who sacrifices several
//! creatures, a choice of one object and the destruction of the rest, a commander taken from the command zone and a
//! sentence whose group names the target ("Tap all creatures target player controls.").

mod choose_rest;
mod commander_back;
mod defender_exiles;
mod flash_spells;
mod reveal_hand;
mod sacrifice_count;
mod shuffle_graveyard;

use super::{Effect, Writer, spec_text};

impl Writer<'_> {
    /// The steps with a wording of their own, and the number of effects they cover.
    pub(super) fn af_steps(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        defender_exiles::text(effects)
            .or_else(|| choose_rest::text(effects))
            .or_else(|| commander_back::text(effects))
            .or_else(|| flash_spells::text(effects))
            .or_else(|| reveal_hand::text(effects))
            .or_else(|| shuffle_graveyard::text(effects))
            .or_else(|| sacrifice_count::text(effects))
            .or_else(|| self.group_of_target(effects))
    }

    /// An effect on a group that the one target controls ("all creatures target player controls"): the sentence
    /// names the target, so no other step names it again.
    fn group_of_target(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [effect, ..] = effects else {
            return None;
        };
        let held = serde_json::to_string(effect)
            .is_ok_and(|json| json.contains(r#""ControlledBy":{"Target":0}"#));
        if self.named || self.targets.len() != 1 || !held {
            return None;
        }
        let spec = spec_text(self.targets.first()?)?;
        let (named, made) = (self.named, self.made);
        let text = self.effect(effect).map(|text| {
            if spec == "target opponent" {
                text.replace("target player controls", "target opponent controls")
            } else {
                text
            }
        });
        match text {
            Some(text) if text.contains(&spec) => {
                self.named = true;
                Some((text, 1))
            }
            _ => {
                (self.named, self.made) = (named, made);
                None
            }
        }
    }
}
