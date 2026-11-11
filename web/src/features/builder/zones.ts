/** The parts of a deck: their order, their names and which of them a format uses. */
import type { DeckZone } from '@/protocol/generated'

/** The zones in the order the deck pane lists them. */
export const ZONES: readonly DeckZone[] = [
  'commander',
  'main',
  'sideboard',
  'companion',
  'attractions',
  'stickers',
]

/** The heading of a zone in the deck pane. */
export const ZONE_TITLE: Readonly<Record<DeckZone, string>> = {
  commander: 'Commander',
  main: 'Main deck',
  sideboard: 'Sideboard',
  companion: 'Companion',
  attractions: 'Attraction deck',
  stickers: 'Sticker sheets',
}

/** The zone after "to": "Add to main deck", "Move to sideboard". */
export const ZONE_TO: Readonly<Record<DeckZone, string>> = {
  commander: 'commander',
  main: 'main deck',
  sideboard: 'sideboard',
  companion: 'companion',
  attractions: 'Attraction deck',
  stickers: 'sticker sheets',
}

/** The zone inside a sentence: "Added Lightning Bolt to the main deck". */
export const ZONE_THE: Readonly<Record<DeckZone, string>> = {
  commander: 'the command zone',
  main: 'the main deck',
  sideboard: 'the sideboard',
  companion: 'the companion slot',
  attractions: 'the Attraction deck',
  stickers: 'the sticker sheets',
}

// Only Commander and Freeform have a command zone (`docs/deck-check.md`, legality rules).
const WITH_COMMAND_ZONE: ReadonlySet<string> = new Set(['commander', 'freeform'])

/**
 * The zones a card of type line `typeLine` can be put in under `format`, in listing order; the check reports a card that
 * does not belong. The Attraction deck is offered only for an Attraction and the sticker sheets only for a sheet.
 */
export function zonesFor(format: string, typeLine = ''): readonly DeckZone[] {
  return ZONES.filter(
    (zone) =>
      (zone !== 'commander' || WITH_COMMAND_ZONE.has(format)) &&
      (zone !== 'attractions' || /\bAttraction\b/.test(typeLine)) &&
      (zone !== 'stickers' || typeLine.startsWith('Stickers')),
  )
}

