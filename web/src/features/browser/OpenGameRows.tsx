import { Link } from '@tanstack/react-router'
import { CrownIcon, SwordsIcon } from 'lucide-react'
import { useState } from 'react'
import { Pill } from '@/components/Pill'
import { Button, buttonVariants } from '@/components/ui/button'
import { cardImageUrl } from '@/features/cards/images'
import { SEAT_EDGE, SeatDot, seatHueStyle } from '@/features/table/PodParts'
import { cn } from '@/lib/utils'
import type { LobbyGame } from '@/protocol/generated'
import {
  ageNow,
  ageText,
  commanderNames,
  commandersBySeat,
  formatName,
  isFull,
  joinLabel,
  seatsText,
  watchLabel,
} from './format'
import type { SeatCommanders } from './format'
import type { OnJoin } from './useJoinGame'

/** What the list needs to draw the games. */
export interface RowsProps {
  rows: readonly LobbyGame[]
  /** When the rows arrived (ms): a row is that much older than the server said by `now`. */
  fetchedAt: number
  now: number
  /** The rows are an old listing, shown dimmed beside an error. */
  stale: boolean
  /** Starts joining a game. */
  onJoin: OnJoin
  /** The id of the game whose join is on its way, or null; that card's Join spins. */
  joining: string | null
  /** Some join is on its way (a card's or the code's): every Join is disabled while one is. */
  busy: boolean
}

const fullId = (game: LobbyGame): string => `full-${game.game}`
const hostId = (game: LobbyGame): string => `host-${game.game}`

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
      {game.running === true ? (
        <Pill className="h-6">Running</Pill>
      ) : isFull(game) ? (
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

/**
 * The one primary control of a card: Watch for a running game (its page asks a visitor who is not signed in to sign
 * in), else Join; a full game keeps Join in place, disabled and explained, and the card being joined spins.
 */
function JoinButton({
  game,
  joining,
  busy,
  onJoin,
}: Pick<RowsProps, 'joining' | 'busy' | 'onJoin'> & { game: LobbyGame }) {
  if (game.running === true) {
    return (
      <Link
        to="/watch/$gameId"
        params={{ gameId: game.game }}
        data-watch={game.game}
        aria-label={watchLabel(game)}
        className={buttonVariants({ variant: 'outline' })}
      >
        Watch
      </Link>
    )
  }
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

/** One seat's band of the banner: its commanders' art crops edged in its hue, or the hue's ground when they fail. */
function SeatArt({ seat }: { seat: SeatCommanders }) {
  const [missed, setMissed] = useState(false)
  const pictured = missed
    ? []
    : seat.commanders.flatMap((c) => (c.identity === undefined ? [] : [c.identity]))
  return (
    <span
      data-slot="seat-art"
      data-seat={seat.seat}
      data-pictured={pictured.length > 0 || undefined}
      style={seatHueStyle(seat.seat)}
      className={cn('flex min-w-0 flex-1 overflow-hidden seat-ground', SEAT_EDGE)}
    >
      {pictured.length === 0 ? (
        <span className="grid size-full place-items-center">
          <CrownIcon className="size-6 seat-mark opacity-70" />
        </span>
      ) : (
        pictured.map((identity) => (
          <img
            key={identity.oracle_id}
            src={cardImageUrl({ kind: 'card', identity }, 0, 'art_crop')}
            alt=""
            loading="lazy"
            decoding="async"
            onError={() => setMissed(true)}
            className="size-full min-w-0 flex-1 object-cover object-[50%_30%]"
          />
        ))
      )}
    </span>
  )
}

/**
 * The top of a card: each seat's commanders side by side, edged in the seat's colour; a game without commanders shows
 * the felt with its format's mark and a band of seat colours, a held seat's lit. Decoration: the card writes it all.
 */
function GameArt({ game, seats, dim }: { game: LobbyGame; seats: SeatCommanders[]; dim: boolean }) {
  const Icon = game.format === 'commander' ? CrownIcon : SwordsIcon
  return (
    <div
      aria-hidden="true"
      data-slot="game-art"
      className={cn(
        'relative flex h-24 shrink-0 overflow-hidden bg-linear-to-b from-felt to-felt-rim sm:h-28',
        dim && 'opacity-60 grayscale-[0.4]',
      )}
    >
      {seats.length > 0 ? (
        seats.map((seat) => <SeatArt key={seat.seat} seat={seat} />)
      ) : (
        <>
          <span className="absolute inset-x-0 top-0 flex h-0.5">
            {Array.from({ length: game.total }, (_, seat) => (
              <span
                key={seat}
                style={seatHueStyle(seat)}
                className={cn('flex-1 seat-fill', seat >= game.filled && 'opacity-25')}
              />
            ))}
          </span>
          <span className="grid size-full place-items-center">
            <Icon className="size-8 text-brass drop-shadow-sm" />
          </span>
        </>
      )}
    </div>
  )
}

/** Who leads each seat, by the seat's dot: the words for the banner's pictures. */
function Commanders({ seats }: { seats: SeatCommanders[] }) {
  return (
    <ul aria-label="Commanders" className="grid gap-1 text-sm text-muted-foreground">
      {seats.map((seat) => (
        <li key={seat.seat} className="flex min-w-0 items-center gap-2">
          <SeatDot seat={seat.seat} />
          <span className="truncate">{commanderNames(seat)}</span>
        </li>
      ))}
    </ul>
  )
}

/** One open game as a card: its commanders' art, the host, the facts, the seats and the Join. */
function GameCard({
  game,
  fetchedAt,
  now,
  stale,
  onJoin,
  joining,
  busy,
}: Omit<RowsProps, 'rows'> & { game: LobbyGame }) {
  const dim = stale || isFull(game)
  const seats = commandersBySeat(game)
  return (
    <li data-game={game.game} data-full={isFull(game) || undefined} className="min-w-0">
      <article
        aria-labelledby={hostId(game)}
        className="plaque flex h-full flex-col overflow-hidden rounded-xl border"
      >
        <GameArt game={game} seats={seats} dim={dim} />
        <div className="flex flex-1 flex-col gap-2 p-3">
          <div className="grid min-w-0 gap-1">
            <h3
              id={hostId(game)}
              className={cn(
                'font-display text-base font-semibold break-words',
                dim ? 'text-muted-foreground' : 'text-foreground',
              )}
            >
              {game.host}
            </h3>
            <p className="text-sm break-words text-muted-foreground">
              {formatName(game.format)} · Your own deck · <Age game={game} fetchedAt={fetchedAt} now={now} />
            </p>
          </div>
          {seats.length > 0 ? <Commanders seats={seats} /> : null}
          <div className="mt-auto flex flex-wrap items-center justify-between gap-x-3 gap-y-2 pt-1">
            <p className={cn('text-sm', dim ? 'text-muted-foreground' : 'text-foreground')}>
              <Seats game={game} dim={dim} />
            </p>
            <JoinButton game={game} onJoin={onJoin} joining={joining} busy={busy} />
          </div>
        </div>
      </article>
    </li>
  )
}

/** The open games as a grid of cards, newest first: one column on a phone, up to four on a wide window. */
export function OpenGameRows({ rows, ...props }: RowsProps) {
  return (
    <ul
      aria-label="Open games, newest first"
      className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,17rem),1fr))] gap-3 p-3"
    >
      {rows.map((game) => (
        <GameCard key={game.game} game={game} {...props} />
      ))}
    </ul>
  )
}
