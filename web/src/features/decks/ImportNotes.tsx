import { InfoIcon, TriangleAlertIcon } from 'lucide-react'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import type { ImportResult } from '@/protocol/generated'
import { cardsText } from './format'

const SUGGESTED = 3

const lines = (count: number): string => `${count} ${count === 1 ? 'line' : 'lines'}`

/**
 * What an import left out, in words: the lines no card was found for (with the names that might have been meant, never
 * applied), the lines that were no card entries, and the parts of the list that are not deck cards, such as the
 * maybeboard. Nothing is drawn for a clean list.
 */
export function ImportNotes({ result }: { result: ImportResult }) {
  const { unresolved, problems, ignored } = result
  return (
    <>
      {unresolved.length === 0 ? null : (
        <Alert variant="warning" role="status" data-note="unresolved">
          <TriangleAlertIcon aria-hidden="true" />
          <AlertTitle>
            {unresolved.length === 1
              ? '1 line was not found and is left out of the deck'
              : `${unresolved.length} lines were not found and are left out of the deck`}
          </AlertTitle>
          <AlertDescription>
            <ul className="grid max-h-48 gap-1 overflow-y-auto">
              {unresolved.map((line, index) => (
                <li key={`${line.zone}:${line.name}:${index}`}>
                  <span className="font-medium text-foreground">
                    {line.count} {line.name}
                  </span>
                  {`: ${line.message}`}
                  {line.suggestions.length === 0
                    ? null
                    : ` Did you mean ${line.suggestions
                        .slice(0, SUGGESTED)
                        .map((suggestion) => suggestion.name)
                        .join(', ')}?`}
                </li>
              ))}
            </ul>
          </AlertDescription>
        </Alert>
      )}
      {problems.length === 0 ? null : (
        <Alert variant="warning" role="status" data-note="problems">
          <TriangleAlertIcon aria-hidden="true" />
          <AlertTitle>
            {lines(problems.length)} {problems.length === 1 ? 'is' : 'are'} no card{' '}
            {problems.length === 1 ? 'entry' : 'entries'}
          </AlertTitle>
          <AlertDescription>
            <ul className="grid max-h-48 gap-1 overflow-y-auto">
              {problems.map((problem) => (
                <li key={problem.line}>{`Line ${problem.line}: ${problem.message}`}</li>
              ))}
            </ul>
          </AlertDescription>
        </Alert>
      )}
      {ignored.length === 0 ? null : (
        <Alert role="status" data-note="ignored">
          <InfoIcon aria-hidden="true" />
          <AlertTitle>Left out of the deck</AlertTitle>
          <AlertDescription>
            <ul className="grid gap-1">
              {ignored.map((part) => (
                <li key={part.name}>
                  <span className="font-medium text-foreground">{part.name}</span>
                  {` (${cardsText(part.count)}) is not part of the deck.`}
                </li>
              ))}
            </ul>
          </AlertDescription>
        </Alert>
      )}
    </>
  )
}
