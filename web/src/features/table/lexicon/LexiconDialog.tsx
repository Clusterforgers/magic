import { useRef } from 'react'
import { useStore } from 'zustand'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { KINDS, pieces, searchTerms } from '@/lib/glossary'
import type { GlossaryKind, Term } from '@/lib/glossary'
import { lexiconStore, setLexiconOpen } from './lexiconStore'

/** The filter buttons' words. */
const KIND_LABELS: Record<GlossaryKind | 'all', string> = {
  all: 'All',
  keyword: 'Keywords',
  action: 'Actions',
  'ability word': 'Ability words',
  rule: 'Rules',
}

/** A term's text with each other term it names as a button that opens that term. */
function LinkedText({ term, onOpen }: { term: Term; onOpen: (title: string) => void }) {
  return (
    <p>
      {pieces(term.text).map((piece, index) =>
        piece.term === undefined || piece.term === term ? (
          <span key={index}>{piece.text}</span>
        ) : (
          <button
            key={index}
            type="button"
            data-slot="lexicon-link"
            onClick={() => onOpen(piece.term?.title ?? piece.text)}
            className="cursor-pointer rounded-sm text-primary underline decoration-dotted underline-offset-2 hover:decoration-solid"
          >
            {piece.text}
          </button>
        ),
      )}
    </p>
  )
}

/**
 * The rules lexicon: every keyword ability, keyword action, ability word and rules term with its reminder text, searched
 * by name or by what it does and filtered by kind. A term named in another's text opens it.
 */
export function LexiconDialog() {
  const { open, query, kind } = useStore(lexiconStore)
  const list = useRef<HTMLUListElement>(null)
  const matching = searchTerms(query)
  const found = kind === 'all' ? matching : matching.filter((term) => term.kind === kind)
  const count = (each: GlossaryKind | 'all'): number =>
    each === 'all' ? matching.length : matching.filter((term) => term.kind === each).length
  const openTerm = (title: string): void => {
    lexiconStore.setState({ query: title, kind: 'all' })
    if (list.current !== null) list.current.scrollTop = 0
  }
  return (
    <Dialog open={open} onOpenChange={setLexiconOpen}>
      <DialogContent
        data-slot="lexicon"
        className="h-[min(85dvh,52rem)] max-w-2xl grid-rows-[auto_auto_auto_minmax(0,1fr)] gap-3 overflow-hidden p-4 sm:p-6"
      >
        <div className="grid gap-1">
          <DialogTitle>Rules lexicon</DialogTitle>
          <DialogDescription>
            Keywords, actions, ability words and the rules of the game. Underlined words open their own entry.
          </DialogDescription>
        </div>
        <Input
          type="search"
          value={query}
          onChange={(event) => lexiconStore.setState({ query: event.target.value })}
          placeholder="Search by name or by what it does"
          aria-label="Search the lexicon"
        />
        <div role="group" aria-label="Show" className="flex flex-wrap gap-1">
          {(['all', ...KINDS] as const).map((each) => (
            <Button
              key={each}
              type="button"
              size="xs"
              variant={kind === each ? 'secondary' : 'ghost'}
              aria-pressed={kind === each}
              data-kind={each}
              onClick={() => lexiconStore.setState({ kind: each })}
              className="aria-pressed:font-semibold aria-pressed:ring-1 aria-pressed:ring-border-strong"
            >
              {KIND_LABELS[each]}
              <span className="text-muted-foreground tabular-nums">{count(each)}</span>
            </Button>
          ))}
        </div>
        {found.length === 0 ? (
          <p className="text-sm text-muted-foreground">Nothing matches.</p>
        ) : (
          <ul
            ref={list}
            aria-label="Lexicon entries"
            className="-mx-1 grid min-h-0 content-start gap-3 overflow-y-auto px-1 text-sm"
          >
            {found.map((term) => (
              <li key={`${term.kind}:${term.title}`} data-slot="lexicon-entry" className="grid gap-0.5">
                <p className="flex flex-wrap items-baseline gap-x-2">
                  <span className="font-medium">{term.title}</span>
                  <span className="text-xs text-muted-foreground">
                    {term.topic === undefined ? term.kind : `rule, ${term.topic.toLowerCase()}`}
                  </span>
                </p>
                <LinkedText term={term} onOpen={openTerm} />
                {term.names.length > 1 ? (
                  <p className="text-xs text-muted-foreground">Also: {term.names.slice(1).join(', ')}</p>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </DialogContent>
    </Dialog>
  )
}
