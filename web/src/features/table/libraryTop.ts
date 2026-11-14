/** Pure words and lookups of the chip that shows the top card of your library. */
import type { ObjectId, PlayerView } from '@/protocol/generated'
import type { HandCommander } from './HandFan'

/** The top card of `player`'s library its owner may see (a rule such as Sigarda's or a look by hand), else undefined. */
export function seenTop(player: PlayerView): ObjectId | undefined {
  return player.known_top?.[0]
}

/** The chip's name: "Top of your library: Serra Angel, can be cast or played now, opens the card". */
export function topChipName(name: string, legal: boolean): string {
  return `Top of your library: ${name}${legal ? ', can be cast or played now' : ''}, opens the card`
}

/** What the open card says under its title. */
export function topDialogLine(legal: boolean): string {
  return legal ? 'Only you see it. Click the card to cast or play it.' : 'Only you see it.'
}

/** The words of the top card of your library at the head of the hand dock. */
export const TOP_WORDS = 'on top of your library'

/** The spoken name of the head of the hand dock: the commanders, the top card of the library, or both. */
export function headLabel(owner: string, head: ReadonlyMap<ObjectId, HandCommander>): string {
  const top = [...head.values()].some((entry) => entry.top === true)
  if (!top) return `${owner}'s commanders in the command zone`
  return head.size > 1
    ? `${owner}'s commanders in the command zone and the top card of the library`
    : `${owner}'s top card of the library`
}
