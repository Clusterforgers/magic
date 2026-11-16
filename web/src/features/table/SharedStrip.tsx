import { laysOutCards } from './prompt/cardPrompts'
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
function NothingToDoNote({ prompt }: { prompt: boolean }) {
  return (
    <div
      className={cn(
        'hidden min-w-0 shrink-[2] has-data-[slot=nothing-to-do]:block has-data-[slot=rewound]:block [&_[data-slot=nothing-to-do-region]]:basis-auto [&_p]:max-w-full [&_p]:truncate',
        // On a phone without a prompt it shares the second line with the waiting line.
        !prompt && 'max-sm:grow max-sm:basis-0',
        // Beside a prompt it is a line of the status, never wider than 12 rem.
        prompt && 'sm:max-w-48',
      )}
    >
      <NothingToDoBanner beside={prompt} />
    </div>
  )
}

/** Who has priority over the key to the combat lines: two short lines, together the height of a stack chip. */
function StripMeta({ holder, className }: { holder: string; className?: string }) {
  return (
    <div
      data-slot="strip-meta"
      className={cn(
        'flex shrink-0 flex-col justify-center gap-0.5 whitespace-nowrap text-xs leading-4 empty:hidden [&_[data-slot=combat-key]]:flex-nowrap [&_[data-slot=combat-key]]:gap-x-3',
        className,
      )}
    >
      {holder === '' ? null : (
        <p data-slot="priority" className="text-muted-foreground max-sm:sr-only">
          {holder}
        </p>
      )}
      <CombatLegend />
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
  sheetWide,
  inset = 0,
  onOverhang,
}: {
  sheetMax: number
  /** The most the sheet grows while a prompt lays out cards (`Regions.sheetWide`); `sheetMax` when left out. */
  sheetWide?: number
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
  const seat = usePlay((state) => state.seat)
  // Beside this seat's own prompt, "You have priority" says what the band already shows.
  const ownPriority = priority !== null && priority.holder === seat
  // A prompt that lays out cards may take the opponents' boards' room too: its cards are what there is to look at.
  const cards = usePlay(laysOutCards)
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
      style={
        { '--sheet-max': `${Math.floor(cards ? (sheetWide ?? sheetMax) : sheetMax)}px` } as CSSProperties
      }
      // The prompt's title ("Your decision: ...") is for a screen reader; its sentence says the same to the eye in a
      // line less. On a phone the key to the combat lines goes too.
      className="relative z-20 min-h-0 max-sm:[&_[data-slot=combat-key]]:hidden [&_[data-slot=prompt-title]]:sr-only"
    >
      <div
        ref={sheet}
        data-slot="shared-sheet"
        data-fx-anchor="stack"
        // The card preview keeps off the strip while it holds a decision where it can; the strip ranks below the sentence and
        // the buttons (`PromptLayout`), which it covers last.
        data-keep-clear={mine ? 'soft' : undefined}
        data-your-turn={yourTurn || undefined}
        // The sheet starts beside the pod column, which keeps the life of the seat above it in view.
        style={inset === 0 ? undefined : { left: inset }}
        // The edge takes the colour of the phase; on your turn the gold glow of `your-turn-cue` lies outside it.
        data-themed=""
        className={cn(
          'band-room fade-scroll absolute inset-x-0 bottom-0 max-h-(--sheet-max) min-h-full plaque rounded-xl border-2 border-phase px-3 py-1',
          yourTurn && 'your-turn-cue',
        )}
      >
        <div className="flex min-h-full flex-col">
          {/*
            While this seat decides the row is the band (`strip-band`, `promptBand.css`): one grid across the strip of
            the stack chips (at most 20 % of it), the status (the note, the opening hands, who has priority, the combat
            key), the sentence on one line, the choices, the clock and the answers at the right end. The prompt's own
            wrappers are `display: contents`, so their parts are the grid's cells. While it waits the row is one line
            from `sm` up, the strip's own height: it gives way from the stack chips first (they fold into "+N more"),
            then from the notes, which truncate; the opening hands may wrap it. The tree keeps its shape either way,
            so the note keeps what it knows across a prompt.
          */}
          <div
            data-slot="strip-row"
            data-band={mine || undefined}
            data-cards={(mine && cards) || undefined}
            className={cn(
              'min-h-0 flex-1',
              mine
                ? 'strip-band'
                : cn(
                    'flex items-center gap-x-3 gap-y-1',
                    stage === 'Mulligan' ? 'flex-wrap' : 'flex-wrap sm:flex-nowrap',
                  ),
            )}
          >
            <div
              data-slot="strip-stack"
              className={cn(
                'flex min-w-0 items-center overflow-hidden',
                !mine &&
                  'shrink-[4] max-sm:has-data-[slot=stack-chips]:basis-full sm:has-data-[slot=stack-chips]:min-w-56',
              )}
            >
              <StackChips />
            </div>
            <div
              data-slot="strip-prompt"
              className={
                mine
                  ? 'contents'
                  : 'flex max-w-full shrink-0 items-center max-sm:min-w-0 max-sm:shrink max-sm:grow max-sm:basis-0 [&_p]:max-w-full [&_p]:truncate'
              }
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
            <div
              data-slot="strip-status"
              className={
                mine
                  ? 'flex min-w-0 flex-col justify-center gap-0.5 text-xs leading-4 empty:hidden'
                  : 'contents'
              }
            >
              <NothingToDoNote prompt={mine} />
              <OpeningStatus />
              <StripMeta
                holder={mine && ownPriority ? '' : holder}
                className={mine ? undefined : 'ms-auto items-end'}
              />
            </div>
          </div>
          <TurnTicker />
        </div>
      </div>
    </section>
  )
}
