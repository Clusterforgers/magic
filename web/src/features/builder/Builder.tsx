import { useRef, useState } from 'react'
import { PageShell } from '@/components/PageShell'
import { Button } from '@/components/ui/button'
import { describeDeckError, isSessionEnded } from '@/features/decks/errors'
import { STATE_LABEL } from '@/features/decks/format'
import { nameProblem as checkName } from '@/features/decks/rules'
import { usePageTitle } from '@/hooks/usePageTitle'
import { cn } from '@/lib/utils'
import type { CardsApi } from '@/net/cards'
import type { DecksApi } from '@/net/decks'
import type { CardHit, DeckView, DeckZone } from '@/protocol/generated'
import {
  addCard,
  bodyOf,
  copiesOf,
  decrement,
  draftOf,
  increment,
  lineIn,
  MAX_DECK_CARDS,
  moveLine,
  removeLine,
  sameDeck,
  totalOf,
} from './deckEdit'
import type { Draft } from './deckEdit'
import { DeckPane } from './DeckPane'
import type { DeckActions } from './DeckPane'
import { LeaveDialog } from './LeaveDialog'
import { SearchPane } from './SearchPane'
import { useUnsavedGuard } from './useUnsavedGuard'
import { ZONE_THE, ZONE_TO } from './zones'

type Shown = 'search' | 'deck'

// Under the M breakpoint one pane is on screen and this switch changes which; both stay mounted, so the search keeps
// its text, page and place when the deck is looked at.
function ViewSwitch({
  shown,
  onChange,
  count,
}: {
  shown: Shown
  onChange: (next: Shown) => void
  count: number
}) {
  return (
    <div role="group" aria-label="Show" className="grid grid-cols-2 gap-2 lg:hidden">
      <Button
        type="button"
        variant={shown === 'search' ? 'default' : 'outline'}
        aria-pressed={shown === 'search'}
        onClick={() => onChange('search')}
      >
        Search
      </Button>
      <Button
        type="button"
        variant={shown === 'deck' ? 'default' : 'outline'}
        aria-pressed={shown === 'deck'}
        onClick={() => onChange('deck')}
      >
        Deck <span className="tabular-nums">{count}</span>
      </Button>
    </div>
  )
}

interface BuilderProps {
  /** The deck as the server has it; the builder edits a copy. */
  view: DeckView
  api: DecksApi
  cards: CardsApi
}

/**
 * The deck builder for one stored deck: card search beside the deck. Edits stay in the browser until Save sends the
 * whole deck; the server checks it and answers the state that the badge then shows. Leaving with unsaved changes asks
 * first. From the `sm` breakpoint the page is as tall as the window under the header and the two panes scroll inside
 * their own regions; under `lg` a Search or Deck switch shows one at a time.
 */
export function Builder({ view, api, cards }: BuilderProps) {
  const [stored, setStored] = useState(() => ({ view, draft: draftOf(view) }))
  const [draft, setDraft] = useState<Draft>(stored.draft)
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<{
    message: string
    sessionEnded: boolean
    deckId: string
  } | null>(null)
  const [nameProblem, setNameProblem] = useState<string | null>(null)
  const [announcement, setAnnouncement] = useState('')
  const [shown, setShown] = useState<Shown>('search')
  const nameRef = useRef<HTMLInputElement>(null)
  const savingRef = useRef(false)

  const dirty = !sameDeck(draft, stored.draft)
  usePageTitle(`Edit ${stored.view.name}`)
  const blocker = useUnsavedGuard(dirty)

  const nameOf = (zone: DeckZone, oracleId: string): string =>
    lineIn(draft, zone, oracleId)?.card.name ?? 'The card'

  function add(card: CardHit, zone: DeckZone): void {
    const next = addCard(draft, card, zone)
    if (next === draft) {
      setAnnouncement(`The deck is at its limit of ${MAX_DECK_CARDS} cards.`)
      return
    }
    setDraft(next)
    const copies = copiesOf(next, card.oracle_id, zone)
    setAnnouncement(`Added ${card.name} to ${ZONE_THE[zone]}: ${copies} ${copies === 1 ? 'copy' : 'copies'}.`)
  }

  async function save(): Promise<void> {
    if (savingRef.current || !dirty) return
    const problem = checkName(draft.name)
    setNameProblem(problem)
    if (problem !== null) {
      nameRef.current?.focus()
      return
    }
    savingRef.current = true
    setSaving(true)
    setSaveError(null)
    const sent = draft
    try {
      const saved = await api.update(stored.view.id, bodyOf(sent))
      const fresh = draftOf(saved)
      setStored({ view: saved, draft: fresh })
      // Edits made while the save was in flight stay as they are; otherwise the draft is what the server stored.
      setDraft((current) => (current === sent ? fresh : current))
      setAnnouncement(`Saved “${saved.name}”: ${STATE_LABEL[saved.state]}.`)
    } catch (error) {
      setSaveError({
        message: describeDeckError(error),
        sessionEnded: isSessionEnded(error),
        deckId: stored.view.id,
      })
    } finally {
      savingRef.current = false
      setSaving(false)
    }
  }

  const actions: DeckActions = {
    rename: (name) => {
      setDraft((current) => ({ ...current, name }))
      setNameProblem(null)
    },
    setFormat: (format) => setDraft((current) => ({ ...current, format })),
    save: () => void save(),
    increment: (zone, id) => setDraft((current) => increment(current, zone, id)),
    decrement: (zone, id) => {
      setDraft((current) => decrement(current, zone, id))
      const line = lineIn(draft, zone, id)
      if (line?.count === 1) setAnnouncement(`Removed ${line.card.name} from ${ZONE_THE[zone]}.`)
    },
    remove: (zone, id) => {
      setAnnouncement(`Removed ${nameOf(zone, id)} from ${ZONE_THE[zone]}.`)
      setDraft((current) => removeLine(current, zone, id))
    },
    move: (zone, id, to, count) => {
      setAnnouncement(`Moved ${nameOf(zone, id)} to ${ZONE_TO[to]}.`)
      setDraft((current) => moveLine(current, zone, id, to, count))
    },
  }

  return (
    <PageShell width="wide" className="flex flex-col gap-3 py-4 sm:h-[calc(100dvh-3.5rem-1px)] sm:py-4">
      <h1 className="sr-only">Edit {stored.view.name}</h1>
      <ViewSwitch shown={shown} onChange={setShown} count={totalOf(draft)} />
      <div className="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)] gap-4 lg:grid-cols-2">
        <SearchPane
          api={cards}
          draft={draft}
          onAdd={add}
          className={cn(shown === 'search' ? 'flex' : 'hidden', 'lg:flex')}
        />
        <DeckPane
          draft={draft}
          state={stored.view.state}
          dirty={dirty}
          saving={saving}
          nameProblem={nameProblem}
          saveError={saveError}
          nameRef={nameRef}
          actions={actions}
          className={cn(shown === 'deck' ? 'flex' : 'hidden', 'lg:flex')}
        />
      </div>
      <p role="status" className="sr-only">
        {announcement}
      </p>
      <LeaveDialog blocker={blocker} name={draft.name} />
    </PageShell>
  )
}
