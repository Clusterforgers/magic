import { useState } from 'react'
import { SeatLinkRow } from '@/components/SeatLinkRow'
import { loadInvites } from '@/lib/invites'
import { seatLinkPath, seatLinkUrl } from '@/lib/seatLink'

interface InvitesProps {
  game: string
  /** The seat this page plays; its link is not an invitation to anyone else. */
  mine: number | null
}

/** The links of the other seats, when this tab created the game; a token is printed only in these share links. */
export function Invites({ game, mine }: InvitesProps) {
  // Read once: sessionStorage may be blocked, and the answer cannot change while the lobby is open.
  const [invites] = useState(() => loadInvites(game))
  const others = invites?.seats.filter((invite) => invite.seat !== mine) ?? []
  if (others.length === 0) return null
  return (
    <section aria-labelledby="invites-title" className="grid gap-3">
      <div>
        <h2 id="invites-title" className="text-lg font-semibold">
          Invite the other players
        </h2>
        <p className="text-sm text-muted-foreground">
          Send each player the link for their own seat. Anyone who has a link can play that seat.
        </p>
      </div>
      <ul aria-label="Seat links" className="grid gap-2">
        {others.map((invite) => (
          <SeatLinkRow
            key={invite.seat}
            title={`Seat ${invite.seat + 1}: ${invite.name}`}
            path={seatLinkPath(game, invite.seat, invite.token)}
            url={seatLinkUrl(window.location.origin, game, invite.seat, invite.token)}
            copyLabel={`Copy link for seat ${invite.seat + 1} (${invite.name})`}
          />
        ))}
      </ul>
    </section>
  )
}
