import { useCallback, useMemo, useRef, useState } from 'react'
import type { RefObject } from 'react'

/** What a form holds of its password field: the input to read at submit, and whether its text shows. */
export interface PasswordControl {
  ref: RefObject<HTMLInputElement | null>
  revealed: boolean
  setRevealed: (revealed: boolean) => void
  /** What is in the field now, exactly as typed. */
  read: () => string
  /** Empties the field and hides its text again. */
  clear: () => void
  /** Puts the cursor in the field. */
  focus: () => void
}

/** The state behind a `PasswordField`; the field is uncontrolled, so a typed password lives only in its input. */
export function usePasswordControl(): PasswordControl {
  const ref = useRef<HTMLInputElement>(null)
  const [revealed, setRevealed] = useState(false)
  const read = useCallback(() => ref.current?.value ?? '', [])
  const clear = useCallback(() => {
    if (ref.current !== null) ref.current.value = ''
    setRevealed(false)
  }, [])
  const focus = useCallback(() => ref.current?.focus(), [])
  // Stable between renders, so an effect can depend on `focus` and run once.
  return useMemo(() => ({ ref, revealed, setRevealed, read, clear, focus }), [revealed, read, clear, focus])
}
