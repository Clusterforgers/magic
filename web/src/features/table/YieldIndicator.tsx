import { useDeferredValue } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { useGame, usePlay } from '@/features/game/gameContext'
import { YIELD_WORDS } from './prompt/passWords'

/**
 * The quiet line that says the server is passing priority for this seat ("Passing until the end of turn") with the
 * button that stops it. It shows the server's word (`yielding`), so it appears with the first pass and goes with the
 * last, whatever ended the yield; nothing is drawn while the seat is not yielding. The words are said once, politely,
 * when they appear.
 */
export function YieldIndicator() {
  const { session } = useGame()
  const { yielding, online } = usePlay(
    useShallow((state) => ({ yielding: state.yielding, online: state.status === 'open' })),
  )
  // The first render has no words; the deferred one that follows has them, which is when screen readers announce.
  const words = useDeferredValue(yielding === null ? '' : YIELD_WORDS[yielding].passing, '')
  if (yielding === null) return null
  return (
    <div
      data-slot="yield-indicator"
      data-yield={yielding}
      className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-md px-2 py-1 text-sm text-muted-foreground"
    >
      <p role="status" aria-live="polite" aria-atomic="true">
        {words}
      </p>
      <Button
        size="sm"
        variant="outline"
        className="max-sm:h-9 max-sm:px-3"
        disabled={!online}
        onClick={() => session.cancelYield()}
      >
        Stop passing
      </Button>
    </div>
  )
}
