import type { ObjectId } from '@/protocol/generated'
import type { Ghost } from './model'

/**
 * The cards of a row with each ghost put in front of the card it stood before, or at the end when that card has
 * gone too. A number is a card's id, anything else a ghost; ghosts of one place keep the order they came in.
 */
export function withGhosts(ids: readonly ObjectId[], ghosts: readonly Ghost[]): Array<ObjectId | Ghost> {
  const items: Array<ObjectId | Ghost> = []
  for (const id of ids) {
    for (const ghost of ghosts) if (ghost.before === id) items.push(ghost)
    items.push(id)
  }
  const present = new Set(ids)
  for (const ghost of ghosts) if (ghost.before === null || !present.has(ghost.before)) items.push(ghost)
  return items
}
