import { KeyRoundIcon, Trash2Icon } from 'lucide-react'
import { useRef, useState } from 'react'
import { Pill } from '@/components/Pill'
import { Button } from '@/components/ui/button'
import { timeSince } from '@/lib/timeSince'
import type { AccountRow } from '@/protocol/generated'
import { CredentialsPanel } from './CredentialsPanel'
import { NewPasswordForm } from './NewPasswordForm'
import { RemoveAccountDialog } from './RemoveAccountDialog'

interface AccountItemProps {
  account: AccountRow
  /** The time the list opened, for "Made 3 days ago". */
  now: number
  /** The account was removed: the list says so and takes the focus. */
  onRemoved: (username: string) => void
}

type Panel = 'none' | 'password' | 'remove'

/**
 * One account of the list. A friend's row offers "New password" (a form under the row, then the new credentials once)
 * and "Remove" (a question first); the admin's own row offers neither, since its password changes in Account settings
 * and nothing makes another admin.
 */
export function AccountItem({ account, now, onRemoved }: AccountItemProps) {
  const [panel, setPanel] = useState<Panel>('none')
  // The password just set, shown once with the address and the username until the admin is done.
  const [given, setGiven] = useState<string | null>(null)
  const passwordButton = useRef<HTMLButtonElement>(null)

  function closePassword(): void {
    setPanel('none')
    setGiven(null)
    // Back where the admin was, so the keyboard does not start over at the top of the page.
    window.requestAnimationFrame(() => passwordButton.current?.focus())
  }

  return (
    <li className="grid gap-3 py-3 first:pt-0 last:pb-0">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <span className="min-w-0 flex-1 font-medium break-all">{account.username}</span>
        {account.is_admin ? <Pill tone="success">Admin</Pill> : null}
        <time
          dateTime={account.created_at}
          title={account.created_at}
          className="text-sm text-muted-foreground"
        >
          Made {timeSince(account.created_at, now)}
        </time>
        {account.is_admin ? null : (
          <div className="flex gap-2 max-sm:basis-full">
            <Button
              ref={passwordButton}
              type="button"
              variant="outline"
              size="sm"
              aria-label={`New password for ${account.username}`}
              aria-expanded={panel === 'password'}
              onClick={() => (panel === 'password' ? closePassword() : setPanel('password'))}
            >
              <KeyRoundIcon aria-hidden="true" />
              New password
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              aria-label={`Remove ${account.username}`}
              onClick={() => setPanel('remove')}
            >
              <Trash2Icon aria-hidden="true" />
              Remove
            </Button>
          </div>
        )}
      </div>

      {panel !== 'password' ? null : given === null ? (
        <NewPasswordForm account={account} onSet={setGiven} onCancel={closePassword} />
      ) : (
        <CredentialsPanel
          label={`The new password for ${account.username}`}
          username={account.username}
          password={given}
          note="Their sessions are signed out. This is the only time the password is shown. Tell your friend to change it under Account settings."
          onDone={closePassword}
        />
      )}

      <RemoveAccountDialog
        account={account}
        open={panel === 'remove'}
        onOpenChange={(open) => setPanel(open ? 'remove' : 'none')}
        onRemoved={() => onRemoved(account.username)}
      />
    </li>
  )
}
