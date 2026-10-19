import { useRef, useState } from 'react'
import type { FormEvent, Ref } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { isJoinCode, normalizeCode } from './format'
import { CODE_SHAPE } from './joinErrors'
import type { OnJoinCode } from './useJoinGame'

const PROBLEM_ID = 'join-code-problem'

interface Props {
  /** Joins the game a code names, given the code as the server reads it and the button that asked. */
  onJoin: OnJoinCode
  /** A request with this form's code is on its way: the button spins and the code cannot change. */
  pending?: boolean
  /** Some request is on its way, a row's too: the button waits, as every Join does. */
  busy?: boolean
  /** Why the last join failed. It stays under the field, beside the code, until the code changes or a new try starts. */
  failure?: string | null
  /** The typed code changed, so a failure no longer describes it. */
  onEdit?: () => void
  /** The field, so the empty list can send people to it. */
  inputRef?: Ref<HTMLInputElement>
}

/** "Join with a code": a field that takes the code as typed (any case, spaces and hyphens) and a button. */
export function JoinWithCode({
  onJoin,
  pending = false,
  busy = false,
  failure = null,
  onEdit,
  inputRef,
}: Props) {
  const [typed, setTyped] = useState('')
  const [problem, setProblem] = useState<string | null>(null)
  const button = useRef<HTMLButtonElement>(null)
  // What the player can fix comes first; a failure of the last try is only shown while the code is as it was sent.
  const shown = problem ?? failure

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    // Enter in the field submits even while the button waits (it stays focusable), so a request that is out stops here.
    if (pending || busy) return
    const code = normalizeCode(typed)
    if (!isJoinCode(code)) {
      setProblem(code === '' ? 'Type the code the host gave you.' : CODE_SHAPE)
      return
    }
    setProblem(null)
    onJoin(code, button.current)
  }

  return (
    <form
      aria-labelledby="join-code-label"
      noValidate
      aria-busy={pending || undefined}
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
            readOnly={pending}
            autoComplete="off"
            autoCapitalize="characters"
            spellCheck={false}
            aria-invalid={problem === null ? undefined : true}
            aria-describedby={shown === null ? undefined : PROBLEM_ID}
            onChange={(event) => {
              setTyped(event.target.value)
              setProblem(null)
              onEdit?.()
            }}
            className="w-48 font-mono tracking-wider uppercase"
          />
          <Button
            ref={button}
            type="submit"
            variant="secondary"
            disabled={busy}
            loading={pending}
            focusableWhenDisabled
          >
            Join
          </Button>
        </div>
      </div>
      {shown === null ? null : (
        <p id={PROBLEM_ID} role="alert" className="basis-full text-sm text-destructive">
          {shown}
        </p>
      )}
    </form>
  )
}
