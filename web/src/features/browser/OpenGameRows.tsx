import { Pill } from '@/components/Pill'
import { Button } from '@/components/ui/button'
import { useMediaQuery } from '@/hooks/useMediaQuery'
import { cn } from '@/lib/utils'
import type { LobbyGame } from '@/protocol/generated'
import { ageNow, ageText, formatName, isFull, joinLabel, seatsText } from './format'
import type { OnJoin } from './useJoinGame'

/** The window from which the open games are a table with columns; under it each game is a stacked row. */
export const TABLE_FROM = '(min-width: 64rem)'

/** What both layouts need to draw the rows. */
export interface RowsProps {
  rows: readonly LobbyGame[]
  /** When the rows arrived (ms): a row is that much older than the server said by `now`. */
  fetchedAt: number
  now: number
  /** The rows are an old listing, shown dimmed beside an error. */
  stale: boolean
  /** Starts joining a game. */
  onJoin: OnJoin
  /** The id of the game whose join is on its way, or null; that row's Join spins. */
  joining: string | null
  /** Some join is on its way (a row's or the code's): every Join is disabled while one is. */
  busy: boolean
}

const fullId = (game: LobbyGame): string => `full-${game.game}`

/** One dot a seat, filled where somebody sits; the words beside it say the same, so the dots are decoration. */
function SeatDots({ game, dim }: { game: LobbyGame; dim: boolean }) {
  return (
    <span aria-hidden="true" data-slot="seat-dots" className="inline-flex flex-wrap items-center gap-1">
      {Array.from({ length: game.total }, (_, seat) => (
        <span
          key={seat}
          className={cn(
            'size-2 rounded-full border',
            seat < game.filled
              ? dim
                ? 'border-muted-foreground bg-muted-foreground'
                : 'border-foreground bg-foreground'
              : 'border-border-strong',
          )}
        />
      ))}
    </span>
  )
}

/** "2 of 4" with its dots and, for a full game, the word Full. */
function Seats({ game, dim }: { game: LobbyGame; dim: boolean }) {
  return (
    <span className="inline-flex flex-wrap items-center gap-x-2 gap-y-1">
      <SeatDots game={game} dim={dim} />
      <span className="text-sm tabular-nums">{seatsText(game)}</span>
      {isFull(game) ? (
        <Pill id={fullId(game)} className="h-6">
          Full
        </Pill>
      ) : null}
    </span>
  )
}

/** "3 min ago" in a `<time>` that carries the game's creation stamp. */
function Age({ game, fetchedAt, now }: { game: LobbyGame; fetchedAt: number; now: number }) {
  return <time dateTime={game.created_at}>{ageText(ageNow(game, fetchedAt, now))}</time>
}

/** The one primary button of a row; a full game keeps it in place, disabled and explained, and the row being joined spins. */
function JoinButton({
  game,
  joining,
  busy,
  onJoin,
}: Pick<RowsProps, 'joining' | 'busy' | 'onJoin'> & { game: LobbyGame }) {
  const full = isFull(game)
  return (
    <Button
      type="button"
      data-join={game.game}
      aria-label={joinLabel(game)}
      aria-describedby={full ? fullId(game) : undefined}
      disabled={full || busy}
      loading={joining === game.game}
      // A disabled button that keeps focus does not drop it when a game fills while it is the one in hand.
      focusableWhenDisabled
      onClick={(event) => onJoin(game, event.currentTarget)}
    >
      Join
    </Button>
  )
}

const dimmed = (game: LobbyGame, stale: boolean): boolean => stale || isFull(game)

const HEAD =
  'sticky top-0 z-10 border-b bg-surface px-4 py-2 text-left text-xs font-medium text-muted-foreground'
const CELL = 'border-b px-4 py-3 align-middle text-sm'

/** From the M breakpoint up: a real table, its header kept in view while the rows scroll. */
function GamesTable({ rows, fetchedAt, now, stale, onJoin, joining, busy }: RowsProps) {
  return (
    <table className="w-full border-separate border-spacing-0 text-left">
      <caption className="sr-only">Open games, newest first</caption>
      <thead>
        <tr>
          <th scope="col" className={HEAD}>
            Host
          </th>
          <th scope="col" className={HEAD}>
            Format
          </th>
          <th scope="col" className={HEAD}>
            Seats
          </th>
          <th scope="col" className={HEAD}>
            Decks
          </th>
          <th scope="col" className={HEAD}>
            Age
          </th>
          <th scope="col" className={cn(HEAD, 'text-right')}>
            Action
          </th>
        </tr>
      </thead>
      <tbody className="[&>tr:last-child>*]:border-b-0">
        {rows.map((game) => {
          const dim = dimmed(game, stale)
          return (
            <tr key={game.game} data-game={game.game} data-full={isFull(game) || undefined}>
              <th
                scope="row"
                className={cn(
                  CELL,
                  'font-medium break-words',
                  dim ? 'text-muted-foreground' : 'text-foreground',
                )}
              >
                {game.host}
              </th>
              <td className={cn(CELL, dim ? 'text-muted-foreground' : 'text-foreground')}>
                {formatName(game.format)}
              </td>
              <td className={cn(CELL, dim ? 'text-muted-foreground' : 'text-foreground')}>
                <Seats game={game} dim={dim} />
              </td>
              <td className={cn(CELL, 'text-muted-foreground')}>Your own deck</td>
              <td className={cn(CELL, 'whitespace-nowrap text-muted-foreground')}>
                <Age game={game} fetchedAt={fetchedAt} now={now} />
              </td>
              <td className={cn(CELL, 'text-right')}>
                <JoinButton game={game} onJoin={onJoin} joining={joining} busy={busy} />
              </td>
            </tr>
          )
        })}
      </tbody>
    </table>
  )
}

/** Under the M breakpoint: one stacked row a game, the Join button at its right. */
function GamesList({ rows, fetchedAt, now, stale, onJoin, joining, busy }: RowsProps) {
  return (
    <ul aria-label="Open games, newest first" className="grid">
      {rows.map((game) => {
        const dim = dimmed(game, stale)
        return (
          <li
            key={game.game}
            data-game={game.game}
            data-full={isFull(game) || undefined}
            className="grid grid-cols-[1fr_auto] items-center gap-x-4 gap-y-1 border-b px-4 py-3 last:border-b-0"
          >
            <div className="grid min-w-0 gap-1">
              <p
                className={cn(
                  'text-base font-medium break-words',
                  dim ? 'text-muted-foreground' : 'text-foreground',
                )}
              >
                {game.host}
              </p>
              <p className="text-sm break-words text-muted-foreground">
                {formatName(game.format)} · Your own deck ·{' '}
                <Age game={game} fetchedAt={fetchedAt} now={now} />
              </p>
              <p className={cn('text-sm', dim ? 'text-muted-foreground' : 'text-foreground')}>
                <Seats game={game} dim={dim} />
              </p>
            </div>
            <JoinButton game={game} onJoin={onJoin} joining={joining} busy={busy} />
          </li>
        )
      })}
    </ul>
  )
}

/** The rows as a table from the M breakpoint up and as stacked rows under it; only the one that applies is in the page. */
export function OpenGameRows(props: RowsProps) {
  const wide = useMediaQuery(TABLE_FROM, true)
  return wide ? <GamesTable {...props} /> : <GamesList {...props} />
}
