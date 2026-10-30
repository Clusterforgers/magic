import type { ReactNode } from 'react'
import { Input } from '@/components/ui/input'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import type { CommonToken } from '@/protocol/generated'
import { COMMON_TOKENS } from '../offers'
import { CUSTOM, TOKEN_NAME_MAX, TOKEN_STAT_MAX } from './customToken'
import type { TokenDraft } from './customToken'

interface Props {
  draft: TokenDraft
  set: (next: Partial<TokenDraft>) => void
}

function Labelled({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="grid gap-0.5 text-xs text-muted-foreground">
      {label}
      {children}
    </label>
  )
}

const STAT_INPUT = 'h-8 tabular-nums pointer-coarse:h-11'

/** One of the creature's numbers; an emptied field is not a number, so the preview says what is wrong. */
function Stat({
  label,
  value,
  onChange,
}: {
  label: string
  value: number
  onChange: (value: number) => void
}) {
  return (
    <Labelled label={label}>
      <Input
        type="number"
        inputMode="numeric"
        min={0}
        max={TOKEN_STAT_MAX}
        value={Number.isNaN(value) ? '' : value}
        onChange={(event) => onChange(event.currentTarget.valueAsNumber)}
        className={STAT_INPUT}
      />
    </Labelled>
  )
}

/** The list of tokens, ending in "Custom creature…" which asks for the creature's name, power and toughness. */
export function TokenChoice({ draft, set }: Props) {
  return (
    <>
      <NativeSelect
        aria-label="Token"
        value={draft.token}
        onChange={(event) => set({ token: event.currentTarget.value as CommonToken | typeof CUSTOM })}
      >
        {COMMON_TOKENS.map((token) => (
          <NativeSelectOption key={token} value={token}>
            {token}
          </NativeSelectOption>
        ))}
        <NativeSelectOption value={CUSTOM}>Custom creature…</NativeSelectOption>
      </NativeSelect>
      {draft.token === CUSTOM ? (
        <div data-slot="custom-token" className="grid gap-2">
          <Labelled label="Name of the token">
            <Input
              // The field appears when "Custom creature…" is chosen, and the name is what comes next.
              autoFocus
              value={draft.name}
              maxLength={TOKEN_NAME_MAX}
              onChange={(event) => set({ name: event.currentTarget.value })}
              className={STAT_INPUT}
            />
          </Labelled>
          <div className="grid grid-cols-2 gap-2">
            <Stat label="Power" value={draft.power} onChange={(power) => set({ power })} />
            <Stat label="Toughness" value={draft.toughness} onChange={(toughness) => set({ toughness })} />
          </div>
        </div>
      ) : null}
    </>
  )
}
