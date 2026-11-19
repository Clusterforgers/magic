import { usePlay } from '@/features/game/gameContext'
import type { PlayerId } from '@/protocol/generated'
import { anchor } from './anchors'
import type { AnchorId } from './anchors'

/** The parts of a seat the coach can light. */
export type SeatPart = 'pod' | 'life' | 'board' | 'graveyard' | 'lands' | 'lands-chip'

type Mark = { 'data-coach': AnchorId }

/** Each part's anchor for the viewer's own seat and for an opponent's. */
const PARTS: Record<SeatPart, { own: Mark; opp: Mark | null }> = {
  pod: { own: anchor('own.pod'), opp: anchor('opp.pod') },
  life: { own: anchor('own.life'), opp: anchor('opp.life') },
  board: { own: anchor('own.board'), opp: anchor('opp.board') },
  graveyard: { own: anchor('own.graveyard'), opp: anchor('opp.graveyard') },
  lands: { own: anchor('own.lands'), opp: null },
  'lands-chip': { own: anchor('own.lands-chip'), opp: null },
}

/** The props that mark `seat`'s `part` for the coach: the own seat's or an opponent's anchor; nothing for a spectator. */
export function useSeatAnchor(seat: PlayerId, part: SeatPart): { 'data-coach'?: AnchorId } {
  const viewer = usePlay((state) => state.seat)
  if (viewer === null) return {}
  return (viewer === seat ? PARTS[part].own : PARTS[part].opp) ?? {}
}
