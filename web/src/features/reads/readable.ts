/** Which cards of the table offer a read (docs/web.md, Reading cards), pure. */
import type { ObjectView } from '@/protocol/generated'

/** A card to open in `ReadDialog`. */
export interface Reading {
  oracleId: string
  name: string
}

/**
 * The card a preview offers to read: one with a known identity that runs on a read, or that leaves clauses to the
 * table (a draft that waits for a read is played by hand); null for any other.
 */
export function readableOf(
  object: Pick<ObjectView, 'identity' | 'read' | 'name'>,
  byHand: number,
): Reading | null {
  const oracleId = object.identity?.oracle_id
  if (oracleId === undefined || (object.read !== true && byHand === 0)) return null
  return { oracleId, name: object.name }
}
