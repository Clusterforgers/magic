/** Pure: which cards a seat may not look at, and so does not name by hand. */
import type { ObjectView, PlayerId } from '@/protocol/generated'

/** Why a card the seat may not look at is not named by hand. */
export const HIDDEN_CARD = 'A card in another seat’s hand or face down in exile is not named by hand.'

/** Whether the card lies where the seat may not look at it: in another seat's hand, or face down off the battlefield. */
export function hiddenCard({ object, seat }: { object: ObjectView; seat: PlayerId }): boolean {
  const { zone } = object
  if (typeof zone === 'object' && 'Hand' in zone) return object.owner !== seat
  return zone !== 'Battlefield' && object.face_down === true && object.name === ''
}
