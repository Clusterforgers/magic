/** The combat forecast summed up for one side of the table (the total in the dock) and per seat (its walkers). Pure. */
import type { CombatForecast, CreatureForecast, DamageTarget, ObjectId, PlayerId } from '@/protocol/generated'
import { CHOICE_WORDS, outlookText } from './forecastText'

/** The attacked planeswalkers of `seat` in `forecast`, in the forecast's order, that take damage or fall. */
export function seatWalkers(
  forecast: CombatForecast | null,
  seat: PlayerId,
  controllerOf: (id: ObjectId) => PlayerId | undefined,
): CreatureForecast[] {
  return (forecast?.attacked ?? []).filter(
    (walker) => controllerOf(walker.object) === seat && (walker.damage > 0 || walker.dies),
  )
}

/** What a walker's pod chip says: "Chandra would take 3 damage and die". */
export function walkerWords(name: string, walker: CreatureForecast, choice = false): string {
  const words = `${name} ${outlookText({ damage: walker.damage, dies: walker.dies })}`
  return choice ? `${words}, ${CHOICE_WORDS}` : words
}

/** What combat would do, counted from the viewer's side (the viewer and their teammates). */
export interface ForecastTotal {
  /** Damage the side's creatures would deal to other players and their planeswalkers. */
  deals: number
  /** Damage the side's players and planeswalkers would take. */
  takes: number
  /** Whether that damage is lethal to one of the side's players. */
  takesLethal: boolean
  /** Whether the damage is lethal to another player. */
  dealsLethal: boolean
  /** The side's creatures and planeswalkers that would die. */
  lost: number
  /** Other players' creatures and planeswalkers that would die. */
  killed: number
  /** The numbers depend on a choice of effects. */
  choice: boolean
}

/**
 * The forecast summed for `side`: the engine's packets from the side's creatures to other players and their
 * planeswalkers, the damage each player and planeswalker of the side takes, and the deaths on each side. Nothing is
 * worked out beyond adding the engine's numbers.
 */
export function forecastTotal(
  forecast: CombatForecast,
  side: ReadonlySet<PlayerId>,
  controllerOf: (id: ObjectId) => PlayerId | undefined,
): ForecastTotal {
  const ours = (id: ObjectId): boolean => {
    const controller = controllerOf(id)
    return controller !== undefined && side.has(controller)
  }
  const walkers = forecast.attacked ?? []
  // What counts as dealt: damage to another side's player or to an attacked planeswalker of theirs, not to blockers.
  const counted = (target: DamageTarget): boolean => {
    if ('Player' in target) return !side.has(target.Player)
    const id = target.Object
    return !ours(id) && walkers.some((walker) => walker.object === id)
  }
  let deals = 0
  for (const attack of forecast.attackers)
    for (const hit of attack.deals) if (ours(hit.source) && counted(hit.target)) deals += hit.amount
  const takes =
    forecast.players.filter((p) => side.has(p.player)).reduce((sum, p) => sum + p.damage, 0) +
    walkers.filter((w) => ours(w.object)).reduce((sum, w) => sum + w.damage, 0)
  const fallen = [...forecast.creatures, ...walkers].filter((entry) => entry.dies)
  return {
    deals,
    takes,
    takesLethal: forecast.players.some((p) => side.has(p.player) && p.lethal),
    dealsLethal: forecast.players.some((p) => !side.has(p.player) && p.lethal),
    lost: fallen.filter((entry) => ours(entry.object)).length,
    killed: fallen.filter((entry) => !ours(entry.object)).length,
    choice: forecast.depends_on_choice === true,
  }
}

/** One chip of the total: what it shows and what it says to a screen reader. */
export interface TotalChip {
  /** Which number it is. */
  kind: 'deals' | 'takes' | 'lost' | 'killed' | 'none'
  /** The chip's words: "Deal 6", "Take 12", "Lose 1", "Kill 2", "No damage". */
  text: string
  /** The sentence: "You would deal 6 damage, which is lethal". */
  label: string
  /** The number ends a player's game. */
  lethal: boolean
}

const creaturesOf = (count: number): string => (count === 1 ? '1 creature' : `${count} creatures`)

/**
 * The chips of a total, in the order the dock shows them (damage before deaths, yours before theirs), only those that
 * are not zero, or a single "No damage" chip. `team` words them for a side of several players.
 */
export function totalChips(total: ForecastTotal, team = false): TotalChip[] {
  const about = total.choice ? '≈' : ''
  const we = team ? 'Your team' : 'You'
  const lethal = (yes: boolean): string => (yes ? ', which is lethal' : '')
  const chips: TotalChip[] = []
  if (total.deals > 0)
    chips.push({
      kind: 'deals',
      text: `Deal ${about}${total.deals}`,
      label: `${we} would deal ${total.deals} damage${lethal(total.dealsLethal)}`,
      lethal: total.dealsLethal,
    })
  if (total.takes > 0)
    chips.push({
      kind: 'takes',
      text: `Take ${about}${total.takes}`,
      label: `${we} would take ${total.takes} damage${lethal(total.takesLethal)}`,
      lethal: total.takesLethal,
    })
  if (total.lost > 0)
    chips.push({
      kind: 'lost',
      text: `Lose ${about}${total.lost}`,
      label: `${we} would lose ${creaturesOf(total.lost)}`,
      lethal: false,
    })
  if (total.killed > 0)
    chips.push({
      kind: 'killed',
      text: `Kill ${about}${total.killed}`,
      label: `${creaturesOf(total.killed)} of the other side would die`,
      lethal: false,
    })
  if (chips.length === 0)
    chips.push({ kind: 'none', text: 'No damage', label: 'No damage would be dealt', lethal: false })
  return chips
}
