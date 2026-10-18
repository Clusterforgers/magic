import { Link } from '@tanstack/react-router'
import { useEffect, useRef } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Badge } from '@/components/ui/badge'
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
    <div className="mx-auto grid w-full max-w-2xl gap-6 px-4 py-6 sm:px-6">
      <section aria-labelledby="over-title" className="grid gap-5">
        <div>
          <p className="text-sm text-muted-foreground">Game over</p>
          <h1
            id="over-title"
            ref={heading}
            tabIndex={-1}
            className="text-3xl font-semibold tracking-tight outline-none"
          >
            {headline}
          </h1>
          {detail === null ? null : <p className="mt-1 text-muted-foreground">{detail}</p>}
        </div>
        {seats.length === 0 ? null : (
          <ul
            aria-label="How each seat finished"
            className="divide-y rounded-xl bg-card ring-1 ring-foreground/10"
          >
            {seats.map((entry) => (
              <li key={entry.seat} className="flex items-center justify-between gap-3 px-3 py-2.5 text-sm">
                <span className="flex min-w-0 items-center gap-2">
                  <span className="shrink-0 text-muted-foreground">Seat {entry.seat + 1}</span>
                  <span className="truncate font-medium">{entry.name}</span>
                  {entry.you ? <Badge variant="secondary">you</Badge> : null}
                </span>
                <span className="shrink-0 text-right text-xs">
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
    </div>
  )
}
