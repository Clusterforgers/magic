import { CircleAlertIcon } from 'lucide-react'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'
import { api as sharedApi } from '@/net/api'
import type { Api } from '@/net/api'
import { CreateForm } from './CreateForm'
import { useDecks } from './useDecks'

// The shape of the form while the decks are on their way: a heading, the format choice and two seats.
function FormSkeleton() {
  return (
    <div className="grid gap-6">
      <div className="grid gap-2">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-5 w-full max-w-md" />
      </div>
      <Skeleton className="h-9 w-56" />
      <div className="grid gap-3">
        <Skeleton className="h-36 rounded-xl" />
        <Skeleton className="h-36 rounded-xl" />
      </div>
    </div>
  )
}

/** `/new`: loads the decks, then shows the form for a new game. */
export function CreateGamePage({ api = sharedApi }: { api?: Api }) {
  usePageTitle('Create a game')
  const { state, reload } = useDecks(api)
  // The form names the creator's own seat after the account, so it waits until the server has said who that is.
  const unknownAccount = useAuth((auth) => auth.status === 'loading')
  return (
    <PageShell>
      {state.status === 'loading' || (state.status === 'ready' && unknownAccount) ? (
        <Loading label="Loading the decks…">
          <FormSkeleton />
        </Loading>
      ) : null}
      {state.status === 'failed' ? (
        <div className="grid gap-4">
          <h1 className="sr-only">Create a game</h1>
          <Alert variant="destructive">
            <CircleAlertIcon aria-hidden="true" />
            <AlertTitle>Cannot load the decks</AlertTitle>
            <AlertDescription>{state.message}</AlertDescription>
            <div className="mt-2">
              <Button type="button" onClick={reload}>
                Retry
              </Button>
            </div>
          </Alert>
        </div>
      ) : null}
      {state.status === 'ready' && state.decks.length === 0 ? (
        <div className="grid gap-4">
          <h1 className="sr-only">Create a game</h1>
          <Alert variant="warning">
            <CircleAlertIcon aria-hidden="true" />
            <AlertTitle>No decks to play with</AlertTitle>
            <AlertDescription>
              The server has no decks yet, so a game cannot be created. Ask whoever runs it to add some.
            </AlertDescription>
            <div className="mt-2">
              <Button type="button" variant="outline" onClick={reload}>
                Retry
              </Button>
            </div>
          </Alert>
        </div>
      ) : null}
      {state.status === 'ready' && state.decks.length > 0 && !unknownAccount ? (
        <CreateForm api={api} decks={state.decks} />
      ) : null}
    </PageShell>
  )
}
