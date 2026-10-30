import { useEffect } from 'react'
import { XIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import { pickedCard, usePick, usePickStore } from './pick'
import './pick.css'

/**
 * The wait for a card, said on the table: a banner "Pick a card for <command>" with a Cancel button, the table's cards
 * marked as targets (`html[data-picking]`, `pick.css`) and Escape to cancel. It also forgets a chosen card that has left
 * the table. Drawn only while a pick is armed.
 */
export function PickBanner() {
  const picks = usePickStore()
  const pick = usePick()
  const armed = pick.phase === 'armed'
  const card = pickedCard(pick)
  const gone = usePlay((state) => card !== null && state.objectsById[card] === undefined)

  useEffect(() => {
    if (picks === null || card === null || !gone) return
    picks.dispatch({ type: 'gone', card })
  }, [picks, card, gone])

  useEffect(() => {
    if (picks === null || !armed) return
    const root = document.documentElement
    root.dataset.picking = ''
    const onKey = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      picks.dispatch({ type: 'cancel' })
    }
    // In the capture phase, before a menu or a dialog takes the key.
    document.addEventListener('keydown', onKey, true)
    return () => {
      delete root.dataset.picking
      document.removeEventListener('keydown', onKey, true)
    }
  }, [picks, armed])

  if (picks === null || pick.phase !== 'armed') return null
  return (
    <div
      role="status"
      data-slot="pick-banner"
      className="fixed top-12 left-1/2 z-[65] flex max-w-[calc(100vw-1rem)] -translate-x-1/2 items-center gap-2 rounded-lg border bg-popover py-1 pr-1 pl-3 text-sm text-popover-foreground shadow-e2"
    >
      <span>
        Pick a card for {pick.for}. <span className="text-muted-foreground">Escape cancels.</span>
      </span>
      <Button size="sm" variant="outline" onClick={() => picks.dispatch({ type: 'cancel' })}>
        <XIcon aria-hidden="true" />
        Cancel
      </Button>
    </div>
  )
}
