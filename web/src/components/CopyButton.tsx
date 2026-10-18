import { Check, Copy } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { copyText } from '@/lib/clipboard'

type CopyState = 'idle' | 'copied' | 'failed'

const FEEDBACK_MS = 2500

interface CopyButtonProps {
  /** What lands on the clipboard. */
  text: string
  /** The accessible name; it must start with the visible words so voice control can say them. */
  label: string
  /** The visible words; "Copy" by default. */
  children?: ReactNode
}

/** A button that copies `text` and says so, aloud as well as on screen; on refusal it asks for a copy by hand. */
export function CopyButton({ text, label, children = 'Copy' }: CopyButtonProps) {
  const [state, setState] = useState<CopyState>('idle')
  const timer = useRef<number | null>(null)

  useEffect(
    () => () => {
      if (timer.current !== null) window.clearTimeout(timer.current)
    },
    [],
  )

  async function copy(): Promise<void> {
    const copied = await copyText(text)
    setState(copied ? 'copied' : 'failed')
    if (timer.current !== null) window.clearTimeout(timer.current)
    timer.current = window.setTimeout(() => setState('idle'), FEEDBACK_MS)
  }

  return (
    <>
      <Button type="button" variant="outline" size="sm" aria-label={label} onClick={() => void copy()}>
        {state === 'copied' ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
        {state === 'copied' ? 'Copied' : state === 'failed' ? 'Copy failed' : children}
      </Button>
      <span role="status" className="sr-only">
        {state === 'copied' ? 'Copied to the clipboard.' : null}
        {state === 'failed' ? 'The browser would not copy. Select the link and copy it by hand.' : null}
      </span>
    </>
  )
}
