import { usePlay } from '@/features/game/gameContext'
import { useAfterDelay } from '@/hooks/useAfterDelay'
import { cn } from '@/lib/utils'
import { CombatLegend } from './combat/CombatLegend'
import { useSeatNamer } from './names'
import { OpeningStatus } from './OpeningStatus'
import { AwayMark } from './PodParts'
import { awaySeats, awayText, untimed } from './presenceText'
import { NothingToDoBanner } from './prompt/NothingToDoBanner'
import { ownDecision } from './prompt/ownDecision'
import { bannerInputOf, isYourTurn } from './selectors'
import { StackChips } from './StackChips'
import { TurnTicker } from './turn/TurnTicker'
import { useIsReplay } from './tableOptions'
import { priorityText, replayLine, waitingOn, waitingText } from './turnText'
import { useStable } from './useStable'

/** The note that this seat has nothing to do or that a play was taken back; drawn only while it says something. */
function NothingToDoNote({ prompt }: { prompt: boolean }) {
  return (
    <div
      // On one line the note keeps its few words whole and the stack chips give way to it. A phone's line is the
      // stack's and the waiting line's, but for a play taken back, which is said there in one truncated line (the
      // whole sentence in its title and for a reader).
      className="hidden shrink-0 has-data-[slot=nothing-to-do]:block has-data-[slot=rewound]:block max-sm:min-w-0 max-sm:shrink max-sm:[&:not(:has([data-slot=rewound]))]:hidden! max-sm:[&_[data-slot=nothing-to-do]]:hidden max-sm:[&_[data-slot=rewound]]:max-w-full max-sm:[&_[data-slot=rewound]]:truncate [&_[data-slot=nothing-to-do-region]]:basis-auto"
    >
      <NothingToDoBanner beside={prompt} />
    </div>
  )
}

/** How long a sent answer may wait for the server's next word before the strip says it is working (Doherty). */
export const WORKING_AFTER_MS = 400

/**
 * A quiet working mark: drawn once an answer this seat sent has waited `WORKING_AFTER_MS` for the server, gone with the
 * server's next word. A small turning ring; under reduced motion a still one.
 */
function WorkingMark() {
  const waiting = usePlay((state) => state.answered !== null)
  const shown = useAfterDelay(waiting, WORKING_AFTER_MS)
  if (!shown) return null
  return (
    <span
      data-slot="working-mark"
      role="status"
      title="Sent; waiting for the server"
      className="ms-2 inline-flex size-4 shrink-0 items-center justify-center motion-safe:animate-in motion-safe:fade-in motion-safe:duration-base"
    >
      <span
        aria-hidden="true"
        className="size-3 rounded-full border-2 border-muted-foreground/40 border-t-muted-foreground motion-safe:animate-spin"
      />
      <span className="sr-only">Sent, waiting for the server</span>
    </span>
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
        <p data-slot="priority" title={holder} className="text-muted-foreground max-sm:sr-only">
          {holder}
        </p>
      )}
      <CombatLegend />
    </div>
  )
}

/**
 * The shared strip between the opponents and your board: one status line of fixed height in every prompt and between
 * prompts. The spells on the stack as chips, then the status (who has priority, what the game waits for, the notes
 * and the opening hands) and the ticker of the last batch. It asks nothing: a decision is answered
 * in its home (`prompt/home.ts`), never here.
 */
export function SharedStrip({ inset = 0 }: { inset?: number }) {
  const name = useSeatNamer()
  const stage = usePlay((state) => state.view?.stage ?? 'PreGame')
  const priority = usePlay((state) => state.view?.priority ?? null)
  const replay = useIsReplay()
  const waiting = usePlay(useStable(waitingOn))
  // A replay waits for nobody: its line says whose turn and step it is at this step, as the turn badge does.
  const line = usePlay((state) => (replay ? replayLine(bannerInputOf(state), name) : null))
  const away = usePlay(useStable((state) => (replay ? [] : awaySeats(state, waitingOn(state)))))
  const status = line ?? waitingText(waiting, name)
  const mine = usePlay((state) => ownDecision(state) !== null)
  const yourTurn = usePlay(isYourTurn)
  const holder = priorityText(priority, stage, name)
  const seat = usePlay((state) => state.seat)
  // Beside this seat's own decision, "You have priority" says what the home already shows.
  const ownPriority = priority !== null && priority.holder === seat
  return (
    <section
      aria-label="Turn and priority"
      data-region="shared"
      data-your-turn={yourTurn || undefined}
      className="relative z-20 min-h-0 max-sm:[&_[data-slot=combat-key]]:hidden"
    >
      <div
        data-slot="shared-sheet"
        data-fx-anchor="stack"
        data-your-turn={yourTurn || undefined}
        // The strip starts beside the pod column, which keeps the life of the seat above it in view.
        style={inset === 0 ? undefined : { left: inset }}
        // The edge takes the colour of the phase; on your turn the gold glow of `your-turn-cue` lies outside it.
        data-themed=""
        className={cn(
          'strip-room absolute inset-0 flex items-center overflow-hidden plaque rounded-xl border-2 border-phase px-3 max-sm:px-2',
          yourTurn && 'your-turn-cue',
        )}
      >
        <div data-slot="strip-row" className="flex min-w-0 flex-1 items-center gap-x-3">
          <div
            data-slot="strip-stack"
            className="flex min-w-0 shrink-[4] items-center overflow-hidden flex-1"
          >
            <StackChips />
          </div>
          <div data-slot="strip-status" className="flex min-w-0 items-center gap-x-3 whitespace-nowrap">
            {mine ? null : (
              <p
                data-slot="waiting-line"
                aria-live="polite"
                aria-atomic="true"
                title={status}
                className="min-w-0 shrink truncate text-sm font-medium"
              >
                {status}
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
            {mine ? null : <WorkingMark />}
            <NothingToDoNote prompt={mine} />
            {/* A phone's bar says who is still deciding beside Keep and Mulligan. */}
            <div className="contents max-sm:hidden">
              <OpeningStatus />
            </div>
            <StripMeta holder={mine && ownPriority ? '' : holder} className="ms-auto shrink-0 items-end" />
          </div>
          <div className="max-w-[40%] min-w-0 shrink">
            <TurnTicker />
          </div>
        </div>
      </div>
    </section>
  )
}
