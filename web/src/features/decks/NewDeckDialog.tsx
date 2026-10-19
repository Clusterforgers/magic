import { CircleAlertIcon } from 'lucide-react'
import { useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import type { DecksApi } from '@/net/decks'
import type { DeckView } from '@/protocol/generated'
import { FormatField, NameField } from './DeckFields'
import { describeDeckError } from './errors'
import { DEFAULT_FORMAT } from './format'
import { nameProblem } from './rules'

interface NewDeckDialogProps {
  api: DecksApi
  /** The server stored the deck. */
  onSaved: (deck: DeckView) => void
  /** The dialog has closed, by Cancel, Escape or a successful save; the caller unmounts it. */
  onClosed: () => void
}

/** "New deck": a name and a format, saved as a deck without cards. */
export function NewDeckDialog({ api, onSaved, onClosed }: NewDeckDialogProps) {
  const [open, setOpen] = useState(true)
  const [name, setName] = useState('')
  const [format, setFormat] = useState(DEFAULT_FORMAT)
  const [pending, setPending] = useState(false)
  const [nameError, setNameError] = useState<string | null>(null)
  const [formError, setFormError] = useState<string | null>(null)
  const nameRef = useRef<HTMLInputElement>(null)

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (pending) return
    const problem = nameProblem(name)
    setNameError(problem)
    setFormError(null)
    if (problem !== null) {
      nameRef.current?.focus()
      return
    }
    setPending(true)
    try {
      onSaved(await api.create({ name: name.trim(), format, notes: '', cards: [] }))
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
      <DialogContent className="max-w-md">
        <form noValidate aria-busy={pending} className="grid gap-4" onSubmit={(event) => void submit(event)}>
          <div className="grid gap-1">
            <DialogTitle>New deck</DialogTitle>
            <DialogDescription>
              Name the deck and choose the format it is checked against. Cards come from an import.
            </DialogDescription>
          </div>
          {formError === null ? null : (
            <Alert variant="destructive">
              <CircleAlertIcon aria-hidden="true" />
              <AlertDescription className="text-foreground">{formError}</AlertDescription>
            </Alert>
          )}
          <NameField
            id="new-deck-name"
            value={name}
            problem={nameError}
            disabled={pending}
            inputRef={nameRef}
            onChange={(value) => {
              setName(value)
              setNameError(null)
            }}
          />
          <FormatField id="new-deck-format" value={format} disabled={pending} onChange={setFormat} />
          <div className="flex flex-wrap justify-end gap-2">
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" loading={pending}>
              Create deck
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
