import { useRef } from 'react'
import type { CSSProperties } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { CombatLegend } from './combat/CombatLegend'
import { useSeatNamer } from './names'
import { OpeningStatus } from './OpeningStatus'
import { AwayMark } from './PodParts'
import { awaySeats, awayText, untimed } from './presenceText'
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

/** The note that this seat has nothing to do or that a play was taken back; drawn only while it says something. */
function NothingToDoNote() {
  return (
    <div className="hidden min-w-0 shrink-[2] has-data-[slot=nothing-to-do]:block has-data-[slot=rewound]:block [&_[data-slot=nothing-to-do-region]]:basis-auto [&_p]:max-w-full [&_p]:truncate">
      <NothingToDoBanner />
    </div>
  )
}

/**
 * The shared strip between the opponents and your board: the spells on the stack as chips, who has priority and what
 * the game is waiting for. When this seat must decide, the prompt (`prompt/PromptHost`) takes the place of the
 * read-only line; `data-slot="decision-line"` marks both. A prompt that needs more than the strip grows into a sheet
 * upward into the room the opponents' area leaves free for it (`useSheetOverhang`), at most `sheetMax` px high, and
 * scrolls inside itself past that. It never grows down over your board, which holds the cards a prompt asks you to
 * click, nor over the hand dock or the controls, and it lies over no card of the opponents' boards.
 */
export function SharedStrip({
  sheetMax,
  inset = 0,
  onOverhang,
}: {
  sheetMax: number
  inset?: number
  /** Hears how far the sheet stands above the strip, in px. */
  onOverhang?: ((px: number) => void) | undefined
}) {
  const name = useSeatNamer()
  const stage = usePlay((state) => state.view?.stage ?? 'PreGame')
  const priority = usePlay((state) => state.view?.priority ?? null)
  const waiting = usePlay(useStable(waitingOn))
  const away = usePlay(useStable((state) => awaySeats(state, waitingOn(state))))
  const mine = usePlay((state) => ownDecision(state) !== null)
  const yourTurn = usePlay(isYourTurn)
  const holder = priorityText(priority, stage, name)
  const strip = useRef<HTMLElement>(null)
  const sheet = useRef<HTMLDivElement>(null)
  useEdgeFade(sheet)
  useSheetOverhang(strip, sheet, onOverhang)
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
        data-fx-anchor="stack"
        data-your-turn={yourTurn || undefined}
        // The sheet starts beside the pod column, which keeps the life of the seat above it in view.
        style={inset === 0 ? undefined : { left: inset }}
        // The edge takes the colour of the phase; on your turn the gold glow of `your-turn-cue` lies outside it.
        data-themed=""
        className={cn(
          'fade-scroll absolute inset-x-0 bottom-0 max-h-(--sheet-max) min-h-full plaque rounded-xl border-2 border-phase px-3 py-1',
          yourTurn && 'your-turn-cue',
        )}
      >
        <div className="flex min-h-full flex-col">
          {/*
            A prompt or the opening hands may wrap the row: they need the room. Without one the row stays one line, the strip's own height, and
            gives way from the stack chips first (they fold into "+N more"), then from the notes, which truncate.
          */}
          <div
            data-slot="strip-row"
            className={cn(
              'flex min-h-0 flex-1 items-center gap-x-3 gap-y-1',
              mine || stage === 'Mulligan' ? 'flex-wrap' : 'flex-wrap sm:flex-nowrap',
            )}
          >
            <div className="flex min-w-0 shrink-[4] items-center overflow-hidden sm:has-data-[slot=stack-chips]:min-w-56">
              <StackChips />
            </div>
            {/* Room for the prompt beside the chips (it takes what they leave, down to 20 rem), so the two share a line. */}
            <div
              className={cn(
                mine
                  ? 'min-w-0 flex-1 basis-80'
                  : 'flex max-w-full shrink-0 items-center [&_p]:max-w-full [&_p]:truncate',
              )}
            >
              {mine ? (
                <PromptHost />
              ) : (
                <p
                  data-slot="decision-line"
                  aria-live="polite"
                  aria-atomic="true"
                  title={waitingText(waiting, name)}
                  className="w-fit rounded-md bg-muted px-2 py-1 text-sm font-medium"
                >
                  {waitingText(waiting, name)}
                  {away.length === 0 ? null : (
                    <span data-slot="away-note">
                      {' · '}
                      <AwayMark className="mr-1" />
                      {/* The seconds tick away to the eye; the live line speaks only who is away, once. */}
                      <span aria-hidden="true">{awayText(away, name)}</span>
                      <span className="sr-only">{awayText(untimed(away), name)}</span>
                    </span>
                  )}
                </p>
              )}
            </div>
            <NothingToDoNote />
            <OpeningStatus />
            {/*
              Who has priority over the key to the combat lines, two short lines at the end of the row: together they take
              the height of a stack chip, not a row of their own.
            */}
            <div
              data-slot="strip-meta"
              className="ms-auto flex shrink-0 flex-col items-end whitespace-nowrap justify-center gap-0.5 text-xs leading-4 empty:hidden [&_[data-slot=combat-key]]:flex-nowrap [&_[data-slot=combat-key]]:gap-x-3"
            >
              {holder === '' ? null : (
                <p data-slot="priority" className="text-muted-foreground max-sm:sr-only">
                  {holder}
                </p>
              )}
              <CombatLegend />
            </div>
          </div>
          <TurnTicker />
        </div>
      </div>
    </section>
  )
}
