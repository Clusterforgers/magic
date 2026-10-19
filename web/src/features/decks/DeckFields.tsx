import type { Ref } from 'react'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { DECK_FORMATS, formatLabel } from './format'

interface NameFieldProps {
  id: string
  value: string
  /** What is wrong with the name, shown under the field in words; null when nothing is. */
  problem: string | null
  disabled: boolean
  onChange: (value: string) => void
  inputRef?: Ref<HTMLInputElement>
  autoFocus?: boolean
}

/** The deck name: a labelled field whose problem is worded under it and tied to it for a screen reader. */
export function NameField({ id, value, problem, disabled, onChange, inputRef, autoFocus }: NameFieldProps) {
  return (
    <div className="grid gap-2">
      <Label htmlFor={id}>Deck name</Label>
      <Input
        ref={inputRef}
        id={id}
        value={value}
        autoComplete="off"
        autoFocus={autoFocus}
        readOnly={disabled}
        aria-invalid={problem !== null}
        aria-describedby={problem === null ? undefined : `${id}-problem`}
        onChange={(event) => onChange(event.target.value)}
      />
      {problem === null ? null : (
        <p id={`${id}-problem`} className="text-sm text-destructive">
          {problem}
        </p>
      )}
    </div>
  )
}

interface FormatFieldProps {
  id: string
  value: string
  disabled: boolean
  onChange: (value: string) => void
  /** What the choice changes, in a line under the field. */
  hint?: string
}

/** The deck format: every format the server checks, by name. */
export function FormatField({ id, value, disabled, onChange, hint }: FormatFieldProps) {
  return (
    <div className="grid gap-2">
      <Label htmlFor={id}>Format</Label>
      <NativeSelect
        id={id}
        value={value}
        disabled={disabled}
        aria-describedby={hint === undefined ? undefined : `${id}-hint`}
        className="w-full"
        onChange={(event) => onChange(event.target.value)}
      >
        {DECK_FORMATS.map((key) => (
          <NativeSelectOption key={key} value={key}>
            {formatLabel(key)}
          </NativeSelectOption>
        ))}
      </NativeSelect>
      {hint === undefined ? null : (
        <p id={`${id}-hint`} className="text-sm text-muted-foreground">
          {hint}
        </p>
      )}
    </div>
  )
}
