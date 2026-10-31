import { useEffect, useRef } from 'react'
import { CopyButton } from '@/components/CopyButton'
import { Button } from '@/components/ui/button'

interface CredentialsPanelProps {
  /** The accessible name of the panel: what it holds. */
  label: string
  username: string
  password: string
  /** The sentence under the credentials. */
  note: string
  /** The admin is done copying; the panel goes and the password with it. */
  onDone: () => void
}

/** The message the admin pastes to a friend: where to sign in, and with what. */
function credentialsMessage(origin: string, username: string, password: string): string {
  return `Address: ${origin}\nUsername: ${username}\nPassword: ${password}`
}

/**
 * The credentials of an account the admin has just made or given a new password, shown once. They sit below the form
 * that produced them, which on a phone is below the fold, so the panel scrolls itself into view and takes focus on its
 * first button ("Copy both"). The password lives in this panel's props only, until "Done".
 */
export function CredentialsPanel({ label, username, password, note, onDone }: CredentialsPanelProps) {
  const panel = useRef<HTMLElement>(null)
  const origin = window.location.origin

  useEffect(() => {
    const element = panel.current
    if (element === null) return
    // Not every environment can scroll (a test's DOM); focusing the first button scrolls it too.
    element.scrollIntoView?.({ block: 'nearest' })
    element.querySelector('button')?.focus({ preventScroll: true })
  }, [username, password])

  return (
    <section
      ref={panel}
      aria-label={label}
      className="grid gap-3 rounded-lg border border-success/40 bg-success/12 p-4 text-sm"
    >
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1">
        <dt className="text-muted-foreground">Address</dt>
        <dd className="font-mono break-all select-all">{origin}</dd>
        <dt className="text-muted-foreground">Username</dt>
        <dd className="font-mono break-all select-all">{username}</dd>
        <dt className="text-muted-foreground">Password</dt>
        <dd className="font-mono break-all select-all">{password}</dd>
      </dl>
      <p className="text-muted-foreground">{note}</p>
      <div className="flex flex-wrap gap-2">
        <CopyButton
          text={credentialsMessage(origin, username, password)}
          label="Copy both, with the site address"
        >
          Copy both
        </CopyButton>
        <Button type="button" variant="ghost" size="sm" onClick={onDone}>
          Done
        </Button>
      </div>
    </section>
  )
}
