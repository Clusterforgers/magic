import { Button } from '@/components/ui/button'
import { usePageTitle } from '@/hooks/usePageTitle'
import { api as sharedApi } from '@/net/api'
import type { Api } from '@/net/api'
import { CreateForm } from './CreateForm'
import { useDecks } from './useDecks'

/** `/`: loads the decks, then shows the form for a new game. */
export function CreateGamePage({ api = sharedApi }: { api?: Api }) {
  usePageTitle('Create a game')
  const { state, reload } = useDecks(api)
  return (
    <div className="mx-auto w-full max-w-3xl px-4 py-8 sm:px-6">
      {state.status === 'loading' ? (
        <p role="status" className="text-sm text-muted-foreground">
          Loading the decks…
        </p>
      ) : null}
      {state.status === 'failed' ? (
        <div role="alert" className="grid justify-items-start gap-3">
          <h1 className="text-2xl font-semibold tracking-tight">Cannot load the decks</h1>
          <p className="text-sm text-muted-foreground">{state.message}</p>
          <Button type="button" onClick={reload}>
            Try again
          </Button>
        </div>
      ) : null}
      {state.status === 'ready' && state.decks.length === 0 ? (
        <div role="alert" className="grid gap-2">
          <h1 className="text-2xl font-semibold tracking-tight">No decks to play with</h1>
          <p className="text-sm text-muted-foreground">
            The server has no decks yet, so a game cannot be created. Ask whoever runs it to add some.
          </p>
        </div>
      ) : null}
      {state.status === 'ready' && state.decks.length > 0 ? (
        <CreateForm api={api} decks={state.decks} />
      ) : null}
    </div>
  )
}
