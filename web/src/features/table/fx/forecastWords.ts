/** The words of a stack forecast (docs/ui-design.md, Forecast): what one card or seat is about to undergo, in the fewest words. */
import type { Change, CounterKind, Fate } from '@/protocol/generated'

/** A typographic minus, so "−3" reads as a number and lines up with "+3". */
const MINUS = '−'

const FATE: Record<Fate, string> = {
  Destroyed: 'destroyed',
  Dies: 'dies',
  Sacrificed: 'sacrificed',
  Exiled: 'exiled',
  Hand: 'to hand',
  Library: 'to library',
  Graveyard: 'to graveyard',
  Countered: 'countered',
}

/** A change with its sign: "+3", "−2", "+0". */
export function signed(delta: number): string {
  return delta < 0 ? `${MINUS}${-delta}` : `+${delta}`
}

function counterName(kind: CounterKind): string {
  return typeof kind === 'string' ? kind : kind.Other
}

/** One change in words: "3 damage", "+3/+3 → 7/7", "+2 +1/+1 counters", "dies", "−3 → 17". */
export function changeWords(change: Change, controllerName?: (seat: number) => string): string {
  if (change === 'Tapped') return 'tapped'
  if (change === 'Untapped') return 'untapped'
  if ('Damage' in change) return `${change.Damage.amount} damage`
  if ('Stats' in change) {
    const { power, toughness } = change.Stats
    return `${signed(power[1] - power[0])}/${signed(toughness[1] - toughness[0])} → ${power[1]}/${toughness[1]}`
  }
  if ('Counters' in change) {
    const [from, to] = change.Counters.count
    const many = Math.abs(to - from) === 1 ? 'counter' : 'counters'
    return `${signed(to - from)} ${counterName(change.Counters.kind)} ${many}`
  }
  if ('Control' in change) {
    const name = controllerName?.(change.Control.controller)
    return name === undefined ? 'changes control' : `to ${name}`
  }
  if ('Leaves' in change) return FATE[change.Leaves.fate]
  const { life, life_after } = change.Life
  return `${signed(life_after - life)} → ${life_after}`
}

/** Every change of one card or seat, joined: "3 damage · dies"; null when there is none. */
export function forecastWords(
  changes: readonly Change[],
  controllerName?: (seat: number) => string,
): string | null {
  if (changes.length === 0) return null
  return changes.map((change) => changeWords(change, controllerName)).join(' · ')
}
