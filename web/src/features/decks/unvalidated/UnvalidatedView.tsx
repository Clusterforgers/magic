import { Link } from '@tanstack/react-router'
import { CircleAlertIcon, ShieldCheckIcon, TriangleAlertIcon } from 'lucide-react'
import { useState } from 'react'
import { CopyButton } from '@/components/CopyButton'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import type { CardsApi } from '@/net/cards'
import type { DecksApi } from '@/net/decks'
import type { FixMarksApi } from '@/net/fixMarks'
import { SUPPORT_LABEL } from '../format'
import { copyText, filterCards, groupByDeck, mergeDecks, stateCounts } from './model'
import type { FlaggedCard, StateFilter } from './model'
import { UnvalidatedCard } from './UnvalidatedCard'
import { useCardTexts } from './useCardTexts'
import { useUnvalidated } from './useUnvalidated'
import type { Scope } from './useUnvalidated'

type Arrangement = 'cards' | 'decks'

function ListSkeleton() {
  return (
    <div className="grid gap-3">
      {[0, 1, 2].map((row) => (
        <Skeleton key={row} className="h-28 rounded-xl" />
      ))}
    </div>
  )
}

/** A row of buttons of which one is on; the one that is on says so with `aria-pressed`, not only with its look. */
function Choice<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string
  value: T
  options: ReadonlyArray<{ value: T; text: string }>
  onChange: (next: T) => void
}) {
  return (
    <div role="group" aria-label={label} className="flex flex-wrap items-center gap-1">
      {options.map((option) => (
        <Button
          key={option.value}
          type="button"
          size="sm"
          variant={option.value === value ? 'secondary' : 'outline'}
          aria-pressed={option.value === value}
          onClick={() => onChange(option.value)}
        >
          {option.text}
        </Button>
      ))}
    </div>
  )
}

interface UnvalidatedViewProps {
  api: Pick<DecksApi, 'list' | 'get' | 'check'>
  cards: Pick<CardsApi, 'get'>
  scope: Scope
  fixMarks?: FixMarksApi
}

/**
 * The cards of the saved decks (or of one) that the game does not run whole: each card once with its unread clauses
 * quoted from its text, merged with the decks that hold it (the most used first) or grouped by deck, narrowed to a
 * state, and copyable as plain text.
 */
export function UnvalidatedView({ api, cards, scope, fixMarks }: UnvalidatedViewProps) {
  const { state, reload } = useUnvalidated(api, scope)
  const [arrangement, setArrangement] = useState<Arrangement>('cards')
  const [chosen, setChosen] = useState<StateFilter>('all')
  const decks = state.status === 'ready' ? state.decks : []
  const merged = mergeDecks(decks)
  const groups = groupByDeck(decks)
  const states = stateCounts(merged)
  const filter = chosen === 'all' || states.some(([name]) => name === chosen) ? chosen : 'all'
  const shown = filterCards(merged, filter)
  const { faces, pending } = useCardTexts(
    cards,
    merged.map((card) => card.oracleId),
  )
  const unread = pending ? 0 : merged.filter((card) => !faces.has(card.oracleId)).length
  const single = scope.kind === 'deck'

  const listOf = (items: readonly FlaggedCard[], showDecks: boolean, label: string) => (
    <ul aria-label={label} className="grid gap-3">
      {items.map((card) => (
        <UnvalidatedCard
          key={card.oracleId}
          card={card}
          faces={faces.get(card.oracleId)}
          pending={pending}
          showDecks={showDecks}
          fixMarks={fixMarks}
        />
      ))}
    </ul>
  )

  return (
    <section aria-labelledby="unvalidated-title" className="grid gap-6">
      <div className="grid gap-2">
        <h1 id="unvalidated-title" className="text-2xl font-semibold tracking-tight">
          Not validated cards
        </h1>
        <p className="max-w-prose text-sm text-muted-foreground">
          {single && decks[0] !== undefined
            ? `Cards of ${decks[0].name} that the game does not run whole, with the clauses it leaves to the table.`
            : 'Cards of your decks that the game does not run whole, with the clauses it leaves to the table. Copy the list to name what to fix first.'}
        </p>
        <div className="flex flex-wrap items-center gap-2">
          {single ? (
            <>
              <Link to="/decks/unvalidated" className={buttonVariants({ variant: 'outline', size: 'sm' })}>
                All decks
              </Link>
              <Link
                to="/decks/$deckId"
                params={{ deckId: scope.id }}
                className={buttonVariants({ variant: 'outline', size: 'sm' })}
              >
                Edit deck
              </Link>
            </>
          ) : (
            <Link to="/decks" className={buttonVariants({ variant: 'outline', size: 'sm' })}>
              Decks
            </Link>
          )}
        </div>
      </div>

      {state.status === 'loading' ? (
        <div className="grid gap-3">
          <Loading label="Checking your decks…">
            <ListSkeleton />
          </Loading>
          {state.total === null ? null : (
            <p className="text-sm text-muted-foreground">{`Checked ${state.done} of ${state.total} ${state.total === 1 ? 'deck' : 'decks'}…`}</p>
          )}
        </div>
      ) : null}

      {state.status === 'failed' ? (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>Could not check your decks</AlertTitle>
          <AlertDescription>{state.message}</AlertDescription>
          <div className="mt-2">
            {state.sessionEnded ? (
              <Link
                to="/login"
                search={{ next: single ? `/decks/${scope.id}/unvalidated` : '/decks/unvalidated' }}
                className={buttonVariants()}
              >
                Sign in
              </Link>
            ) : (
              <Button type="button" onClick={reload}>
                Retry
              </Button>
            )}
          </div>
        </Alert>
      ) : null}

      {state.status === 'unavailable' ? (
        <Alert variant="warning" role="status">
          <TriangleAlertIcon aria-hidden="true" />
          <AlertTitle>This server keeps no decks</AlertTitle>
          <AlertDescription>
            It runs without a database, so there are no saved decks to check.
          </AlertDescription>
        </Alert>
      ) : null}

      {state.status === 'ready' && state.failed.length > 0 ? (
        <Alert variant="warning" role="status" data-slot="unchecked">
          <TriangleAlertIcon aria-hidden="true" />
          <AlertTitle>{`Could not check ${state.failed.length} ${state.failed.length === 1 ? 'deck' : 'decks'}`}</AlertTitle>
          <AlertDescription>
            {`${state.failed.map((deck) => deck.name).join(', ')} ${state.failed.length === 1 ? 'is' : 'are'} not in the list below.`}
          </AlertDescription>
          <div className="mt-2">
            <Button type="button" variant="outline" onClick={reload}>
              Try again
            </Button>
          </div>
        </Alert>
      ) : null}

      {state.status === 'ready' && merged.length === 0 && state.failed.length === 0 ? (
        <EmptyState
          icon={ShieldCheckIcon}
          title={state.saved === 0 ? 'You have no decks yet' : 'The game runs every card'}
          action={
            state.saved === 0 ? (
              <Link to="/decks" className={buttonVariants({ variant: 'outline' })}>
                Go to your decks
              </Link>
            ) : undefined
          }
        >
          {state.saved === 0
            ? 'Import or build a deck, and this page lists the cards of it that the game does not run whole.'
            : single
              ? 'Every card of this deck is played by the game as written.'
              : 'Every card of your decks is played by the game as written.'}
        </EmptyState>
      ) : null}

      {state.status === 'ready' && merged.length > 0 ? (
        <>
          <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
            <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
              {single ? null : (
                <Choice<Arrangement>
                  label="Arrange the list"
                  value={arrangement}
                  options={[
                    { value: 'cards', text: 'By card' },
                    { value: 'decks', text: 'By deck' },
                  ]}
                  onChange={setArrangement}
                />
              )}
              <Choice<StateFilter>
                label="Show cards in state"
                value={filter}
                options={[
                  { value: 'all', text: `All (${merged.length})` },
                  ...states.map(([name, count]) => ({
                    value: name,
                    text: `${SUPPORT_LABEL[name]} (${count})`,
                  })),
                ]}
                onChange={setChosen}
              />
            </div>
            {pending ? (
              <Button type="button" variant="outline" size="sm" disabled loading>
                Reading card text…
              </Button>
            ) : (
              <CopyButton text={copyText(shown, faces)} label="Copy as text">
                Copy as text
              </CopyButton>
            )}
          </div>
          <p className="text-sm text-muted-foreground" data-slot="summary">
            {`${shown.length} ${shown.length === 1 ? 'card' : 'cards'} in ${single ? 'this deck' : `${groups.length} of ${state.saved} ${state.saved === 1 ? 'deck' : 'decks'}`}`}
          </p>
          {unread > 0 ? (
            <p className="text-sm text-muted-foreground" role="status">
              {`The text of ${unread} ${unread === 1 ? 'card' : 'cards'} could not be read, so ${unread === 1 ? 'its' : 'their'} clauses show without the quote.`}
            </p>
          ) : null}

          {arrangement === 'cards' || single ? (
            listOf(shown, !single, 'Cards not validated')
          ) : (
            <div className="grid gap-8">
              {groups.map((group) => {
                const inDeck = filterCards(group.cards, filter)
                return inDeck.length === 0 ? null : (
                  <section key={group.deck.id} data-deck={group.deck.id} className="grid gap-3">
                    <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
                      <h2 className="text-lg font-semibold break-words">{group.deck.name}</h2>
                      <Link
                        to="/decks/$deckId/unvalidated"
                        params={{ deckId: group.deck.id }}
                        className="text-sm text-muted-foreground underline-offset-4 hover:underline"
                      >
                        {`${inDeck.length} ${inDeck.length === 1 ? 'card' : 'cards'}, this deck only`}
                      </Link>
                    </div>
                    {listOf(inDeck, false, `Cards not validated in ${group.deck.name}`)}
                  </section>
                )
              })}
            </div>
          )}
        </>
      ) : null}
    </section>
  )
}
