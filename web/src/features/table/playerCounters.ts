/** The counters on a player (`PlayerView.counters`) as the chips of its seat: a symbol, the count and the words. Pure. */
import type { PlayerView } from '@/protocol/generated'
import { counterName } from './cardModel'

/** The kinds of player counter drawn with an icon of their own; any other kind shows a short label instead. */
export type PlayerCounterIcon = 'poison' | 'rad' | 'energy' | 'experience' | 'ticket'

const ICON_KINDS: readonly string[] = ['poison', 'rad', 'energy', 'experience', 'ticket']

/** One kind of counter a player has. */
export interface PlayerCounterChip {
  /** The kind's name: "rad", "poison". */
  key: string
  /** The icon of a common kind; null for another kind, which shows `short`. */
  icon: PlayerCounterIcon | null
  /** The kind's name cut to four letters with a capital, for a kind without an icon: "Acor". */
  short: string
  count: number
  /** "3 rad counters", "1 poison counter". */
  words: string
}

const isIconKind = (name: string): name is PlayerCounterIcon => ICON_KINDS.includes(name)

/** "3 rad counters", "1 poison counter". */
export function playerCounterWords(name: string, count: number): string {
  return `${count} ${name} ${count === 1 ? 'counter' : 'counters'}`
}

/** The chips of the counters on `player`, in the order the view lists the kinds; a kind with a count of 0 is left out. */
export function playerCounterChips(player: Pick<PlayerView, 'counters'>): PlayerCounterChip[] {
  return (player.counters ?? [])
    .filter(([, count]) => count > 0)
    .map(([kind, count]) => {
      const key = counterName(kind)
      return {
        key,
        icon: isIconKind(key) ? key : null,
        short: key.slice(0, 1).toUpperCase() + key.slice(1, 4),
        count,
        words: playerCounterWords(key, count),
      }
    })
}

/** What a chip says aloud and in its tooltip: "Bo: 3 rad counters". */
export function playerCounterLabel(owner: string, chip: PlayerCounterChip): string {
  return `${owner}: ${chip.words}`
}
