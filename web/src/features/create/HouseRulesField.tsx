import { ChevronRight } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { describeRules, requestRules, ruleGroups } from '@/lib/houseRules'
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
  return (
    <div className="grid gap-1" data-rule={spec.id}>
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
  const changed = describeRules(requestRules(draft.rules, format), format)
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
      <div className="grid gap-5 border-t px-4 py-4">
        <p className="text-sm text-muted-foreground">
          Leave a field empty to play by the format’s rule. Only what you fill in changes the game, and every
          player sees it.
        </p>
        {ruleGroups(format).map((group) => (
          <fieldset key={group.title} className="grid gap-4">
            <legend className="mb-2 text-sm font-semibold">{group.title}</legend>
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
          </fieldset>
        ))}
      </div>
    </details>
  )
}
