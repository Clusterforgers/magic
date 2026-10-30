import type { KeyboardEvent } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import type { AmountSpec } from './commands'
import { pressPreset } from './commands'

/** The step of Page Up and Page Down in an amount field. */
const PAGE_STEP = 5

interface Props {
  spec: AmountSpec
  value: number
  onChange: (value: number) => void
  /** The field's name, for a screen reader. */
  label: string
  /** Names a signed quick amount by its change alone ("Add 5"), for a form whose one field the group already names; else it starts with the field's name ("Change: add 5"). */
  plainPresets?: boolean
}

/** What a signed quick amount is called. */
function presetName(preset: number, label: string, plain: boolean): string {
  const change = `${Math.abs(preset)}`
  if (plain) return `${preset > 0 ? 'Add' : 'Take'} ${change}`
  return `${label}: ${preset > 0 ? 'add' : 'take'} ${change}`
}

/** The quick amounts and the field of one number; a signed spec's buttons add to the pending change, a count's set it. */
export function AmountField({ spec, value, onChange, label, plainPresets = false }: Props) {
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    const step = event.key === 'PageUp' ? PAGE_STEP : event.key === 'PageDown' ? -PAGE_STEP : 0
    if (step === 0) return
    event.preventDefault()
    onChange(Math.min(spec.max, Math.max(spec.min, (Number.isNaN(value) ? 0 : value) + step)))
  }
  return (
    <div className="flex flex-wrap items-center gap-1">
      {spec.presets.map((preset) => (
        <Button
          key={preset}
          variant="outline"
          size="sm"
          className="min-w-10 px-2 tabular-nums"
          aria-label={spec.signed ? presetName(preset, label, plainPresets) : `${preset}`}
          onClick={() => onChange(pressPreset(spec, value, preset))}
        >
          {spec.signed && preset > 0 ? `+${preset}` : preset}
        </Button>
      ))}
      <Input
        type="number"
        aria-label={label}
        // A numeric keypad has no minus sign, so a field that takes one gets the full keyboard.
        inputMode={spec.signed ? 'text' : 'numeric'}
        min={spec.min}
        max={spec.max}
        value={Number.isNaN(value) ? '' : value}
        onKeyDown={onKeyDown}
        onChange={(event) => onChange(event.currentTarget.valueAsNumber)}
        className="h-8 w-20 tabular-nums pointer-coarse:h-11"
      />
    </div>
  )
}
