import { useState } from 'react'
import type { FormEvent } from 'react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { groupNameProblem } from './grouping'

interface GroupNameDialogProps {
  /** The name of the group being renamed; absent for a new group. */
  initial?: string
  onSubmit(name: string): void
  /** The dialog has closed; the caller unmounts it. */
  onClosed(): void
}

/** "New group" or "Rename group": one labelled name field. */
export function GroupNameDialog({ initial, onSubmit, onClosed }: GroupNameDialogProps) {
  const [open, setOpen] = useState(true)
  const [name, setName] = useState(initial ?? '')
  const [problem, setProblem] = useState<string | null>(null)
  const renaming = initial !== undefined

  const submit = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault()
    const wrong = groupNameProblem(name)
    setProblem(wrong)
    if (wrong !== null) return
    onSubmit(name.trim())
    setOpen(false)
  }

  return (
    <Dialog
      open={open}
      onOpenChange={setOpen}
      onOpenChangeComplete={(isOpen) => {
        if (!isOpen) onClosed()
      }}
    >
      <DialogContent className="max-w-sm">
        <form noValidate className="grid gap-4" onSubmit={submit}>
          <div className="grid gap-1">
            <DialogTitle>{renaming ? 'Rename group' : 'New group'}</DialogTitle>
            <DialogDescription>
              {renaming
                ? 'The decks in it stay where they are.'
                : 'Groups sort your decks, for example by how you play them.'}
            </DialogDescription>
          </div>
          <div className="grid gap-2">
            <Label htmlFor="group-name">Group name</Label>
            <Input
              id="group-name"
              value={name}
              autoComplete="off"
              autoFocus
              aria-invalid={problem !== null}
              aria-describedby={problem === null ? undefined : 'group-name-problem'}
              onChange={(event) => {
                setName(event.target.value)
                setProblem(null)
              }}
            />
            {problem === null ? null : (
              <p id="group-name-problem" className="text-sm text-destructive">
                {problem}
              </p>
            )}
          </div>
          <div className="flex flex-wrap justify-end gap-2">
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button type="submit">{renaming ? 'Rename' : 'Create group'}</Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
