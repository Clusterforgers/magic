import { BookOpenIcon, CheckIcon, XIcon } from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'
import { Kbd } from '@/components/Kbd'
import { Loading } from '@/components/Loading'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { describeDeckError } from '@/features/decks/errors'
import { cn } from '@/lib/utils'
import type { ReadsApi } from '@/net/reads'
import type { ReadCard, ReadVerdict } from '@/protocol/generated'
import {
  MAX_READ_NOTE_CHARS,
  READ_DELAY_MS,
  STATE_WORDS,
  countsLine,
  differsCount,
  noteProblem,
  verdictKey,
} from './model'
import { ReadSheet } from './ReadSheet'
import { isStaleRead, useReadCard } from './useReadCard'

interface ReadCardViewProps {
  /** The read endpoints. */
  api: Pick<ReadsApi, 'get' | 'judge'>
  /** The card to read. */
  oracleId: string
  /** Hears of every verdict the server took, with the card as it stands after it. */
  onJudged?: (card: ReadCard, verdict: ReadVerdict) => void
  /** Whether C and W give a verdict: off while another card or dialog has the keys. */
  keys?: boolean
  /** The level of the card's name. */
  heading?: 'h2' | 'h3'
  /** Whether the card's name is drawn: off where a dialog title already names it. */
  named?: boolean
  /** Classes of the verdict bar: a dialog keeps it stuck to its foot while the clauses scroll. */
  verdictClassName?: string
}

/** True once the card has been in view for `READ_DELAY_MS`; starts again for every new draft. */
function useReadDelay(hash: string | null): boolean {
  const [readyFor, setReadyFor] = useState<string | null>(null)
  useEffect(() => {
    if (hash === null) return
    const timer = window.setTimeout(() => setReadyFor(hash), READ_DELAY_MS)
    return () => window.clearTimeout(timer)
  }, [hash])
  return hash !== null && readyFor === hash
}

function MineLine({ card }: { card: ReadCard }) {
  const mine = card.mine
  if (mine === null) return null
  const said = mine.verdict === 'correct' ? 'You found it correct' : 'You found it wrong'
  return (
    <p data-slot="read-mine" className="text-sm text-muted-foreground">
      {mine.current ? `${said}.` : `${said} before the rules changed; read the new version.`}
      {mine.verdict === 'wrong' && mine.note !== null ? ` “${mine.note}”` : ''}
    </p>
  )
}

interface VerdictProps {
  card: ReadCard
  ready: boolean
  sending: ReturnType<typeof useReadCard>['sending']
  send: (verdict: ReadVerdict, note: string | null) => void
  reload: () => void
  keys: boolean
  className?: string
}

/** Correct and Wrong, with their keys, the note a `wrong` needs, and what the server said. */
function VerdictBar({ card, ready, sending, send, reload, keys, className }: VerdictProps) {
  const [noting, setNoting] = useState(false)
  const [note, setNote] = useState('')
  const [tried, setTried] = useState(false)
  const field = useId()
  const problemId = useId()
  const area = useRef<HTMLTextAreaElement>(null)
  const busy = sending.status === 'sending'
  const live = ready && !busy
  const problem = noteProblem(note)

  useEffect(() => {
    if (noting) area.current?.focus()
  }, [noting])

  useEffect(() => {
    if (!keys) return
    const onKey = (event: KeyboardEvent): void => {
      const verdict = verdictKey(event)
      if (verdict === null || !live) return
      event.preventDefault()
      if (verdict === 'correct') send('correct', null)
      else setNoting(true)
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [keys, live, send])

  const submitWrong = (): void => {
    setTried(true)
    if (problem !== null || !live) return
    send('wrong', note.trim())
  }

  return (
    <div data-slot="read-verdict" className={cn('grid gap-3', className)}>
      <p aria-live="polite" className="text-sm text-muted-foreground">
        {ready
          ? 'Does the game do what the card says?'
          : 'Read every clause first; you can judge in a moment.'}
      </p>
      <div className="flex flex-wrap gap-2">
        <Button
          data-verdict="correct"
          aria-disabled={!live}
          onClick={() => {
            if (live) send('correct', null)
          }}
        >
          <CheckIcon aria-hidden="true" />
          Correct
          <Kbd aria-hidden="true">C</Kbd>
        </Button>
        <Button
          data-verdict="wrong"
          variant="outline"
          aria-disabled={!live}
          aria-expanded={noting}
          onClick={() => {
            if (live) setNoting(true)
          }}
        >
          <XIcon aria-hidden="true" />
          Wrong
          <Kbd aria-hidden="true">W</Kbd>
        </Button>
      </div>
      {noting ? (
        <form
          className="grid gap-2"
          onSubmit={(event) => {
            event.preventDefault()
            submitWrong()
          }}
        >
          <label htmlFor={field} className="text-sm font-medium">
            What do the rules do wrong?
          </label>
          <textarea
            id={field}
            ref={area}
            value={note}
            rows={3}
            maxLength={MAX_READ_NOTE_CHARS}
            aria-required="true"
            aria-invalid={tried && problem !== null}
            aria-describedby={problemId}
            className="min-h-20 w-full resize-y rounded-lg border border-input bg-background px-3 py-2 text-base text-foreground placeholder:text-muted-foreground"
            onChange={(event) => setNote(event.target.value)}
          />
          <div className="flex flex-wrap items-center justify-between gap-2">
            <p id={problemId} aria-live="polite" className="text-xs text-muted-foreground tabular-nums">
              {tried && problem !== null ? problem : `${note.length} of ${MAX_READ_NOTE_CHARS}`}
            </p>
            <div className="flex gap-2">
              <Button type="button" variant="ghost" size="sm" onClick={() => setNoting(false)}>
                Cancel
              </Button>
              <Button type="submit" size="sm" aria-disabled={!live}>
                Send: wrong
              </Button>
            </div>
          </div>
        </form>
      ) : null}
      {sending.status === 'failed' ? (
        <Alert variant="destructive" role="alert">
          <AlertTitle>
            {isStaleRead(sending.error) ? 'The rules changed' : 'The verdict was not saved'}
          </AlertTitle>
          <AlertDescription className="grid justify-items-start gap-2">
            <span>{describeDeckError(sending.error)}</span>
            {isStaleRead(sending.error) ? (
              <Button variant="outline" size="sm" onClick={reload}>
                Load the new version
              </Button>
            ) : null}
          </AlertDescription>
        </Alert>
      ) : null}
      {card.mine?.current === true && sending.status === 'sent' ? (
        <p role="status" className="text-sm font-medium">
          Saved. Thank you for reading.
        </p>
      ) : null}
    </div>
  )
}

/**
 * One card to read (docs/web.md, Reading cards): its name and read state, the read sheet, and the verdict. Correct and
 * Wrong wait `READ_DELAY_MS` after the card appears, a `wrong` needs a note, and a draft that changed meanwhile asks to
 * be loaded again.
 */
export function ReadCardView({
  api,
  oracleId,
  onJudged,
  keys = true,
  heading: Heading = 'h2',
  named = true,
  verdictClassName,
}: ReadCardViewProps) {
  const { load, sending, send, reload } = useReadCard(api, oracleId, onJudged)
  const ready = useReadDelay(load.status === 'ready' ? load.card.read_hash : null)

  if (load.status === 'loading') {
    return (
      <Loading label="Loading the card's rules…">
        <div className="grid gap-3">
          <Skeleton className="h-7 w-48" />
          <Skeleton className="h-24 w-full" />
          <Skeleton className="h-24 w-full" />
        </div>
      </Loading>
    )
  }
  if (load.status === 'missing') {
    return <p className="text-sm text-muted-foreground">This card has no draft of its rules to read now.</p>
  }
  if (load.status === 'error') {
    return (
      <Alert variant="destructive" role="alert">
        <AlertTitle>The card could not be loaded</AlertTitle>
        <AlertDescription className="grid justify-items-start gap-2">
          <span>{describeDeckError(load.error)}</span>
          <Button variant="outline" size="sm" onClick={reload}>
            Try again
          </Button>
        </AlertDescription>
      </Alert>
    )
  }
  const { card } = load
  const counts = countsLine(card)
  const differs = differsCount(card)
  return (
    <article data-slot="read-card" data-oracle-id={card.oracle_id} className="grid gap-4">
      <header className="grid gap-1">
        <div className="flex flex-wrap items-center gap-2">
          <BookOpenIcon aria-hidden="true" className="size-5 shrink-0 text-support-read" />
          {named ? <Heading className="text-xl font-semibold tracking-tight">{card.name}</Heading> : null}
          <Badge variant="outline" data-state={card.state}>
            {STATE_WORDS[card.state]}
          </Badge>
        </div>
        {counts === null ? null : <p className="text-sm text-muted-foreground">{counts}.</p>}
        {differs === 0 ? null : (
          <p className="text-sm text-foreground">
            {differs === 1
              ? '1 clause is worded differently by the game. Compare it closely.'
              : `${differs} clauses are worded differently by the game. Compare them closely.`}
          </p>
        )}
        <MineLine card={card} />
      </header>
      <ReadSheet sheet={card.sheet} />
      <VerdictBar
        card={card}
        ready={ready}
        sending={sending}
        send={send}
        reload={reload}
        keys={keys}
        className={verdictClassName}
      />
    </article>
  )
}
