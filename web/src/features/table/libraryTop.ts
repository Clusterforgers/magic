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

/**
 * The spoken name of the head of the hand dock: what it holds of the commanders, the Satchel and the top card of the
 * library ("Ann's commanders in the command zone and the Satchel").
 */
export function headLabel(owner: string, head: ReadonlyMap<ObjectId, HandCommander>): string {
  const entries = [...head.values()]
  const parts = [
    entries.some((entry) => entry.top !== true && entry.satchel !== true)
      ? 'commanders in the command zone'
      : null,
    entries.some((entry) => entry.satchel === true) ? 'Satchel' : null,
    entries.some((entry) => entry.top === true) ? 'top card of the library' : null,
  ].filter((part): part is string => part !== null)
  const [first = 'commanders in the command zone', ...rest] = parts
  const more = rest.map((part) => `the ${part}`)
  const last = more.pop()
  const head0 = [first, ...more].join(', ')
  return `${owner}'s ${last === undefined ? head0 : `${head0} and ${last}`}`
}
