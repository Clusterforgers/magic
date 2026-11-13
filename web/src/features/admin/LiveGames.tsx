import { CircleAlertIcon, CircleCheckIcon, SquareIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Pill } from '@/components/Pill'
import { Alert, AlertAction, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { timeSince } from '@/lib/timeSince'
import type { AdminGamesApi } from '@/net/adminGames'
import type { AdminGame } from '@/protocol/generated'
import { EndGameDialog } from './EndGameDialog'
import { gameTitle, modeOf } from './gameWords'

type Load =
  { status: 'loading' } | { status: 'ready'; games: AdminGame[] } | { status: 'error'; message: string }

/** Every lobby and running game, newest first, each with an "End game" button that asks first. */
export function LiveGames({ api }: { api: AdminGamesApi }) {
  const [load, setLoad] = useState<Load>({ status: 'loading' })
  const [now, setNow] = useState(() => Date.now())
  const [ending, setEnding] = useState<AdminGame | null>(null)
  // The game just ended, said once; the heading takes the focus the removed row leaves behind.
  const [ended, setEnded] = useState<string | null>(null)
  const heading = useRef<HTMLHeadingElement>(null)

  // Bumped by Refresh and Try again; each value loads the list once.
  const [round, setRound] = useState(0)
  const refresh = () => setRound((n) => n + 1)

  useEffect(() => {
    let live = true
    api.listGames().then(
      (games) => {
        if (!live) return
        setNow(Date.now())
        setLoad({ status: 'ready', games })
      },
      (error: unknown) => {
        if (!live) return
        setLoad({ status: 'error', message: error instanceof Error ? error.message : 'Unknown error.' })
      },
    )
    return () => {
      live = false
    }
  }, [api, round])

  useEffect(() => {
    if (ended !== null) heading.current?.focus()
  }, [ended])

  const games = load.status === 'ready' ? load.games : []

  return (
    <section aria-labelledby="live-games-title" className="surface grid gap-4 p-4 sm:p-6">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="grid gap-1">
          <h2
            ref={heading}
            id="live-games-title"
            tabIndex={-1}
            className="text-lg font-semibold tracking-tight outline-none"
          >
            Live games
          </h2>
          <p className="text-sm text-muted-foreground">
            {load.status === 'ready'
              ? `${games.length} ${games.length === 1 ? 'game' : 'games'} in a lobby or running, newest first.`
              : 'Every lobby and running game on this server.'}
          </p>
        </div>
        <Button type="button" variant="outline" size="sm" onClick={refresh}>
          Refresh
        </Button>
      </div>

      <div role="status" className="empty:hidden">
        {ended === null ? null : (
          <p className="flex items-start gap-2 text-sm">
            <CircleCheckIcon aria-hidden="true" className="size-4 shrink-0 translate-y-0.5 text-success" />
            <span>{`${ended} was ended.`}</span>
          </p>
        )}
      </div>

      {load.status === 'error' ? (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertDescription className="text-foreground">{`Could not load the games. ${load.message}`}</AlertDescription>
          <AlertAction>
            <Button type="button" variant="ghost" size="xs" onClick={refresh}>
              Try again
            </Button>
          </AlertAction>
        </Alert>
      ) : null}

      {load.status === 'loading' ? (
        <div role="status" aria-label="Loading the games" className="grid gap-2">
          <Skeleton className="h-6 w-full" />
          <Skeleton className="h-6 w-2/3" />
        </div>
      ) : load.status === 'ready' && games.length === 0 ? (
        <p className="text-sm text-muted-foreground">No game is in a lobby or running.</p>
      ) : (
        <ul className="divide-y divide-border">
          {games.map((game) => (
            <li key={game.game} className="grid gap-1 py-3 first:pt-0 last:pb-0">
              <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
                <span className="min-w-0 flex-1 font-medium break-words">{gameTitle(game)}</span>
                <Pill tone={game.started ? 'success' : 'neutral'}>{game.started ? 'Running' : 'Lobby'}</Pill>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  aria-label={`End ${gameTitle(game)}`}
                  onClick={() => setEnding(game)}
                >
                  <SquareIcon aria-hidden="true" />
                  End game
                </Button>
              </div>
              <p className="flex flex-wrap gap-x-3 text-sm text-muted-foreground">
                <span>{modeOf(game)}</span>
                <time dateTime={game.created_at} title={game.created_at}>
                  Created {timeSince(game.created_at, now)}
                </time>
                <time dateTime={game.last_activity} title={game.last_activity}>
                  Last active {timeSince(game.last_activity, now)}
                </time>
                <span className="font-mono text-xs leading-5 break-all">{game.game}</span>
              </p>
            </li>
          ))}
        </ul>
      )}

      {ending === null ? null : (
        <EndGameDialog
          api={api}
          game={ending.game}
          title={gameTitle(ending)}
          open
          onOpenChange={(open) => {
            if (!open) setEnding(null)
          }}
          onEnded={() => {
            setEnded(gameTitle(ending))
            setLoad((current) =>
              current.status === 'ready'
                ? { status: 'ready', games: current.games.filter((game) => game.game !== ending.game) }
                : current,
            )
          }}
        />
      )}
    </section>
  )
}
