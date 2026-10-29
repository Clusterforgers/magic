/** The dev table's `?menu=`: opens the manual menu of a card as a right click would, to look at it. */
import { useEffect } from 'react'
import type { ObjectId, PlayerId, StateView } from '@/protocol/generated'
import type { DevMenu } from '../devSearch'

/** How many times, 100 ms apart, the table gets to draw the card before the request gives up. */
const TRIES = 100

/** The cards `kind` names, the best first: a creature with counters shows the most of the menu. */
export function menuCandidates(view: StateView, viewer: PlayerId, kind: DevMenu): ObjectId[] {
  if (kind === 'hand') return view.players.find((player) => player.seat === viewer)?.hand.cards ?? []
  const wanted = (controller: PlayerId): boolean => (kind === 'permanent') === (controller === viewer)
  const found = view.objects.filter(
    (object) =>
      object.zone === 'Battlefield' && wanted(object.controller) && object.types.includes('Creature'),
  )
  const rank = (counters: number): number => (counters > 0 ? 0 : 1)
  return [...found]
    .sort((a, b) => rank(a.counters?.length ?? 0) - rank(b.counters?.length ?? 0))
    .map((object) => object.id)
}

/** Sends the `contextmenu` event of a right click to the first of `ids` that is on the page; false while none is. */
export function rightClickFirst(ids: readonly ObjectId[], root: ParentNode = document): boolean {
  for (const id of ids) {
    const element = root.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)
    if (element === null) continue
    const { left, top, width, height } = element.getBoundingClientRect()
    element.dispatchEvent(
      new MouseEvent('contextmenu', {
        bubbles: true,
        cancelable: true,
        clientX: left + width / 2,
        clientY: top + height / 2,
      }),
    )
    return true
  }
  return false
}

/** Opens the menu of `ids` once a card of them is drawn; nothing for `undefined`. */
export function useOpenDevMenu(ids: readonly ObjectId[] | undefined): void {
  useEffect(() => {
    if (ids === undefined) return
    let timer = 0
    let left = TRIES
    // A timer and not a frame: a page that is not shown draws no frames.
    const tick = (): void => {
      if (rightClickFirst(ids) || (left -= 1) <= 0) return
      timer = window.setTimeout(tick, 100)
    }
    timer = window.setTimeout(tick, 100)
    return () => window.clearTimeout(timer)
  }, [ids])
}
