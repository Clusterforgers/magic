import { SearchIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { filtersActive, NO_FILTERS, STATUS_LABEL, STATUSES } from './format'
import type { Filters, PreconStatus, SetOption } from './format'

interface Props {
  filters: Filters
  sets: readonly SetOption[]
  onChange(next: Filters): void
}

const isStatus = (value: string): value is PreconStatus => STATUSES.some((status) => status === value)

/** Search by name, set or commander, and the set and deck-check filters above the list. */
export function PreconFilters({ filters, sets, onChange }: Props) {
  // A set that was chosen stays in the menu if the list no longer holds it, so the menu never shows blank.
  const options =
    filters.set === null || sets.some((option) => option.set === filters.set)
      ? sets
      : [...sets, { set: filters.set, count: 0 }]
  return (
    <div role="group" aria-label="Filter the precons" className="flex flex-wrap items-end gap-x-4 gap-y-3">
      <div className="grid w-full gap-1 sm:w-72">
        <Label htmlFor="precon-search" className="text-xs text-muted-foreground">
          Search
        </Label>
        <div className="relative">
          <SearchIcon
            aria-hidden="true"
            className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            id="precon-search"
            type="search"
            value={filters.text}
            placeholder="Name, set or commander"
            autoComplete="off"
            className="h-8 pl-9 text-sm pointer-coarse:h-11"
            onChange={(event) => onChange({ ...filters, text: event.target.value })}
          />
        </div>
      </div>
      <div className="grid gap-1">
        <Label htmlFor="precon-set" className="text-xs text-muted-foreground">
          Set
        </Label>
        <NativeSelect
          id="precon-set"
          size="sm"
          value={filters.set ?? ''}
          onChange={(event) =>
            onChange({ ...filters, set: event.target.value === '' ? null : event.target.value })
          }
        >
          <NativeSelectOption value="">Any set</NativeSelectOption>
          {options.map((option) => (
            <NativeSelectOption key={option.set} value={option.set}>
              {`${option.set} (${option.count})`}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      </div>
      <div className="grid gap-1">
        <Label htmlFor="precon-status" className="text-xs text-muted-foreground">
          Deck check
        </Label>
        <NativeSelect
          id="precon-status"
          size="sm"
          value={filters.status ?? ''}
          onChange={(event) =>
            onChange({ ...filters, status: isStatus(event.target.value) ? event.target.value : null })
          }
        >
          <NativeSelectOption value="">Any state</NativeSelectOption>
          {STATUSES.map((status) => (
            <NativeSelectOption key={status} value={status}>
              {STATUS_LABEL[status]}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      </div>
      {filtersActive(filters) ? (
        <Button type="button" variant="ghost" size="sm" onClick={() => onChange(NO_FILTERS)}>
          Clear filters
        </Button>
      ) : null}
    </div>
  )
}
