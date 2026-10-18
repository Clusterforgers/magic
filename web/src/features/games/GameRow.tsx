import { Link } from '@tanstack/react-router'
import { Badge } from '@/components/ui/badge'
import { buttonVariants } from '@/components/ui/button'
import type { MyGame } from '@/protocol/generated'
import { othersText, resultText, STATUS_LABEL, timeSince } from './format'

const BADGE = { lobby: 'secondary', running: 'default', finished: 'outline' } as const

/**
 * One game of the list: its state, how long ago it was made, who else sits at it, how it ended if it did, and a way
 * back in. "Rejoin" opens the game page with no seat link, which connects through the session; a finished game
 * cannot be rejoined (the server forgets it soon after it ends), so its row is the record.
 */
export function GameRow({ game, now }: { game: MyGame; now: number }) {
  const others = othersText(game)
  const result = resultText(game)
  return (
    <li
      data-game={game.game}
      data-status={game.status}
      className="grid gap-3 rounded-xl bg-card p-3 ring-1 ring-foreground/10 sm:grid-cols-[1fr_auto] sm:items-center sm:p-4"
    >
      <div className="grid min-w-0 gap-1.5">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
          <Badge variant={BADGE[game.status]}>{STATUS_LABEL[game.status]}</Badge>
          <time dateTime={game.created_at} title={game.created_at} className="text-xs text-muted-foreground">
            {timeSince(game.created_at, now)}
          </time>
        </div>
        <p className="text-sm break-words">
          <span className="text-muted-foreground">With </span>
          <span className="font-medium">{others}</span>
        </p>
        {result === null ? null : <p className="text-sm font-medium">{result}</p>}
        <p className="text-xs text-muted-foreground">
          You play {game.name} in seat {game.seat + 1}.
        </p>
      </div>
      {game.status === 'finished' ? null : (
        <div>
          <Link
            to="/game/$gameId"
            params={{ gameId: game.game }}
            aria-label={`Rejoin the game with ${others}`}
            className={buttonVariants({ size: 'lg' })}
          >
            Rejoin
          </Link>
        </div>
      )}
    </li>
  )
}
