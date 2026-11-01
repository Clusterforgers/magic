import { BookOpenIcon } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { searchTerms } from '@/lib/glossary'

/** The searchable list of every keyword, keyword action and ability word with its reminder text, as a button in the top bar. */
export function GlossaryPanel() {
  const [query, setQuery] = useState('')
  const found = searchTerms(query)
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            data-slot="glossary"
            variant="ghost"
            size="icon"
            title="Keywords"
            aria-label="Keyword glossary"
          />
        }
      >
        <BookOpenIcon aria-hidden="true" />
      </PopoverTrigger>
      <PopoverContent align="end" className="w-96 gap-2 p-3">
        <PopoverTitle>Keywords</PopoverTitle>
        <Input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search keywords"
          aria-label="Search keywords"
        />
        {found.length === 0 ? (
          <p className="text-sm text-muted-foreground">Nothing matches.</p>
        ) : (
          <ul aria-label="Keyword reminders" className="grid max-h-96 gap-2 overflow-y-auto text-sm">
            {found.map((term) => (
              <li key={term.title}>
                <span className="font-medium">{term.title}</span>
                <span className="text-xs text-muted-foreground"> ({term.kind})</span>
                <p>{term.text}</p>
              </li>
            ))}
          </ul>
        )}
      </PopoverContent>
    </Popover>
  )
}
