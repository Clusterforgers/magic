/** Pure words and figures of Commander at the table: the tax chip, commander damage and its thresholds, the return question. */
import type {
  Color,
  CommanderView,
  ManaCost,
  ManaSymbol,
  ObjectId,
  ObjectView,
  PlayerView,
  Zone,
} from '@/protocol/generated'
import { costText } from './cardModel'

/** "Atraxa" for "Atraxa, Praetors' Voice": the part of a name before its comma, which a pod has room for. */
export function shortName(name: string): string {
  const [head = name] = name.split(',')
  return head.trim() === '' ? name : head.trim()
}

/** "Cast 2× · tax {4}" while a commander was cast from the command zone or carries tax; null while neither. */
export function taxChip(commander: Pick<CommanderView, 'casts' | 'tax'>): string | null {
  if (commander.casts === 0 && commander.tax === 0) return null
  return `Cast ${commander.casts}× · tax {${commander.tax}}`
}

/** The tax chip in words: "Atraxa: cast 2 times from the command zone, tax 4 mana". */
export function taxWords(name: string, commander: Pick<CommanderView, 'casts' | 'tax'>): string {
  const times = commander.casts === 1 ? '1 time' : `${commander.casts} times`
  return `${name}: cast ${times} from the command zone, tax ${commander.tax} mana`
}

/** The cost of a cast from the command zone: "{3}{G}{G} + {2} tax", or the printed cost alone while the tax is 0. */
export function castCostLine(cost: ManaCost | null, tax: number): string {
  const printed = cost === null ? '' : costText(cost)
  if (tax === 0) return printed
  return printed === '' ? `{${tax}} tax` : `${printed} + {${tax}} tax`
}

/** How close commander damage is to lethal: plain, a warning, red, or lethal. */
export type DamageLevel = 'plain' | 'warn' | 'danger' | 'lethal'

/**
 * The level of `damage` against `lethal`: a warning from six below lethal (15 of 21), red from three below (18 of 21),
 * lethal at the amount. With no lethal amount (the rule is off) damage is only counted.
 */
export function damageLevel(damage: number, lethal: number | undefined): DamageLevel {
  if (lethal === undefined) return 'plain'
  if (damage >= lethal) return 'lethal'
  if (damage >= lethal - 3) return 'danger'
  if (damage >= Math.max(1, lethal - 6)) return 'warn'
  return 'plain'
}

/** "14/21", or "14" when no amount is lethal. */
export function damageFraction(damage: number, lethal: number | undefined): string {
  return lethal === undefined ? `${damage}` : `${damage}/${lethal}`
}

/** A damage chip in words: "14 of 21 commander damage from Atraxa", "21 commander damage from Atraxa, lethal". */
export function damageWords(name: string, damage: number, lethal: number | undefined): string {
  const level = damageLevel(damage, lethal)
  const amount = lethal === undefined ? `${damage}` : `${damage} of ${lethal}`
  return `${amount} commander damage from ${name}${level === 'lethal' ? ', lethal' : ''}`
}

/** The log line of a hit: "Atraxa deals 7 commander damage to Bo (14/21)". */
export function commanderDamageLine(
  source: string,
  amount: number,
  player: string,
  total: number,
  lethal: number | undefined,
): string {
  return `${source} deals ${amount} commander damage to ${player} (${damageFraction(total, lethal)})`
}

/** Where a commander goes when its owner says no. */
export type CommanderDestination = 'graveyard' | 'exile' | 'hand' | 'library'

/**
 * Where a commander the engine asks about goes on a no: the engine names it after "instead of" while the move is
 * still to happen ("instead of a hand"), and otherwise the commander lies in a graveyard or exile already.
 */
export function commanderDestination(text: string, zone: Zone | undefined): CommanderDestination | null {
  const instead = /instead of (?:a |the )?(graveyard|exile|hand|library)/.exec(text)?.[1]
  if (instead === 'graveyard' || instead === 'exile' || instead === 'hand' || instead === 'library')
    return instead
  if (zone === undefined || typeof zone === 'string') return zone === 'Exile' ? 'exile' : null
  return 'Graveyard' in zone ? 'graveyard' : null
}

/** The question of the return choice: "Return Atraxa to the command zone?". */
export function returnQuestion(name: string): string {
  return `Return ${name} to the command zone?`
}

/** What a no does, by destination, after the question: "No leaves it in the graveyard." */
const STAYS: Record<CommanderDestination, string> = {
  graveyard: 'leaves it in your graveyard',
  exile: 'leaves it in exile',
  hand: 'puts it into your hand',
  library: 'puts it into your library',
}

/** The line under the question that names the destination: "No leaves it in your graveyard." */
export function returnDetail(destination: CommanderDestination | null): string {
  return destination === null ? 'No leaves it where it is.' : `No ${STAYS[destination]}.`
}

/** The label of the No button: "Leave in graveyard", "Put into hand". */
const NO_LABEL: Record<CommanderDestination, string> = {
  graveyard: 'Leave in graveyard',
  exile: 'Leave in exile',
  hand: 'Put into hand',
  library: 'Put into library',
}

/** The words of the No button. */
export function returnNoLabel(destination: CommanderDestination | null): string {
  return destination === null ? 'Leave it' : NO_LABEL[destination]
}

/** Whether `id` is one of the commanders any seat owns. */
export function isCommander(players: readonly PlayerView[], id: number): boolean {
  return players.some((player) => player.commanders?.some((commander) => commander.object === id) === true)
}

const WUBRG: readonly Color[] = ['White', 'Blue', 'Black', 'Red', 'Green']

/** The colours a mana symbol of a cost shows; generic and colourless symbols show none. */
function symbolColors(symbol: ManaSymbol): readonly Color[] {
  if (typeof symbol === 'string') return []
  if ('Colored' in symbol) return [symbol.Colored]
  if ('MonoHybrid' in symbol) return [symbol.MonoHybrid]
  if ('Phyrexian' in symbol) return [symbol.Phyrexian]
  if ('ColorlessHybrid' in symbol) return [symbol.ColorlessHybrid]
  if ('Hybrid' in symbol) return symbol.Hybrid
  if ('HybridPhyrexian' in symbol) return symbol.HybridPhyrexian
  return []
}

/**
 * The colours of a seat's commanders as the view shows them (each card's colours and the colour symbols of its cost,
 * hybrid and Phyrexian symbols included), in WUBRG order; empty for a colourless identity and null while none of its
 * commanders is in the view (no Commander game).
 */
export function identityOf(
  player: Pick<PlayerView, 'commanders'>,
  objects: Readonly<Record<ObjectId, ObjectView | undefined>>,
): Color[] | null {
  const cards = (player.commanders ?? []).flatMap((commander) => {
    const card = objects[commander.object]
    return card === undefined ? [] : [card]
  })
  if (cards.length === 0) return null
  const seen = new Set<Color>(
    cards.flatMap((card) => [...card.colors, ...(card.mana_cost ?? []).flatMap(symbolColors)]),
  )
  return WUBRG.filter((color) => seen.has(color))
}
