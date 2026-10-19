import { useState } from 'react'
import type { FormEvent, Ref } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { isJoinCode, normalizeCode } from './format'

const PROBLEM_ID = 'join-code-problem'
const NOTE_ID = 'join-code-note'

/**
 * Said while the form cannot join: `POST /api/join` answers a seat and a token without the id of the game, so the page
 * has no game to open for the seat it would take.
 */
export const CODE_NOTE = 'Joining with a code is not available yet'

interface Props {
  /** Joins the game a code names, given the code as the server reads it; without it the button is disabled and the note says why. */
  onJoin?: (code: string) => void
  /** The field, so the empty list can send people to it. */
  inputRef?: Ref<HTMLInputElement>
}

/** "Join with a code": a field that takes the code as typed (any case, spaces and hyphens) and a button. */
export function JoinWithCode({ onJoin, inputRef }: Props) {
  const [typed, setTyped] = useState('')
  const [problem, setProblem] = useState<string | null>(null)

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    // Enter in the field submits even though the button is disabled (it stays focusable), so a page that cannot join stops here.
    if (onJoin === undefined) return
    const code = normalizeCode(typed)
    if (!isJoinCode(code)) {
      setProblem(
        code === '' ? 'Type the code the host gave you.' : 'A code is 8 letters and digits, like K7M2-9QXA.',
      )
      return
    }
    setProblem(null)
    onJoin(code)
  }

  return (
    <form
      aria-labelledby="join-code-label"
      noValidate
      onSubmit={submit}
      className="surface flex flex-wrap items-end gap-x-4 gap-y-3 p-4"
    >
      <div className="grid gap-2">
        <Label id="join-code-label" htmlFor="join-code">
          Join with a code
        </Label>
        <div className="flex gap-2">
          <Input
            ref={inputRef}
            id="join-code"
            name="code"
            value={typed}
            maxLength={24}
            autoComplete="off"
            autoCapitalize="characters"
            spellCheck={false}
            aria-invalid={problem === null ? undefined : true}
            aria-describedby={problem === null ? undefined : PROBLEM_ID}
            onChange={(event) => {
              setTyped(event.target.value)
              setProblem(null)
            }}
            className="w-48 font-mono tracking-wider uppercase"
          />
          <Button
            type="submit"
            variant="secondary"
            disabled={onJoin === undefined}
            focusableWhenDisabled
            aria-describedby={onJoin === undefined ? NOTE_ID : undefined}
          >
            Join
          </Button>
        </div>
      </div>
      {onJoin === undefined ? (
        <p id={NOTE_ID} className="pb-2 text-sm text-muted-foreground">
          {CODE_NOTE}
        </p>
      ) : null}
      {problem === null ? null : (
        <p id={PROBLEM_ID} role="alert" className="basis-full text-sm text-destructive">
          {problem}
        </p>
      )}
    </form>
  )
}
