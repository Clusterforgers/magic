import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { Switch } from '@/components/ui/switch'
import type { LobbyGame } from '@/protocol/generated'
import { filtersActive, formatName, formatOptions, NO_FILTERS, seatOptions } from './format'
import type { Filters } from './format'

interface Props {
  /** The whole listing, which decides what the choices are. */
  games: readonly LobbyGame[]
  filters: Filters
  onChange(next: Filters): void
}

// A choice that was made stays in the menu after the last game that had it has gone, so the menu never shows blank.
const withChoice = <T extends string | number>(options: T[], chosen: T | null): T[] =>
  chosen === null || options.includes(chosen) ? options : [...options, chosen]

/** Seats, format and "hide full" above a long list. */
export function GameFilters({ games, filters, onChange }: Props) {
  const seats = withChoice(seatOptions(games), filters.seats).sort((a, b) => a - b)
  const formats = withChoice(formatOptions(games), filters.format).sort()
  return (
    <div role="group" aria-label="Filter the open games" className="flex flex-wrap items-end gap-x-4 gap-y-3">
      <div className="grid gap-1">
        <Label htmlFor="filter-seats" className="text-xs text-muted-foreground">
          Seats
        </Label>
        <NativeSelect
          id="filter-seats"
          size="sm"
          value={filters.seats === null ? '' : String(filters.seats)}
          onChange={(event) =>
            onChange({ ...filters, seats: event.target.value === '' ? null : Number(event.target.value) })
          }
        >
          <NativeSelectOption value="">Any number</NativeSelectOption>
          {seats.map((count) => (
            <NativeSelectOption key={count} value={String(count)}>
              {count} seats
            </NativeSelectOption>
          ))}
        </NativeSelect>
      </div>
      <div className="grid gap-1">
        <Label htmlFor="filter-format" className="text-xs text-muted-foreground">
          Format
        </Label>
        <NativeSelect
          id="filter-format"
          size="sm"
          value={filters.format ?? ''}
          onChange={(event) =>
            onChange({ ...filters, format: event.target.value === '' ? null : event.target.value })
          }
        >
          <NativeSelectOption value="">Any format</NativeSelectOption>
          {formats.map((format) => (
            <NativeSelectOption key={format} value={format}>
              {formatName(format)}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      </div>
      <div className="flex h-8 items-center gap-2 pointer-coarse:h-11">
        <Switch
          id="filter-hide-full"
          aria-labelledby="filter-hide-full-label"
          checked={filters.hideFull}
          onCheckedChange={(hideFull) => onChange({ ...filters, hideFull })}
        />
        <Label id="filter-hide-full-label" htmlFor="filter-hide-full">
          Hide full games
        </Label>
      </div>
      {filtersActive(filters) ? (
        <Button type="button" variant="ghost" size="sm" onClick={() => onChange(NO_FILTERS)}>
          Clear filters
        </Button>
      ) : null}
    </div>
  )
}
