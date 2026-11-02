/** The two piles of a scry: the cards that stay on top and the cards that go to the bottom, each first-on-top, and the moves between them. */
import type { ObjectId } from '@/protocol/generated'

/** Where the cards lie once the scry is answered; each list has the card nearest the top of the library first. */
export interface Piles {
  top: readonly ObjectId[]
  bottom: readonly ObjectId[]
}

/** Which pile a card is in. */
export type PileName = 'top' | 'bottom'

/** Every card on top in the order looked at: the scry that changes nothing. */
export function startPiles(looked: readonly ObjectId[]): Piles {
  return { top: [...looked], bottom: [] }
}

/** The pile `id` lies in, or null for a card of neither. */
export function pileOfCard(piles: Piles, id: ObjectId): PileName | null {
  if (piles.top.includes(id)) return 'top'
  return piles.bottom.includes(id) ? 'bottom' : null
}

/**
 * `id` taken out of its pile and put into `to` just before the card `before`, or last when `before` is null or not in
 * that pile. A card dropped before itself stays where it is. Piles of the same cards in a different shape never
 * lose or repeat a card.
 */
export function placed(piles: Piles, id: ObjectId, to: PileName, before: ObjectId | null): Piles {
  if (pileOfCard(piles, id) === null || id === before) return piles
  const rest = {
    top: piles.top.filter((card) => card !== id),
    bottom: piles.bottom.filter((card) => card !== id),
  }
  const target = [...rest[to]]
  const at = before === null ? -1 : target.indexOf(before)
  target.splice(at < 0 ? target.length : at, 0, id)
  return { ...rest, [to]: target }
}

/** The card sent to the other pile, last there. */
export function crossed(piles: Piles, id: ObjectId): Piles {
  const from = pileOfCard(piles, id)
  return from === null ? piles : placed(piles, id, from === 'top' ? 'bottom' : 'top', null)
}

/** The card moved one place toward the top of the library (-1) or the bottom (1) within its pile; unchanged at either end. */
export function shifted(piles: Piles, id: ObjectId, by: -1 | 1): Piles {
  const from = pileOfCard(piles, id)
  if (from === null) return piles
  const list = [...piles[from]]
  const at = list.indexOf(id)
  const to = at + by
  const [one, other] = [list[at], list[to]]
  if (one === undefined || other === undefined) return piles
  list[at] = other
  list[to] = one
  return { ...piles, [from]: list }
}

/** Whether the piles are the scry that changes nothing: every card on top in the order looked at. */
export function isStart(piles: Piles, looked: readonly ObjectId[]): boolean {
  return (
    piles.bottom.length === 0 &&
    piles.top.length === looked.length &&
    piles.top.every((id, i) => id === looked[i])
  )
}

/** The sentence under the piles: "Top of your library: A, then B. Bottom: C." */
export function scryPreview(top: readonly string[], bottom: readonly string[]): string {
  const list = (names: readonly string[]): string => (names.length === 0 ? 'none' : names.join(', then '))
  return `Top of your library: ${list(top)}. Bottom, top to bottom: ${list(bottom)}.`
}

/** The keys of the scry for a screen reader; `count` cards are numbered in the order looked at. */
export function scryKeys(count: number): string {
  const range = count <= 1 ? '1' : count <= 9 ? `1 to ${count}` : '1 to 9, or 0'
  return `Press ${range} to send that card to the other pile. Alt with the arrow keys moves the focused card within its pile. Cards can also be dragged between the piles and into order.`
}
