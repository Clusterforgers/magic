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
import { TurnTicker } from './turn/TurnTicker'
import { priorityText, waitingOn, waitingText } from './turnText'
import { useEdgeFade } from './useEdgeFade'
import { useSheetOverhang } from './useSheetOverhang'
import { useStable } from './useStable'

/**
 * The shared strip between the opponents and your board: the spells on the stack as chips, who has priority and what
 * the game is waiting for. When this seat must decide, the prompt (`prompt/PromptHost`) takes the place of the
 * read-only line; `data-slot="decision-line"` marks both. A prompt that needs more than the strip grows into a sheet
 * upward into the room the opponents' area leaves free for it (`useSheetOverhang`), at most `sheetMax` px high, and
 * scrolls inside itself past that. It never grows down over your board, which holds the cards a prompt asks you to
 * click, nor over the hand dock or the controls, and it lies over no card of the opponents' boards.
 */
export function SharedStrip({ sheetMax, inset = 0 }: { sheetMax: number; inset?: number }) {
  const name = useSeatNamer()
  const stage = usePlay((state) => state.view?.stage ?? 'PreGame')
  const priority = usePlay((state) => state.view?.priority ?? null)
  const waiting = usePlay(useStable(waitingOn))
  const mine = usePlay((state) => ownDecision(state) !== null)
  const yourTurn = usePlay(isYourTurn)
  const holder = priorityText(priority, stage, name)
  const strip = useRef<HTMLElement>(null)
  const sheet = useRef<HTMLDivElement>(null)
  useEdgeFade(sheet)
  useSheetOverhang(strip, sheet)
  return (
    <section
      ref={strip}
      aria-label="Turn and priority"
      data-region="shared"
      data-your-turn={yourTurn || undefined}
      style={{ '--sheet-max': `${Math.floor(sheetMax)}px` } as CSSProperties}
      // The prompt's title ("Your decision: ...") is for a screen reader; its sentence says the same to the eye in a
      // line less. On a phone the key to the combat lines goes too.
      className="relative z-20 min-h-0 max-sm:[&_[data-slot=combat-key]]:hidden [&_[data-slot=prompt-title]]:sr-only"
    >
      <div
        ref={sheet}
        data-slot="shared-sheet"
        data-your-turn={yourTurn || undefined}
        // The sheet starts beside the pod column, which keeps the life of the seat above it in view.
        style={inset === 0 ? undefined : { left: inset }}
        // The edge takes the colour of the phase; on your turn the gold glow of `your-turn-cue` lies outside it.
        data-themed=""
        className={cn(
          'fade-scroll absolute inset-x-0 bottom-0 max-h-(--sheet-max) min-h-full rounded-xl border-2 border-phase bg-card px-3 py-1',
          yourTurn && 'your-turn-cue',
        )}
      >
        <div className="flex min-h-full flex-col">
          <div className="flex min-h-0 flex-1 flex-wrap items-center gap-x-4 gap-y-1">
            <StackChips />
            {/* Room for the prompt beside the chips (it takes what they leave, down to 20 rem), so the two share a line. */}
            <div className="min-w-0 flex-1 basis-80">
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
            </div>
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
          <TurnTicker />
        </div>
      </div>
    </section>
  )
}
