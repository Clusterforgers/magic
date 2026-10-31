import { CircleAlertIcon } from 'lucide-react'
import { useRef, useState } from 'react'
import type { ChangeEvent, FormEvent } from 'react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import type { DecksApi } from '@/net/decks'
import type { DeckBody, DeckReport, DeckView, ImportResult } from '@/protocol/generated'
import { FormatField, NameField } from './DeckFields'
import { describeDeckError } from './errors'
import { DEFAULT_FORMAT, formatLabel, importSummary } from './format'
import { ImportCheck } from './ImportCheck'
import { ImportNotes } from './ImportNotes'
import { fileProblem, listProblem, nameFromCommanders, nameFromFile, nameProblem } from './rules'

// Whole class names, so Tailwind can see them. The same edge, fill and focus as `Input`, for a block of lines.
const TEXTAREA =
  'min-h-28 w-full resize-y rounded-lg border border-input bg-background px-3 py-2 font-mono text-base text-foreground transition-colors duration-fast ease-out placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:bg-muted disabled:opacity-50 aria-invalid:border-destructive'

type Step = { kind: 'list' } | { kind: 'review'; result: ImportResult; report: DeckReport | null }

interface ImportDialogProps {
  api: DecksApi
  /** The server stored the deck. */
  onSaved: (deck: DeckView) => void
  /** The dialog has closed, by Cancel, Escape or a successful save; the caller unmounts it. */
  onClosed: () => void
}

/** The lines of an import as the cards of a deck body. */
const lines = (result: ImportResult): DeckBody['cards'] =>
  result.cards.map((line) => ({
    zone: line.zone,
    oracle_id: line.card.oracle_id,
    printing_id: line.printing_id,
    count: line.count,
  }))

function Failure({ message }: { message: string }) {
  return (
    <Alert variant="destructive">
      <CircleAlertIcon aria-hidden="true" />
      <AlertDescription className="text-foreground">{message}</AlertDescription>
    </Alert>
  )
}

/**
 * "Import deck" in two steps. First a pasted list or a chosen file and a format; the server reads it (nothing is saved)
 * and detects the kind of list. Then the review: what was read, how far the game runs it and what it breaks in the
 * format, what was left out or not found, and the name to save the deck under.
 */
export function ImportDialog({ api, onSaved, onClosed }: ImportDialogProps) {
  const [open, setOpen] = useState(true)
  const [step, setStep] = useState<Step>({ kind: 'list' })
  const [text, setText] = useState('')
  const [format, setFormat] = useState(DEFAULT_FORMAT)
  const [suggested, setSuggested] = useState('')
  const [name, setName] = useState('')
  const [pending, setPending] = useState(false)
  const [listError, setListError] = useState<string | null>(null)
  const [nameError, setNameError] = useState<string | null>(null)
  const [formError, setFormError] = useState<string | null>(null)
  const textRef = useRef<HTMLTextAreaElement>(null)
  const nameRef = useRef<HTMLInputElement>(null)

  async function choose(event: ChangeEvent<HTMLInputElement>): Promise<void> {
    const file = event.target.files?.[0]
    if (file === undefined) return
    const problem = fileProblem(file)
    if (problem !== null) {
      setListError(problem)
      return
    }
    try {
      setText(await file.text())
      setSuggested(nameFromFile(file.name))
      setListError(null)
    } catch {
      setListError('That file could not be read. Paste the list instead.')
    }
  }

  /** The check of what was read, so the review can name its support; null when the check does not answer. */
  async function checked(result: ImportResult): Promise<DeckReport | null> {
    if (result.cards.length === 0) return null
    try {
      return await api.check({ deck: null, format, cards: lines(result) })
    } catch {
      return null
    }
  }

  async function read(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (pending) return
    const problem = listProblem(text)
    setListError(problem)
    setFormError(null)
    if (problem !== null) {
      textRef.current?.focus()
      return
    }
    setPending(true)
    try {
      const result = await api.importList({ text, source: null, format })
      // A file name comes first; a pasted list has none, so its commanders name it.
      setName((current) => (current !== '' ? current : suggested || nameFromCommanders(result.cards)))
      setStep({ kind: 'review', result, report: await checked(result) })
    } catch (error) {
      setFormError(describeDeckError(error))
    } finally {
      setPending(false)
    }
  }

  async function save(event: FormEvent<HTMLFormElement>, result: ImportResult): Promise<void> {
    event.preventDefault()
    if (pending) return
    const problem = nameProblem(name)
    setNameError(problem)
    setFormError(null)
    if (problem !== null) {
      nameRef.current?.focus()
      return
    }
    const deck: DeckBody = {
      name: name.trim(),
      format,
      notes: '',
      cards: lines(result),
    }
    setPending(true)
    try {
      onSaved(await api.create(deck))
      setOpen(false)
    } catch (error) {
      setFormError(describeDeckError(error))
    } finally {
      setPending(false)
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!pending) setOpen(next)
      }}
      onOpenChangeComplete={(isOpen) => {
        if (!isOpen) onClosed()
      }}
    >
      <DialogContent className="max-w-xl">
        {step.kind === 'list' ? (
          <form noValidate aria-busy={pending} className="grid gap-4" onSubmit={(event) => void read(event)}>
            <div className="grid gap-1">
              <DialogTitle>Import a deck</DialogTitle>
              <DialogDescription>
                Paste a list from Arena, MTG Online, Moxfield or Archidekt, or choose a file. The server
                detects which kind it is; nothing is saved until you have reviewed it.
              </DialogDescription>
            </div>
            {formError === null ? null : <Failure message={formError} />}
            <div className="grid gap-2">
              <Label htmlFor="import-text">Deck list</Label>
              <textarea
                ref={textRef}
                id="import-text"
                value={text}
                rows={5}
                spellCheck={false}
                readOnly={pending}
                placeholder={'4 Lightning Bolt\n20 Mountain'}
                aria-invalid={listError !== null}
                aria-describedby={listError === null ? undefined : 'import-text-problem'}
                className={TEXTAREA}
                onChange={(event) => {
                  setText(event.target.value)
                  setListError(null)
                }}
              />
              {listError === null ? null : (
                <p id="import-text-problem" className="text-sm text-destructive">
                  {listError}
                </p>
              )}
            </div>
            <div className="grid gap-2">
              <Label htmlFor="import-file">Or choose a file</Label>
              <Input
                id="import-file"
                type="file"
                accept=".txt,.dek,.csv,text/plain"
                disabled={pending}
                onChange={(event) => void choose(event)}
              />
            </div>
            <FormatField
              id="import-format"
              value={format}
              disabled={pending}
              onChange={setFormat}
              hint="The rules the deck is checked against. For a Moxfield list, Commander reads the first line as the commander."
            />
            <div className="flex flex-wrap justify-end gap-2">
              <Button type="button" variant="outline" onClick={() => setOpen(false)}>
                Cancel
              </Button>
              <Button type="submit" loading={pending}>
                Read list
              </Button>
            </div>
          </form>
        ) : (
          <form
            noValidate
            aria-busy={pending}
            className="grid gap-4"
            onSubmit={(event) => void save(event, step.result)}
          >
            <div className="grid gap-1">
              <DialogTitle>Review the list</DialogTitle>
              <DialogDescription>{importSummary(step.result)}</DialogDescription>
            </div>
            {formError === null ? null : <Failure message={formError} />}
            {step.result.cards.length === 0 ? null : <ImportCheck report={step.report} />}
            <ImportNotes result={step.result} />
            {step.result.cards.length === 0 ? (
              <Alert variant="destructive" role="status">
                <CircleAlertIcon aria-hidden="true" />
                <AlertDescription className="text-foreground">
                  No cards could be read from this list. Go back and check it.
                </AlertDescription>
              </Alert>
            ) : (
              <>
                <NameField
                  id="import-name"
                  value={name}
                  problem={nameError}
                  disabled={pending}
                  inputRef={nameRef}
                  autoFocus
                  onChange={(value) => {
                    setName(value)
                    setNameError(null)
                  }}
                />
                <p className="text-sm text-muted-foreground">
                  Format: {formatLabel(format)}. Go back to change it.
                </p>
              </>
            )}
            <div className="flex flex-wrap justify-end gap-2">
              <Button
                type="button"
                variant="outline"
                disabled={pending}
                onClick={() => {
                  setFormError(null)
                  setStep({ kind: 'list' })
                }}
              >
                Back
              </Button>
              {step.result.cards.length === 0 ? null : (
                <Button type="submit" loading={pending}>
                  Save deck
                </Button>
              )}
            </div>
          </form>
        )}
      </DialogContent>
    </Dialog>
  )
}
