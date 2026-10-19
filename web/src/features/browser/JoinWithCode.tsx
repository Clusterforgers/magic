import { useState } from 'react'
import type { FormEvent, Ref } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { isJoinCode, normalizeCode } from './format'
import { JOIN_NOTE, JOIN_NOTE_ID } from './joinTarget'
import type { OnJoin } from './joinTarget'

const PROBLEM_ID = 'join-code-problem'

interface Props {
  /** Joins the game a code names; without it the button is disabled and the note says why. */
  onJoin: OnJoin | undefined
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
    onJoin?.({ kind: 'code', code })
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
            aria-describedby={onJoin === undefined ? JOIN_NOTE_ID : undefined}
          >
            Join
          </Button>
        </div>
      </div>
      {onJoin === undefined ? (
        <p id={JOIN_NOTE_ID} className="pb-2 text-sm text-muted-foreground">
          {JOIN_NOTE}
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
