import { useRef, useState } from 'react'
import { CircleAlertIcon } from 'lucide-react'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { describeDeckError, isSessionEnded } from '@/features/decks/errors'
import { STATE_LABEL } from '@/features/decks/format'
import { nameProblem as checkName } from '@/features/decks/rules'
import { usePageTitle } from '@/hooks/usePageTitle'
import { cn } from '@/lib/utils'
import type { CardsApi } from '@/net/cards'
import type { DecksApi } from '@/net/decks'
import type { FixMarksApi } from '@/net/fixMarks'
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
  openingPane,
  removeLine,
  sameDeck,
  totalOf,
} from './deckEdit'
import type { Draft } from './deckEdit'
import { CardPreview } from './CardPreview'
import { judgeCommander } from './commanderCheck'
import type { FindKind } from './CommanderSlot'
import { cardsIn, identityOf } from './identity'
import { DeckPane } from './DeckPane'
import type { DeckActions } from './DeckPane'
import { LeaveDialog } from './LeaveDialog'
import { SearchPane } from './SearchPane'
import { NO_FILTERS } from './search'
import type { SearchFilters } from './search'
import { useLiveCheck } from './useLiveCheck'
import { useUnsavedGuard } from './useUnsavedGuard'
import { ZONE_THE, ZONE_TO } from './zones'

type Shown = 'search' | 'deck' | 'stats'

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
    <div role="group" aria-label="Show" className="grid grid-cols-3 gap-2 lg:hidden">
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
      <Button
        type="button"
        variant={shown === 'stats' ? 'default' : 'outline'}
        aria-pressed={shown === 'stats'}
        onClick={() => onChange('stats')}
      >
        Stats
      </Button>
    </div>
  )
}

interface BuilderProps {
  /** The deck as the server has it; the builder edits a copy. */
  view: DeckView
  api: DecksApi
  cards: CardsApi
  /** The fix-mark endpoints; with them a card's preview offers "Mark important to fix". */
  fixMarks?: FixMarksApi
}

/**
 * The deck builder for one stored deck: card search beside the deck. Edits stay in the browser until Save sends the
 * whole deck; the server checks it and answers the report that the deck pane then shows, with the marker. While the
 * draft's cards differ from the saved ones the deck is checked live (`useLiveCheck`), so the counts, marks and reasons
 * follow the draft. A card's name opens its preview. Leaving with unsaved changes asks first. From the `sm` breakpoint
 * the page is as tall as the window under the header and the two panes scroll inside their own regions; under `lg` a
 * Search or Deck switch shows one at a time.
 */
export function Builder({ view, api, cards, fixMarks }: BuilderProps) {
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
  const [shown, setShown] = useState<Shown>(() => openingPane(stored.draft))
  // The card whose preview is open, kept after it closes so it fades out whole, and the control that opened it.
  const [preview, setPreview] = useState<{ card: CardHit; open: boolean } | null>(null)
  const opener = useRef<HTMLElement | null>(null)
  const nameRef = useRef<HTMLInputElement>(null)
  const savingRef = useRef(false)
  // The commander being checked, the choice the check refused, and the search the slot asked for.
  const [judging, setJudging] = useState<string | null>(null)
  const [refusal, setRefusal] = useState<{ name: string; reason: string } | null>(null)
  const [seed, setSeed] = useState<{ id: number; filters: SearchFilters } | null>(null)
  const judgingRef = useRef(false)

  const dirty = !sameDeck(draft, stored.draft)
  const commanders = draft.format === 'commander' ? cardsIn(draft.lines, 'commander') : []
  const commanderIdentity = commanders.length === 0 ? null : identityOf(commanders)
  const check = useLiveCheck(api, { draft: stored.draft, report: stored.view.report }, draft)
  usePageTitle(`Edit ${stored.view.name}`)
  const blocker = useUnsavedGuard(dirty)

  const nameOf = (zone: DeckZone, oracleId: string): string =>
    lineIn(draft, zone, oracleId)?.card.name ?? 'The card'

  // A card joins the command zone of a Commander deck only when the deck check accepts it there, alone or beside the
  // commander already chosen; otherwise the reason is shown and nothing changes. `from` is the zone a move leaves.
  async function chooseCommander(card: CardHit, from: DeckZone | null): Promise<void> {
    if (judgingRef.current) return
    judgingRef.current = true
    setJudging(card.name)
    setRefusal(null)
    const verdict = await judgeCommander(api, draft, card)
    judgingRef.current = false
    setJudging(null)
    if (!verdict.ok) {
      setRefusal({ name: card.name, reason: verdict.reason })
      setAnnouncement(`${card.name} cannot be a commander. ${verdict.reason}`)
      return
    }
    setDraft((current) =>
      from === null
        ? addCard(current, card, 'commander')
        : moveLine(current, from, card.oracle_id, 'commander', 1),
    )
    setAnnouncement(`${card.name} is now a commander.`)
  }

  function find(kind: FindKind): void {
    const filters: SearchFilters =
      kind === 'commander'
        ? { ...NO_FILTERS, type: 'legendary creature' }
        : kind === 'partner'
          ? { ...NO_FILTERS, text: 'partner', type: 'legendary creature' }
          : { ...NO_FILTERS, type: 'background' }
    setSeed((current) => ({ id: (current?.id ?? 0) + 1, filters }))
    setShown('search')
  }

  function add(card: CardHit, zone: DeckZone): void {
    if (zone === 'commander' && draft.format === 'commander') {
      void chooseCommander(card, null)
      return
    }
    const next = addCard(draft, card, zone)
    if (next === draft) {
      setAnnouncement(`The deck is at its limit of ${MAX_DECK_CARDS} cards.`)
      return
    }
    setDraft(next)
    const copies = copiesOf(next, card.oracle_id, zone)
    setAnnouncement(`Added ${card.name} to ${ZONE_THE[zone]}: ${copies} ${copies === 1 ? 'copy' : 'copies'}.`)
  }

  function showPreview(card: CardHit, from: HTMLElement): void {
    opener.current = from
    setPreview({ card, open: true })
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
      const card = lineIn(draft, zone, id)?.card
      if (to === 'commander' && draft.format === 'commander' && card !== undefined) {
        void chooseCommander(card, zone)
        return
      }
      setAnnouncement(`Moved ${nameOf(zone, id)} to ${ZONE_TO[to]}.`)
      setDraft((current) => moveLine(current, zone, id, to, count))
    },
    preview: showPreview,
  }

  return (
    <PageShell width="wide" className="flex flex-col gap-3 py-4 sm:h-[calc(100dvh-3.5rem-1px)] sm:py-4">
      <h1 className="sr-only">Edit {stored.view.name}</h1>
      <ViewSwitch shown={shown} onChange={setShown} count={totalOf(draft)} />
      {refusal === null ? null : (
        <Alert variant="destructive" data-slot="commander-refusal">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>{refusal.name} cannot be added as a commander</AlertTitle>
          <AlertDescription>{refusal.reason}</AlertDescription>
          <div className="mt-2">
            <Button type="button" variant="outline" size="sm" onClick={() => setRefusal(null)}>
              Dismiss
            </Button>
          </div>
        </Alert>
      )}
      <div className="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)] gap-4 lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)]">
        <SearchPane
          api={cards}
          draft={draft}
          onAdd={add}
          onPreview={showPreview}
          commanderIdentity={commanderIdentity}
          seed={seed}
          fixMarks={fixMarks}
          className={cn(shown === 'search' ? 'flex' : 'hidden', 'lg:flex')}
        />
        <DeckPane
          draft={draft}
          deckId={stored.view.id}
          state={stored.view.state}
          changed={stored.view.changed}
          check={check}
          dirty={dirty}
          saving={saving}
          nameProblem={nameProblem}
          saveError={saveError}
          nameRef={nameRef}
          actions={actions}
          judging={judging}
          onFind={find}
          part={shown === 'stats' ? 'stats' : 'deck'}
          fixMarks={fixMarks}
          onShowHand={() => setShown('stats')}
          className={cn(shown === 'search' ? 'hidden' : 'flex', 'lg:flex')}
        />
      </div>
      <p role="status" className="sr-only">
        {announcement}
      </p>
      <CardPreview
        card={preview?.card ?? null}
        open={preview?.open === true}
        api={cards}
        fixMarks={fixMarks}
        finalFocus={opener}
        onClose={() => setPreview((current) => current && { ...current, open: false })}
      />
      <LeaveDialog blocker={blocker} name={draft.name} />
    </PageShell>
  )
}
