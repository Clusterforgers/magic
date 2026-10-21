import { Link } from '@tanstack/react-router'
import { Pill } from '@/components/Pill'
import type { PillTone } from '@/components/Pill'
import { Button, buttonVariants } from '@/components/ui/button'
import { useMediaQuery } from '@/hooks/useMediaQuery'
import { cn } from '@/lib/utils'
import { timeSince } from '@/lib/timeSince'
import type { MyGame } from '@/protocol/generated'
import { continueLabel, othersText, removeLabel, resultText, STATUS_LABEL } from './format'

// The Play page's breakpoint: a table from here up, stacked rows under it.
const TABLE_FROM = '(min-width: 64rem)'

const TONE: Record<MyGame['status'], PillTone> = {
  lobby: 'neutral',
  running: 'success',
  finished: 'neutral',
  failed: 'warning',
}

/** What both layouts need to draw the rows. */
export interface GameRowsProps {
  /** The games, newest first. */
  games: readonly MyGame[]
  /** The clock the ages are read from, in milliseconds. */
  now: number
  /** The rows are an old listing, shown dimmed beside an error. */
  stale: boolean
  /** The id of the failed game being removed right now, or null. */
  removing: string | null
  /** Takes a failed game off the list. */
  onRemove: (game: MyGame) => void
}

/** The state of a game as a badge: a word as well as a tone. */
function State({ game }: { game: MyGame }) {
  return <Pill tone={TONE[game.status]}>{STATUS_LABEL[game.status]}</Pill>
}

/** "5 minutes ago" in a `<time>` that carries the game's creation stamp. */
function Age({ game, now }: { game: MyGame; now: number }) {
  return (
    <time dateTime={game.created_at} title={game.created_at}>
      {timeSince(game.created_at, now)}
    </time>
  )
}

/**
 * The one action of a row: Continue opens the game page with no seat link, which connects through the session.
 * A finished game has none, since the server forgets it soon after it ends: its row is the record. A failed game cannot
 * be played, so it has no link, only Remove, which takes it off the list.
 */
function Action({
  game,
  removing,
  onRemove,
}: { game: MyGame } & Pick<GameRowsProps, 'removing' | 'onRemove'>) {
  if (game.status === 'finished') return null
  if (game.status === 'failed') {
    return (
      <Button
        type="button"
        variant="outline"
        loading={removing === game.game}
        aria-label={removeLabel(game)}
        onClick={() => onRemove(game)}
      >
        Remove
      </Button>
    )
  }
  return (
    <Link
      to="/game/$gameId"
      params={{ gameId: game.game }}
      aria-label={continueLabel(game)}
      className={buttonVariants()}
    >
      Continue
    </Link>
  )
}

const HEAD =
  'sticky top-0 z-10 border-b bg-surface px-4 py-2 text-left text-xs font-medium text-muted-foreground'
const CELL = 'border-b px-4 py-3 align-middle text-sm'

/** From the table breakpoint up: a real table, its header kept in view while the rows scroll. */
function GamesTable({ games, now, stale, removing, onRemove }: GameRowsProps) {
  return (
    <table className="w-full border-separate border-spacing-0 text-left">
      <caption className="sr-only">Your games, newest first</caption>
      <thead>
        <tr>
          <th scope="col" className={HEAD}>
            Players
          </th>
          <th scope="col" className={HEAD}>
            State
          </th>
          <th scope="col" className={HEAD}>
            Result
          </th>
          <th scope="col" className={HEAD}>
            Your seat
          </th>
          <th scope="col" className={HEAD}>
            Started
          </th>
          <th scope="col" className={cn(HEAD, 'text-right')}>
            Action
          </th>
        </tr>
      </thead>
      <tbody className="[&>tr:last-child>*]:border-b-0">
        {games.map((game) => {
          const result = resultText(game)
          return (
            <tr key={game.game} data-game={game.game} data-status={game.status}>
              <th
                scope="row"
                className={cn(CELL, 'font-medium [overflow-wrap:anywhere]', stale && 'text-muted-foreground')}
              >
                {othersText(game)}
              </th>
              <td className={CELL}>
                <State game={game} />
              </td>
              <td className={cn(CELL, stale && 'text-muted-foreground')}>
                {result === null ? (
                  <span className="text-muted-foreground">
                    <span aria-hidden="true">–</span>
                    <span className="sr-only">Not over</span>
                  </span>
                ) : (
                  // A reason is a sentence: it wraps in a measure of its own instead of widening the column.
                  <span className="block max-w-md [overflow-wrap:anywhere]">{result}</span>
                )}
              </td>
              <td className={cn(CELL, '[overflow-wrap:anywhere] text-muted-foreground')}>
                {game.name}, seat {game.seat + 1}
              </td>
              <td className={cn(CELL, 'whitespace-nowrap text-muted-foreground')}>
                <Age game={game} now={now} />
              </td>
              <td className={cn(CELL, 'text-right')}>
                <Action game={game} removing={removing} onRemove={onRemove} />
              </td>
            </tr>
          )
        })}
      </tbody>
    </table>
  )
}

/** Under the table breakpoint: one stacked row a game, Continue at its right. */
function GamesList({ games, now, stale, removing, onRemove }: GameRowsProps) {
  return (
    <ul aria-label="Your games, newest first" className="grid">
      {games.map((game) => {
        const result = resultText(game)
        return (
          <li
            key={game.game}
            data-game={game.game}
            data-status={game.status}
            className="grid grid-cols-[1fr_auto] items-center gap-x-4 gap-y-1 border-b px-4 py-3 last:border-b-0"
          >
            <div className="grid min-w-0 gap-1">
              <p
                className={cn(
                  'text-base font-medium [overflow-wrap:anywhere]',
                  stale ? 'text-muted-foreground' : 'text-foreground',
                )}
              >
                <span className="font-normal text-muted-foreground">With </span>
                {othersText(game)}
              </p>
              <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-muted-foreground">
                <State game={game} />
                <Age game={game} now={now} />
              </p>
              {result === null ? null : <p className="text-sm font-medium">{result}</p>}
              <p className="text-sm [overflow-wrap:anywhere] text-muted-foreground">
                You play {game.name} in seat {game.seat + 1}.
              </p>
            </div>
            <Action game={game} removing={removing} onRemove={onRemove} />
          </li>
        )
      })}
    </ul>
  )
}

/** The games as a table from the Play page's breakpoint up and as stacked rows under it; only one is in the page. */
export function GameRows(props: GameRowsProps) {
  const wide = useMediaQuery(TABLE_FROM, true)
  return wide ? <GamesTable {...props} /> : <GamesList {...props} />
}
