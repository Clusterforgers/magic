import { useNavigate } from '@tanstack/react-router'
import { ArrowRightIcon, EyeIcon, SkullIcon, TrophyIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { useGame, usePlay } from '@/features/game/gameContext'
import { describeResult, seatOutcomes } from '@/features/game/outcome'
import { seatName } from '@/features/game/useGameInfo'
import { GameTable } from '@/features/table/GameTable'
import { LogPanel } from '@/features/table/LogPanel'
import { winningSeats } from '@/lib/teams'
import { cn } from '@/lib/utils'

/** Seconds the result waits before it goes on to the run by itself, unless the player stays. */
const CONTINUE_AFTER_S = 60

/**
 * The end of a Delves boss fight: the final table stays, under a result that says whether the party won or lost and
 * how (each seat's end and the last turns of the log), with "Continue to the run". It goes on by itself after
 * `CONTINUE_AFTER_S` seconds, counted down in sight, unless the player presses "Stay"; "Look at the table" folds the
 * result into a bar that opens it again.
 */
export function FightEnd({ delve }: { delve: string }) {
  const navigate = useNavigate()
  const [open, setOpen] = useState(true)
  const [left, setLeft] = useState<number | null>(CONTINUE_AFTER_S)
  const go = (): void => {
    void navigate({ to: '/delves/$delveId', params: { delveId: delve } })
  }
  const counting = left !== null
  useEffect(() => {
    if (!counting) return undefined
    const tick = window.setInterval(
      () => setLeft((now) => (now === null ? null : Math.max(0, now - 1))),
      1000,
    )
    return () => window.clearInterval(tick)
  }, [counting])
  useEffect(() => {
    if (left === 0) void navigate({ to: '/delves/$delveId', params: { delveId: delve } })
  }, [left, navigate, delve])
  const stay = (): void => setLeft(null)

  return (
    <>
      <GameTable />
      {open ? (
        <FightResult
          left={left}
          onContinue={go}
          onStay={stay}
          onLook={() => {
            stay()
            setOpen(false)
          }}
        />
      ) : (
        <div className="pointer-events-none fixed inset-x-0 top-11 z-50 mx-auto max-w-page px-4">
          <section
            aria-label="Fight over, looking back"
            data-slot="fight-bar"
            className="pointer-events-auto flex flex-wrap items-center gap-2 rounded-lg border bg-popover py-1 pr-1 pl-3 text-sm text-popover-foreground shadow-e2"
          >
            <span className="font-medium">Fight over</span>
            <span className="text-muted-foreground">Final table, read only</span>
            <span className="ml-auto flex gap-1">
              <Button size="sm" variant="outline" data-slot="fight-result-open" onClick={() => setOpen(true)}>
                Show result
              </Button>
              <Button size="sm" data-slot="fight-continue" onClick={go}>
                Continue to the run
              </Button>
            </span>
          </section>
        </div>
      )}
    </>
  )
}

function FightResult(props: {
  left: number | null
  onContinue: () => void
  onStay: () => void
  onLook: () => void
}) {
  const { lookup } = useGame()
  const { result, seat, view } = usePlay(
    useShallow((state) => ({ result: state.result, seat: state.seat, view: state.view })),
  )
  const heading = useRef<HTMLHeadingElement>(null)
  useEffect(() => heading.current?.focus(), [])
  if (result === null) return null
  const name = (who: number): string => seatName(lookup, who)
  const players = view?.players ?? []
  const teams = view?.house_rules?.teams ?? lookup.game?.house_rules?.teams ?? null
  // The party is team 0 of a fight (docs/delves.md), so its win reads before the table's teams are known.
  const partyWon = result !== 'Draw' && 'Team' in result && result.Team === 0
  const won = partyWon || (seat !== null && winningSeats(result, teams).includes(seat))
  const { detail } = describeResult(result, seat, name, players, teams)
  const seats = seatOutcomes(result, seat, name, players, teams)
  const Icon = won ? TrophyIcon : SkullIcon
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-scrim p-4">
      <section
        role="dialog"
        aria-modal="false"
        aria-labelledby="fight-over-title"
        data-slot="fight-result"
        data-won={won}
        className="flex max-h-full w-full max-w-xl flex-col gap-4 rounded-xl border bg-popover p-4 text-popover-foreground shadow-e3"
      >
        <header className="flex shrink-0 items-start gap-3">
          <span
            className={cn(
              'flex size-11 shrink-0 items-center justify-center rounded-full',
              won ? 'bg-success/15 text-success' : 'bg-destructive/15 text-destructive',
            )}
          >
            <Icon aria-hidden="true" className="size-6" />
          </span>
          <div className="min-w-0">
            <p className="text-sm font-medium text-muted-foreground">Fight over</p>
            <h2
              id="fight-over-title"
              ref={heading}
              tabIndex={-1}
              className="text-2xl font-semibold tracking-tight outline-none"
            >
              {won ? 'The party won the fight' : 'The party lost the fight'}
            </h2>
            {detail === null ? null : (
              <p data-slot="fight-how" className="mt-1 text-sm text-muted-foreground">
                {detail}
              </p>
            )}
          </div>
        </header>
        {seats.length === 0 ? null : (
          <ul aria-label="How each seat finished" className="grid shrink-0 gap-1 text-sm">
            {seats.map((entry) => (
              <li key={entry.seat} className="flex min-w-0 items-baseline gap-2">
                <span className="truncate font-medium">{entry.name}</span>
                <span
                  className={cn(
                    'truncate',
                    entry.status === 'Winner' ? 'text-success' : 'text-muted-foreground',
                  )}
                >
                  {entry.status}
                </span>
                <span className="ml-auto shrink-0 text-muted-foreground tabular-nums">life {entry.life}</span>
              </li>
            ))}
          </ul>
        )}
        <div className="flex min-h-0 flex-col gap-1">
          <h3 className="text-sm font-medium">The last turns</h3>
          <div data-slot="fight-log" className="h-48 min-h-0 rounded-md border bg-background/50">
            <LogPanel />
          </div>
        </div>
        <footer className="flex shrink-0 flex-wrap items-center gap-2">
          <Button data-slot="fight-continue" onClick={props.onContinue}>
            Continue to the run
            <ArrowRightIcon aria-hidden="true" />
          </Button>
          <Button variant="outline" data-slot="fight-look" onClick={props.onLook}>
            <EyeIcon aria-hidden="true" />
            Look at the table
          </Button>
          {props.left === null ? null : (
            <span className="ml-auto flex items-center gap-2 text-sm text-muted-foreground">
              <span data-slot="fight-countdown" aria-live="off" className="tabular-nums">
                Continuing in {props.left} s
              </span>
              <Button size="sm" variant="ghost" data-slot="fight-stay" onClick={props.onStay}>
                Stay
              </Button>
            </span>
          )}
        </footer>
      </section>
    </div>
  )
}
