import { CircleAlertIcon } from 'lucide-react'
import { useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { nameProblem } from '@/features/create/form'
import { loadGuestName, saveGuestName } from './guestName'

const NAME_ID = 'join-name'
const PROBLEM_ID = 'join-name-problem'

interface Props {
  /** What the guest is joining: "Join Bo's game", or "Join with a code". */
  title: string
  /** False while the dialog closes; it stays mounted until `onClosed`. */
  open: boolean
  /** The request is on its way: the name cannot change and the button spins. */
  pending: boolean
  /** Why the last request failed, when sending the same name again could work. */
  failure: string | null
  /** The element that takes focus when the dialog closes; null leaves that to the dialog. */
  returnFocus(): HTMLElement | null
  /** The name to join under, trimmed and checked. */
  onSubmit(name: string): void
  /** Cancel, Escape and the close button. */
  onCancel(): void
  /** The dialog has finished closing. */
  onClosed(): void
  /** The words of the submit button; "Join game" by default. */
  submitLabel?: string
}

/** "Join Bo's game": what a guest is called at the table. The name is remembered for the next join. */
export function GuestJoinDialog({
  title,
  open,
  pending,
  failure,
  returnFocus,
  onSubmit,
  onCancel,
  onClosed,
  submitLabel = 'Join game',
}: Props) {
  const [name, setName] = useState(loadGuestName)
  const [problem, setProblem] = useState<string | null>(null)
  const nameField = useRef<HTMLInputElement>(null)

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    if (pending) return
    const wrong = nameProblem(name)
    setProblem(wrong)
    if (wrong !== null) {
      nameField.current?.focus()
      return
    }
    const trimmed = name.trim()
    saveGuestName(trimmed)
    onSubmit(trimmed)
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onCancel()
      }}
      onOpenChangeComplete={(isOpen) => {
        if (!isOpen) onClosed()
      }}
    >
      <DialogContent className="max-w-md" initialFocus={nameField} finalFocus={() => returnFocus() ?? true}>
        <form noValidate aria-busy={pending} className="grid gap-4" onSubmit={submit}>
          <div className="grid gap-1">
            <DialogTitle>{title}</DialogTitle>
            <DialogDescription>Choose the name the other players see at the table.</DialogDescription>
          </div>
          {failure === null ? null : (
            <Alert variant="destructive">
              <CircleAlertIcon aria-hidden="true" />
              <AlertDescription className="text-foreground">{failure}</AlertDescription>
            </Alert>
          )}
          <div className="grid gap-2">
            <Label htmlFor={NAME_ID}>Your name</Label>
            <Input
              ref={nameField}
              id={NAME_ID}
              name="name"
              value={name}
              readOnly={pending}
              autoComplete="nickname"
              aria-invalid={problem === null ? undefined : true}
              aria-describedby={problem === null ? undefined : PROBLEM_ID}
              onChange={(event) => {
                setName(event.target.value)
                setProblem(null)
              }}
            />
            {problem === null ? null : (
              <p id={PROBLEM_ID} className="text-sm text-destructive">
                {problem}
              </p>
            )}
          </div>
          <div className="flex flex-wrap justify-end gap-2">
            <Button type="button" variant="outline" onClick={onCancel}>
              Cancel
            </Button>
            <Button type="submit" loading={pending}>
              {submitLabel}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
