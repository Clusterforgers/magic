import { CircleAlertIcon } from 'lucide-react'
import { useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Pill } from '@/components/Pill'
import { Alert, AlertAction, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth, useAuthStore } from '@/features/auth/context'
import { timeSince } from '@/lib/timeSince'

/** Every account, oldest first, with when it was made; the admin's own row carries an "Admin" badge. */
export function AccountList() {
  const { status, users, error } = useAuth(useShallow((state) => state.accounts))
  const store = useAuthStore()
  // Read once, when the page opens: the ages need no ticking, and an account made since reads as "just now".
  const [now] = useState(() => Date.now())

  return (
    <section aria-labelledby="account-list-title" className="surface grid gap-4 p-4 sm:p-6">
      <div className="grid gap-1">
        <h2 id="account-list-title" className="text-lg font-semibold tracking-tight">
          Accounts
        </h2>
        <p className="text-sm text-muted-foreground">
          {status === 'ready' || users.length > 0
            ? `${users.length} ${users.length === 1 ? 'account' : 'accounts'}, oldest first.`
            : 'Everyone who can sign in here.'}
        </p>
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
            <li
              key={account.id}
              className="flex flex-wrap items-center gap-x-3 gap-y-1 py-3 first:pt-0 last:pb-0"
            >
              <span className="min-w-0 flex-1 font-medium break-all">{account.username}</span>
              {account.is_admin ? <Pill tone="success">Admin</Pill> : null}
              <time
                dateTime={account.created_at}
                title={account.created_at}
                className="text-sm text-muted-foreground"
              >
                Made {timeSince(account.created_at, now)}
              </time>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
