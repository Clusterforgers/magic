import { Link } from '@tanstack/react-router'
import { useEffect, useRef } from 'react'
import { CopyButton } from '@/components/CopyButton'
import { SeatLinkRow } from '@/components/SeatLinkRow'
import { Button } from '@/components/ui/button'
import { useAuth } from '@/features/auth/context'
import { seatLinkPath, seatLinkUrl } from '@/lib/seatLink'
import type { CreatedGame, CreateGameRequest, DeckInfo } from '@/net/api'

interface GameCreatedProps {
  created: CreatedGame
  /** What was asked for; the server's answer names seats by number only. */
  request: CreateGameRequest
  decks: readonly DeckInfo[]
  /** The address links are made on, such as `https://play.example`. */
  origin: string
  onOpenSeat(seat: number, token: string): void
  onAnother(): void
}

/** The result of creating a game: one link per seat, each with a copy button, and a way into the host's own seat. */
export function GameCreated({ created, request, decks, origin, onOpenSeat, onAnother }: GameCreatedProps) {
  const account = useAuth((state) => state.user?.username ?? null)
  const heading = useRef<HTMLHeadingElement>(null)
  useEffect(() => heading.current?.focus(), [])

  const seats = created.seats.map((seat) => {
    const asked = request.seats[seat.seat]
    return {
      ...seat,
      position: seat.seat + 1,
      name: asked?.name ?? `Seat ${seat.seat + 1}`,
      deck: decks.find((deck) => deck.id === asked?.deck)?.name ?? null,
      path: seatLinkPath(created.game, seat.seat, seat.token),
      url: seatLinkUrl(origin, created.game, seat.seat, seat.token),
    }
  })
  const mine = seats[0]
  const everyLink = seats.map((seat) => `${seat.name}: ${seat.url}`).join('\n')

  return (
    <section aria-labelledby="created-title" className="grid gap-5">
      <div>
        <h1
          id="created-title"
          ref={heading}
          tabIndex={-1}
          className="text-2xl font-semibold tracking-tight outline-none"
        >
          Game created
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Each link is the key to one seat: anyone who has it can play that seat. Send every player only their
          own link.
        </p>
        {account === null || mine === undefined ? null : (
          <p data-slot="saved-note" className="mt-2 text-sm break-words">
            Seat 1 ({mine.name}) is yours and saved to your account,{' '}
            <strong className="font-medium">{account}</strong>. Find this game under{' '}
            <Link to="/games" className="font-medium underline underline-offset-3">
              My games
            </Link>{' '}
            and rejoin it from there without a link. The other links are for your friends.
          </p>
        )}
      </div>
      <div className="flex flex-wrap items-center gap-2">
        {mine === undefined ? null : (
          <Button type="button" size="lg" onClick={() => onOpenSeat(mine.seat, mine.token)}>
            Open my seat ({mine.name})
          </Button>
        )}
        <CopyButton text={everyLink} label="Copy all links">
          Copy all links
        </CopyButton>
      </div>
      <ol aria-label="Seat links" className="grid gap-3">
        {seats.map((seat) => (
          <SeatLinkRow
            key={seat.seat}
            title={`Seat ${seat.position}: ${seat.name}`}
            note={seat.deck ?? undefined}
            path={seat.path}
            url={seat.url}
            copyLabel={`Copy link for seat ${seat.position} (${seat.name})`}
          />
        ))}
      </ol>
      <p className="text-xs text-muted-foreground">
        The server shows these keys only once. This browser tab keeps a copy so you can hand them out again
        from the lobby; it forgets them when you close the tab.
      </p>
      <div>
        <Button type="button" variant="outline" onClick={onAnother}>
          Create another game
        </Button>
      </div>
    </section>
  )
}
