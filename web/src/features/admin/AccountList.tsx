import { CircleAlertIcon, CircleCheckIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Alert, AlertAction, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth, useAuthStore } from '@/features/auth/context'
import { AccountItem } from './AccountItem'

/** Every account, oldest first, with when it was made; the admin's own row carries an "Admin" badge, a friend's row the actions on it. */
export function AccountList() {
  const { status, users, error } = useAuth(useShallow((state) => state.accounts))
  const store = useAuthStore()
  // Read once, when the page opens: the ages need no ticking, and an account made since reads as "just now".
  const [now] = useState(() => Date.now())
  // The name of the account just removed, said once; the heading takes the focus the removed row leaves behind.
  const [removed, setRemoved] = useState<string | null>(null)
  const heading = useRef<HTMLHeadingElement>(null)
  useEffect(() => {
    if (removed !== null) heading.current?.focus()
  }, [removed])

  return (
    <section aria-labelledby="account-list-title" className="surface grid gap-4 p-4 sm:p-6">
      <div className="grid gap-1">
        <h2
          ref={heading}
          id="account-list-title"
          tabIndex={-1}
          className="text-lg font-semibold tracking-tight outline-none"
        >
          Accounts
        </h2>
        <p className="text-sm text-muted-foreground">
          {status === 'ready' || users.length > 0
            ? `${users.length} ${users.length === 1 ? 'account' : 'accounts'}, oldest first.`
            : 'Everyone who can sign in here.'}
        </p>
      </div>

      {/* Always in the page, so the words of a removal are announced when they arrive. */}
      <div role="status" className="empty:hidden">
        {removed === null ? null : (
          <p className="flex items-start gap-2 text-sm">
            <CircleCheckIcon aria-hidden="true" className="size-4 shrink-0 translate-y-0.5 text-success" />
            <span>{`${removed} was removed.`}</span>
          </p>
        )}
      </div>

      {error === null ? null : (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertDescription className="text-foreground">
            {`Could not load the accounts. ${error}`}
          </AlertDescription>
          <AlertAction>
            <Button
              type="button"
              variant="ghost"
              size="xs"
              onClick={() => void store.getState().loadAccounts()}
            >
              Try again
            </Button>
          </AlertAction>
        </Alert>
      )}

      {status === 'loading' && users.length === 0 ? (
        <div role="status" aria-label="Loading the accounts" className="grid gap-2">
          <Skeleton className="h-6 w-full" />
          <Skeleton className="h-6 w-2/3" />
        </div>
      ) : (
        <ul className="divide-y divide-border" aria-busy={status === 'loading'}>
          {users.map((account) => (
            <AccountItem key={account.id} account={account} now={now} onRemoved={setRemoved} />
          ))}
        </ul>
      )}
    </section>
  )
}
