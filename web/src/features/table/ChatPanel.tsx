import { useId, useLayoutEffect, useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { useGame, usePlay } from '@/features/game/gameContext'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { MAX_CHAT_CHARS } from '@/play/session'
import { useSeatNamer } from './names'

/** Table chat: the lines so far and a box to say something, sent with `session.chat`. */
export function ChatPanel() {
  const { session } = useGame()
  const chat = usePlay((state) => state.chat)
  const me = usePlay((state) => state.seat)
  const name = useSeatNamer()
  const [text, setText] = useState('')
  const inputId = useId()
  const scroller = useRef<HTMLDivElement>(null)
  useLayoutEffect(() => {
    const element = scroller.current
    if (element !== null) element.scrollTop = element.scrollHeight
  }, [chat.length])

  function send(event: FormEvent): void {
    event.preventDefault()
    if (text.trim() === '') return
    // Keep the text when it was not sent, so a dropped connection loses nothing.
    if (session.chat(text)) setText('')
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <div
        ref={scroller}
        role="log"
        aria-label="Chat"
        aria-live="polite"
        tabIndex={0}
        className="min-h-0 flex-1 overflow-y-auto rounded-md px-1 text-sm outline-primary focus-visible:outline-2"
      >
        {chat.length === 0 ? (
          <p className="p-2 text-muted-foreground">No messages yet.</p>
        ) : (
          <ol className="grid gap-1">
            {chat.map((line, index) => (
              <li key={index} className="px-1 break-words">
                <span className="font-semibold">{line.from === me ? 'You' : name(line.from)}</span>:{' '}
                {line.text}
              </li>
            ))}
          </ol>
        )}
      </div>
      <form onSubmit={send} className="flex gap-2">
        <label htmlFor={inputId} className="sr-only">
          Message to the table
        </label>
        <Input
          id={inputId}
          value={text}
          maxLength={MAX_CHAT_CHARS}
          autoComplete="off"
          placeholder="Say something to the table"
          onChange={(event) => setText(event.target.value)}
        />
        <Button type="submit" disabled={text.trim() === ''}>
          Send
        </Button>
      </form>
    </div>
  )
}
