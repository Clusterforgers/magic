import { Link } from '@tanstack/react-router'
import { useEffect, useRef } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { PageShell } from '@/components/PageShell'
import { Pill } from '@/components/Pill'
import { buttonVariants } from '@/components/ui/button'
import { useGame, usePlay } from './gameContext'
import { describeResult, seatOutcomes } from './outcome'
import { seatName } from './useGameInfo'

/** The end of the game: who won, a draw or a concession, how each seat finished, and the way to a new game. */
export function GameOver() {
  const { lookup } = useGame()
  const { result, seat, view } = usePlay(
    useShallow((state) => ({ result: state.result, seat: state.seat, view: state.view })),
  )
  const heading = useRef<HTMLHeadingElement>(null)
  useEffect(() => heading.current?.focus(), [])

  if (result === null) return null
  const name = (who: number): string => seatName(lookup, who)
  const players = view?.players ?? []
  const { headline, detail } = describeResult(result, seat, name, players)
  const seats = seatOutcomes(result, seat, name, players)

  return (
    <PageShell className="grid gap-6">
      <section aria-labelledby="over-title" className="grid gap-6">
        <div>
          <p className="text-sm text-muted-foreground">Game over</p>
          <h1
            id="over-title"
            ref={heading}
            tabIndex={-1}
            className="text-display font-semibold tracking-tight outline-none"
          >
            {headline}
          </h1>
          {detail === null ? null : <p className="mt-1 text-base text-muted-foreground">{detail}</p>}
        </div>
        {seats.length === 0 ? null : (
          // A long table of seats scrolls inside its list, so the page and its button stay in the window.
          <ul
            aria-label="How each seat finished"
            className="surface max-h-[min(24rem,45dvh)] divide-y overflow-y-auto"
          >
            {seats.map((entry) => (
              <li key={entry.seat} className="flex items-center justify-between gap-3 px-4 py-3 text-sm">
                <span className="flex min-w-0 items-center gap-2">
                  <span className="shrink-0 text-muted-foreground">Seat {entry.seat + 1}</span>
                  <span className="truncate font-medium">{entry.name}</span>
                  {entry.you ? <Pill>you</Pill> : null}
                </span>
                <span className="shrink-0 text-right text-sm">
                  {entry.status}
                  <span className="text-muted-foreground"> · life {entry.life}</span>
                </span>
              </li>
            ))}
          </ul>
        )}
        <div>
          <Link to="/" className={buttonVariants({ size: 'lg' })}>
            Create a new game
          </Link>
        </div>
      </section>
    </PageShell>
  )
}
