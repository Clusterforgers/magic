import { ChevronRight } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { Switch } from '@/components/ui/switch'
import { describeRules, HANDICAP_FIELDS, handicapId, requestRules, ruleGroups } from '@/lib/houseRules'
import type { GameFormat, RuleSpec } from '@/lib/houseRules'
import type { Draft } from './form'

interface RuleRowProps {
  spec: RuleSpec
  format: GameFormat
  text: string
  problem: string | null
  onChange(id: string, text: string): void
}

function RuleRow({ spec, format, text, problem, onChange }: RuleRowProps) {
  const id = `rule-${spec.id.replace('.', '-')}`
  const own = spec.fallback(format)
  const describedBy = `${id}-help${problem === null ? '' : ` ${id}-problem`}`
  if (spec.input.kind === 'switch') {
    return (
      <div className="grid content-start gap-1" data-rule={spec.id}>
        <div className="flex items-center gap-2">
          <Switch
            id={id}
            aria-labelledby={`${id}-label`}
            aria-describedby={`${id}-help`}
            checked={text.trim() !== 'off'}
            onCheckedChange={(on) => onChange(spec.id, on ? '' : 'off')}
          />
          <Label id={`${id}-label`} htmlFor={id}>
            {spec.label}
          </Label>
        </div>
        <p id={`${id}-help`} className="text-xs text-muted-foreground">
          {spec.help}
        </p>
      </div>
    )
  }
  return (
    <div className="grid content-start gap-1" data-rule={spec.id}>
      <Label htmlFor={id}>{spec.label}</Label>
      {spec.input.kind === 'choice' ? (
        <NativeSelect
          id={id}
          className="w-full"
          value={text}
          aria-describedby={describedBy}
          aria-invalid={problem !== null}
          onChange={(event) => onChange(spec.id, event.target.value)}
        >
          <NativeSelectOption value="">
            {own === '' ? 'Format’s rule' : `Format’s rule (${own})`}
          </NativeSelectOption>
          {spec.input.options.map((option) => (
            <NativeSelectOption key={option.value} value={option.value}>
              {option.label}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      ) : (
        <Input
          id={id}
          inputMode={spec.input.word === undefined ? 'numeric' : 'text'}
          autoComplete="off"
          value={text}
          placeholder={own === '' ? 'Not set' : own}
          aria-describedby={describedBy}
          aria-invalid={problem !== null}
          onChange={(event) => onChange(spec.id, event.target.value)}
        />
      )}
      <p id={`${id}-help`} className="text-xs text-muted-foreground">
        {spec.help}
      </p>
      {problem === null ? null : (
        <p id={`${id}-problem`} className="text-sm text-destructive">
          {problem}
        </p>
      )}
    </div>
  )
}

interface HandicapsProps {
  draft: Draft
  problems: Readonly<Record<string, string>> | null
  onChange(id: string, text: string): void
}

/** The name a seat goes by in the handicaps: its number, and the name the form gives it when the form names it. */
function seatTitle(draft: Draft, index: number): string {
  const name = draft.seats[index]?.name.trim() ?? ''
  const named = draft.links || index === 0
  return named && name !== '' ? `Seat ${index + 1}, ${name}` : `Seat ${index + 1}`
}

/** A row per seat with the life and opening hand it starts with in place of the table's; empty keeps the table's. */
function Handicaps({ draft, problems, onChange }: HandicapsProps) {
  return (
    <fieldset data-slot="handicaps">
      <legend className="mb-1 text-sm font-semibold">Handicaps</legend>
      <p className="mb-3 text-xs text-muted-foreground">
        What one seat starts with in place of the table’s rule. Leave a field empty to keep the table’s.
      </p>
      <div className="grid gap-3">
        {draft.seats.map((seat, index) => (
          <div key={seat.id} className="grid gap-1" data-handicap-seat={index}>
            <p className="text-sm font-medium">{seatTitle(draft, index)}</p>
            <div className="grid grid-cols-2 gap-x-4 gap-y-1">
              {HANDICAP_FIELDS.map((field) => {
                const id = handicapId(index, field.key)
                const dom = `rule-${id.replaceAll('.', '-')}`
                const problem = problems?.[id] ?? null
                return (
                  <div key={field.key} className="grid content-start gap-1">
                    {/* The input's own name says whose it is; this is the column's word for the eye. */}
                    <span aria-hidden="true" className="text-xs font-medium">
                      {field.label}
                    </span>
                    <Input
                      id={dom}
                      inputMode="numeric"
                      autoComplete="off"
                      value={draft.rules[id] ?? ''}
                      placeholder="Table’s"
                      aria-label={`${field.label} of ${seatTitle(draft, index)}`}
                      aria-describedby={problem === null ? undefined : `${dom}-problem`}
                      aria-invalid={problem !== null}
                      onChange={(event) => onChange(id, event.target.value)}
                    />
                    {problem === null ? null : (
                      <p id={`${dom}-problem`} className="text-sm text-destructive">
                        {problem}
                      </p>
                    )}
                  </div>
                )
              })}
            </div>
          </div>
        ))}
      </div>
    </fieldset>
  )
}

interface HouseRulesFieldProps {
  draft: Draft
  /** What is wrong with each rule's text, by rule id, or null to show nothing yet. */
  problems: Readonly<Record<string, string>> | null
  onChange(id: string, text: string): void
}

/**
 * The house rules behind a disclosure. Every field starts empty and shows the format's own value as its placeholder, so
 * a game with nothing filled in plays by the format's rules; the summary says how many differ.
 */
export function HouseRulesField({ draft, problems, onChange }: HouseRulesFieldProps) {
  const { format } = draft
  const details = useRef<HTMLDetailsElement>(null)
  const troubled = problems !== null && Object.keys(problems).length > 0
  // A rule to fix is inside the disclosure: open it so the form can focus the field.
  useEffect(() => {
    if (troubled && details.current !== null) details.current.open = true
  }, [troubled])
  const changed = describeRules(requestRules(draft.rules, format, draft.seats.length), format)
  const summary =
    changed.length === 0
      ? `${format === 'commander' ? 'Commander' : 'Normal'} rules`
      : `${changed.length} ${changed.length === 1 ? 'rule differs' : 'rules differ'}`
  return (
    <details ref={details} className="group surface" data-slot="house-rules">
      <summary className="flex cursor-pointer list-none items-center gap-2 px-4 py-3 text-sm font-medium marker:hidden pointer-coarse:py-4 [&::-webkit-details-marker]:hidden">
        <ChevronRight
          aria-hidden="true"
          className="size-4 transition-transform duration-fast ease-out group-open:rotate-90"
        />
        House rules
        <span data-slot="house-rules-summary" className="ml-auto text-xs font-normal text-muted-foreground">
          {summary}
        </span>
      </summary>
      <div className="grid gap-6 border-t px-4 py-4">
        <p className="text-sm text-muted-foreground">
          Leave a field empty to play by the format’s rule. Only what you fill in changes the game, and every
          player sees it.
        </p>
        {ruleGroups(format).map((group) => (
          <fieldset key={group.title}>
            <legend className="mb-3 text-sm font-semibold">{group.title}</legend>
            <div className="grid content-start gap-x-6 gap-y-4 sm:grid-cols-2 md:grid-cols-1 lg:grid-cols-2">
              {group.specs.map((spec) => (
                <RuleRow
                  key={spec.id}
                  spec={spec}
                  format={format}
                  text={draft.rules[spec.id] ?? ''}
                  problem={problems?.[spec.id] ?? null}
                  onChange={onChange}
                />
              ))}
            </div>
          </fieldset>
        ))}
        <Handicaps draft={draft} problems={problems} onChange={onChange} />
      </div>
    </details>
  )
}
