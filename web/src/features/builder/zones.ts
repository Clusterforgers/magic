/** The four parts of a deck: their order, their names and which of them a format uses. */
import type { DeckZone } from '@/protocol/generated'

/** The zones in the order the deck pane lists them. */
export const ZONES: readonly DeckZone[] = ['commander', 'main', 'sideboard', 'companion']

/** The heading of a zone in the deck pane. */
export const ZONE_TITLE: Readonly<Record<DeckZone, string>> = {
  commander: 'Commander',
  main: 'Main deck',
  sideboard: 'Sideboard',
  companion: 'Companion',
}

/** The zone after "to": "Add to main deck", "Move to sideboard". */
export const ZONE_TO: Readonly<Record<DeckZone, string>> = {
  commander: 'commander',
  main: 'main deck',
  sideboard: 'sideboard',
  companion: 'companion',
}

/** The zone inside a sentence: "Added Lightning Bolt to the main deck". */
export const ZONE_THE: Readonly<Record<DeckZone, string>> = {
  commander: 'the command zone',
  main: 'the main deck',
  sideboard: 'the sideboard',
  companion: 'the companion slot',
}

// Only Commander and Freeform have a command zone (`docs/deck-check.md`, legality rules).
const WITH_COMMAND_ZONE: ReadonlySet<string> = new Set(['commander', 'freeform'])

/** The zones a card can be put in under `format`, in listing order; the check reports a card that does not belong. */
export function zonesFor(format: string): readonly DeckZone[] {
  return WITH_COMMAND_ZONE.has(format) ? ZONES : ZONES.filter((zone) => zone !== 'commander')
}
