import { useId } from 'react'
import { formatLabel } from '@/features/decks/format'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { cn } from '@/lib/utils'
import { FILTER_FORMATS, IDENTITY_LETTERS, IDENTITY_NAME, manaValueProblem, toggleLetter } from './search'
import type { SearchFilters } from './search'

// Whole class names, so Tailwind can see them: the fill and the ink of each identity.
const DISC: Readonly<Record<string, string>> = {
  W: 'bg-mana-white text-on-mana-white',
  U: 'bg-mana-blue text-on-mana-blue',
  B: 'bg-mana-black text-on-mana-black',
  R: 'bg-mana-red text-on-mana-red',
  G: 'bg-mana-green text-on-mana-green',
  C: 'bg-mana-colorless text-on-mana-colorless',
}

interface FilterBarProps {
  filters: SearchFilters
  onChange: (next: SearchFilters) => void
}

/**
 * The filters under the search box: colour identity as six toggles, the type line, the mana value and the format. A
 * toggle that is on has a ring as well as `aria-pressed`, so colour is never the only sign.
 */
export function FilterBar({ filters, onChange }: FilterBarProps) {
  const id = useId()
  const problem = manaValueProblem(filters.manaValue)
  return (
    <div className="grid gap-2">
      <div role="group" aria-label="Colour identity" className="flex flex-wrap items-center gap-1">
        <span aria-hidden="true" className="mr-1 text-xs text-muted-foreground">
          Colour identity
        </span>
        {IDENTITY_LETTERS.map((letter) => {
          const on = filters.identity.includes(letter)
          return (
            <button
              key={letter}
              type="button"
              aria-pressed={on}
              aria-label={IDENTITY_NAME[letter] ?? letter}
              onClick={() => onChange({ ...filters, identity: toggleLetter(filters.identity, letter) })}
              className="inline-flex size-8 items-center justify-center rounded-full pointer-coarse:size-11"
            >
              <span
                aria-hidden="true"
                className={cn(
                  'flex size-6 items-center justify-center rounded-full text-xs font-semibold transition-shadow duration-fast ease-out',
                  DISC[letter],
                  on ? 'ring-2 ring-ring ring-offset-2 ring-offset-surface' : 'ring-1 ring-foreground/25',
                )}
              >
                {letter}
              </span>
            </button>
          )
        })}
      </div>
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-[3fr_2fr_3fr]">
        <div className="col-span-2 grid gap-1 sm:col-span-1">
          <Label htmlFor={`${id}-type`}>Type</Label>
          <Input
            id={`${id}-type`}
            value={filters.type}
            maxLength={60}
            autoComplete="off"
            spellCheck={false}
            placeholder="creature, elf"
            onChange={(event) => onChange({ ...filters, type: event.target.value })}
          />
        </div>
        <div className="grid gap-1">
          <Label htmlFor={`${id}-value`}>Mana value</Label>
          <Input
            id={`${id}-value`}
            value={filters.manaValue}
            maxLength={5}
            autoComplete="off"
            placeholder="3, 2-4, 5+"
            aria-invalid={problem !== null}
            aria-describedby={problem === null ? undefined : `${id}-value-problem`}
            onChange={(event) => onChange({ ...filters, manaValue: event.target.value })}
          />
        </div>
        <div className="grid gap-1">
          <Label htmlFor={`${id}-format`}>Format</Label>
          <NativeSelect
            id={`${id}-format`}
            value={filters.format}
            className="w-full"
            onChange={(event) => onChange({ ...filters, format: event.target.value })}
          >
            <NativeSelectOption value="">Any format</NativeSelectOption>
            {FILTER_FORMATS.map((key) => (
              <NativeSelectOption key={key} value={key}>
                {formatLabel(key)}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </div>
      </div>
      {problem === null ? null : (
        <p id={`${id}-value-problem`} className="text-sm text-destructive">
          {problem}
        </p>
      )}
    </div>
  )
}
