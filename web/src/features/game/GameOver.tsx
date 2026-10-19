import { Link } from '@tanstack/react-router'
import { FlagIcon, HandshakeIcon, PlusIcon, SwordsIcon, TrophyIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useFitWindow } from '@/components/appChrome'
import { PageShell } from '@/components/PageShell'
import { Pill } from '@/components/Pill'
import { buttonVariants } from '@/components/ui/button'
import { useAuth } from '@/features/auth/context'
import { cn } from '@/lib/utils'
import { useGame, usePlay } from './gameContext'
import { describeResult, endKind, seatOutcomes } from './outcome'
import type { EndKind } from './outcome'
import { seatName } from './useGameInfo'

/** The picture of each ending: a trophy for a win, crossed swords for a loss, a handshake, a flag for a concession. */
const ICON: Record<EndKind, LucideIcon> = {
  won: TrophyIcon,
  lost: SwordsIcon,
  drawn: HandshakeIcon,
  conceded: FlagIcon,
}

/** The small line above the headline; a concession and a seat that left say so here as well as in the sentence. */
const EYEBROW: Record<EndKind, string> = {
  won: 'Game over',
  lost: 'Game over',
  drawn: 'Game over',
  conceded: 'Game over · Conceded',
}

/**
 * The end of the game: who won and how, how each seat finished, and the ways on (a new game, the Play page and, for
 * an account, My games). The page fits the window under the header; only the list of seats scrolls, so the headline
 * and the buttons stay in view at any size.
 */
export function GameOver() {
  useFitWindow()
  const { lookup } = useGame()
  const { result, seat, view } = usePlay(
    useShallow((state) => ({ result: state.result, seat: state.seat, view: state.view })),
  )
  const signedIn = useAuth((state) => state.status === 'signed-in')
  const heading = useRef<HTMLHeadingElement>(null)
  useEffect(() => heading.current?.focus(), [])

  if (result === null) return null
  const name = (who: number): string => seatName(lookup, who)
  const players = view?.players ?? []
  const { headline, detail } = describeResult(result, seat, name, players)
  const seats = seatOutcomes(result, seat, name, players)
  const kind = endKind(result, seat, players)
  const Icon = ICON[kind]

  return (
    <PageShell className="flex min-h-0 flex-1 flex-col gap-6 sm:py-6">
      <section aria-labelledby="over-title" data-end={kind} className="flex min-h-0 flex-1 flex-col gap-6">
        <header className="flex shrink-0 items-start gap-4">
          <span className="flex size-12 shrink-0 items-center justify-center rounded-full bg-muted text-foreground">
            <Icon aria-hidden="true" className="size-6" />
          </span>
          <div className="min-w-0">
            <p className="text-sm font-medium text-muted-foreground">{EYEBROW[kind]}</p>
            <h1
              id="over-title"
              ref={heading}
              tabIndex={-1}
              className="text-display font-semibold tracking-tight break-words outline-none"
            >
              {headline}
            </h1>
            {detail === null ? null : <p className="mt-1 text-base text-muted-foreground">{detail}</p>}
          </div>
        </header>
        {seats.length === 0 ? null : (
          // A long table of seats scrolls inside its list, so the headline and the buttons stay in the window.
          <ul
            aria-label="How each seat finished"
            className="surface min-h-0 divide-y overflow-y-auto overscroll-contain"
          >
            {seats.map((entry) => (
              <li
                key={entry.seat}
                className="grid gap-1 px-4 py-3 text-sm sm:grid-cols-[1fr_auto] sm:items-center sm:gap-x-4"
              >
                <span className="flex min-w-0 items-center gap-2">
                  <span className="shrink-0 text-muted-foreground">Seat {entry.seat + 1}</span>
                  <span className="truncate font-medium">{entry.name}</span>
                  {entry.you ? <Pill>you</Pill> : null}
                </span>
                <span className="text-left sm:text-right">
                  <span className={cn(entry.status === 'Winner' && 'font-medium text-success')}>
                    {entry.status}
                  </span>
                  <span className="text-muted-foreground"> · life {entry.life}</span>
                </span>
              </li>
            ))}
          </ul>
        )}
        <div className="grid shrink-0 grid-cols-2 gap-3 sm:flex sm:flex-wrap">
          <Link to="/new" className={cn(buttonVariants({ size: 'lg' }), 'col-span-2 sm:col-auto')}>
            <PlusIcon aria-hidden="true" />
            Create another game
          </Link>
          <Link
            to="/"
            className={cn(
              buttonVariants({ size: 'lg', variant: 'outline' }),
              !signedIn && 'col-span-2 sm:col-auto',
            )}
          >
            Back to Play
          </Link>
          {signedIn ? (
            <Link to="/games" className={buttonVariants({ size: 'lg', variant: 'outline' })}>
              My games
            </Link>
          ) : null}
        </div>
      </section>
    </PageShell>
  )
}
