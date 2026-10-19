import { useRef } from 'react'
import type { CSSProperties } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { CombatLegend } from './combat/CombatLegend'
import { useSeatNamer } from './names'
import { OpeningStatus } from './OpeningStatus'
import { NothingToDoBanner } from './prompt/NothingToDoBanner'
import { ownDecision } from './prompt/ownDecision'
import { PromptHost } from './prompt/PromptHost'
import { isYourTurn } from './selectors'
import { StackChips } from './StackChips'
import { priorityText, waitingOn, waitingText } from './turnText'
import { useEdgeFade } from './useEdgeFade'
import { useStable } from './useStable'

/**
 * The shared strip between the opponents and your board: the spells on the stack as chips, who has priority and what
 * the game is waiting for. When this seat must decide, the prompt (`prompt/PromptHost`) takes the place of the
 * read-only line; `data-slot="decision-line"` marks both. A prompt that needs more than the strip grows into a sheet
 * over your board, at most `sheetMax` px high and never over the hand dock or the controls (they sit above it), and
 * scrolls inside itself past that.
 */
export function SharedStrip({ sheetMax }: { sheetMax: number }) {
  const name = useSeatNamer()
  const stage = usePlay((state) => state.view?.stage ?? 'PreGame')
  const priority = usePlay((state) => state.view?.priority ?? null)
  const waiting = usePlay(useStable(waitingOn))
  const mine = usePlay((state) => ownDecision(state) !== null)
  const yourTurn = usePlay(isYourTurn)
  const holder = priorityText(priority, stage, name)
  const sheet = useRef<HTMLDivElement>(null)
  useEdgeFade(sheet)
  return (
    <section
      aria-label="Turn and priority"
      data-region="shared"
      data-your-turn={yourTurn || undefined}
      style={{ '--sheet-max': `${Math.floor(sheetMax)}px` } as CSSProperties}
      // On a phone the strip is a line: the prompt's title ("Your decision: ...") and the key to the combat lines go.
      className="relative z-20 min-h-0 max-sm:[&_[data-slot=combat-key]]:hidden max-sm:[&_[data-slot=prompt-title]]:hidden"
    >
      <div
        ref={sheet}
        data-slot="shared-sheet"
        data-your-turn={yourTurn || undefined}
        className={cn(
          'fade-scroll absolute inset-x-0 top-0 max-h-(--sheet-max) min-h-full rounded-xl bg-card px-3 py-1',
          yourTurn ? 'your-turn-cue' : 'ring-1 ring-foreground/10',
        )}
      >
        <div className="flex min-h-full flex-wrap items-center gap-x-4 gap-y-1">
          <StackChips />
          {mine ? (
            <PromptHost />
          ) : (
            <p
              data-slot="decision-line"
              aria-live="polite"
              aria-atomic="true"
              className="w-fit rounded-md bg-muted px-2 py-1 text-sm font-medium"
            >
              {waitingText(waiting, name)}
            </p>
          )}
          {holder === '' ? null : (
            <p data-slot="priority" className="text-xs text-muted-foreground max-sm:sr-only">
              {holder}
            </p>
          )}
          <OpeningStatus />
          {/* Their own full-width rows are for a strip that is one flex line; here they sit beside the prompt when there is room. */}
          <div className="min-w-0 empty:hidden">
            <CombatLegend />
          </div>
          <div className="hidden min-w-0 has-data-[slot=nothing-to-do]:block">
            <NothingToDoBanner />
          </div>
        </div>
      </div>
    </section>
  )
}
