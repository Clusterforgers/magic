import { Link } from '@tanstack/react-router'
import { Pill } from '@/components/Pill'
import type { PillTone } from '@/components/Pill'
import { buttonVariants } from '@/components/ui/button'
import type { MyGame } from '@/protocol/generated'
import { othersText, resultText, STATUS_LABEL, timeSince } from './format'

const TONE: Record<MyGame['status'], PillTone> = { lobby: 'neutral', running: 'success', finished: 'neutral' }

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
      className="surface grid gap-3 p-4 sm:grid-cols-[1fr_auto] sm:items-center"
    >
      <div className="grid min-w-0 gap-2">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
          <Pill tone={TONE[game.status]}>{STATUS_LABEL[game.status]}</Pill>
          <time dateTime={game.created_at} title={game.created_at} className="text-xs text-muted-foreground">
            {timeSince(game.created_at, now)}
          </time>
        </div>
        <p className="text-sm break-words">
          <span className="text-muted-foreground">With </span>
          <span className="font-medium">{others}</span>
        </p>
        {result === null ? null : <p className="text-sm font-medium">{result}</p>}
        <p className="text-xs break-words text-muted-foreground">
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
