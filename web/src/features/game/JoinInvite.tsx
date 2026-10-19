import { useState } from 'react'
import { CopyButton } from '@/components/CopyButton'
import { loadInvites } from '@/lib/invites'
import { codeText, joinLinkUrl } from '@/lib/joinCode'

/**
 * The host's way to bring friends in: the invite link and the join code, each with a copy button. The server shows
 * the code once, so it comes from this tab's `sessionStorage`; a tab that does not have it says so instead.
 */
export function JoinInvite({ game }: { game: string }) {
  // Read once: sessionStorage may be blocked, and the answer cannot change while the lobby is open.
  const [stored] = useState(() => loadInvites(game))
  // A game made with a link for every seat has no open seat to take by code; its links are listed beside this.
  if (stored !== null && stored.code === undefined) return null
  const code = stored?.code ?? null
  const shown = code === null ? null : codeText(code)
  const link = code === null ? null : joinLinkUrl(window.location.origin, code)
  return (
    <section aria-labelledby="invite-title" data-slot="join-invite" className="surface grid gap-3 p-4">
      <div>
        <h2 id="invite-title" className="text-lg font-semibold">
          Invite players
        </h2>
        <p className="text-sm text-muted-foreground">
          Send the link, or tell them the code to type on the Play page.
        </p>
      </div>
      {shown === null || link === null ? (
        <p data-slot="invite-missing" className="text-sm text-muted-foreground">
          The code is shown only in the browser tab that made the game, and this tab does not have it.
        </p>
      ) : (
        <dl className="grid gap-3">
          <div className="grid gap-1">
            <dt className="text-xs text-muted-foreground">Invite link</dt>
            <dd className="flex items-center gap-2">
              <a
                data-slot="invite-link"
                href={link}
                className="min-w-0 flex-1 rounded-md bg-muted px-2 py-2 font-mono text-xs break-all underline-offset-2 hover:underline"
              >
                {link}
              </a>
              <CopyButton text={link} label="Copy invite link">
                Copy
              </CopyButton>
            </dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-xs text-muted-foreground">Code</dt>
            <dd className="flex items-center justify-between gap-2">
              <span data-slot="invite-code" className="font-mono text-xl tracking-widest">
                {shown}
              </span>
              <CopyButton text={shown} label="Copy code">
                Copy
              </CopyButton>
            </dd>
          </div>
        </dl>
      )}
    </section>
  )
}
