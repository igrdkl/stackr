import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react'
import { Check, ChevronDown, ChevronUp, RotateCcw, Save, Search, X } from 'lucide-react'
import { ModalBackdrop } from './ui/ModalBackdrop'
import { Spinner } from './ui/Spinner'
import { useStore } from '../store/useStore'
import { primaryBtn } from '../lib/styles'

export function ConfigEditorModal() {
  const cfg = useStore((s) => s.cfg)
  const closeConfig = useStore((s) => s.closeConfig)
  const setConfigContent = useStore((s) => s.setConfigContent)
  const saveConfig = useStore((s) => s.saveConfig)
  const resetConfig = useStore((s) => s.resetConfig)
  const askConfirm = useStore((s) => s.askConfirm)

  const taRef = useRef<HTMLTextAreaElement>(null)
  const findRef = useRef<HTMLInputElement>(null)
  const backdropRef = useRef<HTMLDivElement>(null)
  const dirty = cfg.content !== cfg.original

  // Always-visible in-editor find (the textarea has no native search, and browser
  // Ctrl+F is disabled in release). Case-insensitive; the current match is shown
  // via the textarea's selection while the find box keeps focus.
  const [find, setFind] = useState('')
  const [matchIdx, setMatchIdx] = useState(0)
  const [matchCount, setMatchCount] = useState(0)

  // Render the content with every match wrapped in <mark> for the highlight
  // backdrop (a textarea can't style its own text). The current match is
  // emphasized. <mark> must add no padding/border so it stays pixel-aligned
  // with the (transparent) textarea on top.
  const highlighted = useMemo<ReactNode[]>(() => {
    const text = cfg.content
    if (!find) return [text]
    const q = find.toLowerCase()
    const hay = text.toLowerCase()
    const nodes: ReactNode[] = []
    let last = 0
    let k = 0
    for (let i = hay.indexOf(q); i !== -1; i = hay.indexOf(q, i + q.length)) {
      if (i > last) nodes.push(text.slice(last, i))
      nodes.push(
        <mark
          key={k}
          style={{
            padding: 0,
            borderRadius: 2,
            color: k === matchIdx ? '#fff' : 'inherit',
            background: k === matchIdx ? 'rgba(79,127,255,.7)' : 'rgba(255,214,102,.28)',
          }}
        >
          {text.slice(i, i + find.length)}
        </mark>,
      )
      last = i + find.length
      k++
    }
    nodes.push(text.slice(last))
    return nodes
  }, [cfg.content, find, matchIdx])

  // Focus the editor when it opens.
  useEffect(() => {
    if (cfg.open && !cfg.loading) taRef.current?.focus()
  }, [cfg.open, cfg.loading])

  // Reset find state when the editor closes or switches to another config.
  useEffect(() => {
    setFind('')
    setMatchCount(0)
    setMatchIdx(0)
  }, [cfg.path])

  /** All (case-insensitive) match start offsets for `q` in the current content. */
  const collect = (q: string): number[] => {
    const ta = taRef.current
    if (!ta || !q) return []
    const hay = ta.value.toLowerCase()
    const needle = q.toLowerCase()
    const out: number[] = []
    for (let i = hay.indexOf(needle); i !== -1; i = hay.indexOf(needle, i + 1)) out.push(i)
    return out
  }

  /** Keep the highlight backdrop scrolled in lockstep with the textarea. */
  const syncScroll = () => {
    const ta = taRef.current
    const bd = backdropRef.current
    if (ta && bd) {
      bd.scrollTop = ta.scrollTop
      bd.scrollLeft = ta.scrollLeft
    }
  }

  /** Select + scroll the textarea to a match (the <mark> backdrop shows the
   *  highlight; the native selection is a secondary cue). Keeps find-box focus. */
  const select = (start: number, len: number) => {
    const ta = taRef.current
    if (!ta) return
    ta.setSelectionRange(start, start + len)
    const line = ta.value.slice(0, start).split('\n').length - 1
    const lineH = 12.5 * 1.7 // font-size × line-height (see textarea classes)
    ta.scrollTop = Math.max(0, line * lineH - ta.clientHeight / 2)
    syncScroll()
  }

  const applyFind = (q: string, idx: number) => {
    const idxs = collect(q)
    setMatchCount(idxs.length)
    if (!idxs.length) {
      setMatchIdx(0)
      return
    }
    const n = ((idx % idxs.length) + idxs.length) % idxs.length
    setMatchIdx(n)
    select(idxs[n], q.length)
  }

  const onFindKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      applyFind(find, matchIdx + (e.shiftKey ? -1 : 1))
    } else if (e.key === 'Escape') {
      e.preventDefault()
      taRef.current?.focus()
    }
  }

  if (!cfg.open) return null

  // Guard against losing unsaved edits on a stray backdrop/Cancel click.
  const tryClose = async () => {
    if (dirty) {
      const ok = await askConfirm({
        title: 'Discard changes?',
        message: `You have unsaved changes to ${cfg.label}. Close without saving?`,
        confirmLabel: 'Discard',
        danger: true,
      })
      if (!ok) return
    }
    closeConfig()
  }

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
      e.preventDefault()
      if (dirty) void saveConfig()
      return
    }
    if (e.key === 'Tab') {
      e.preventDefault()
      const ta = e.currentTarget
      const { selectionStart: a, selectionEnd: b, value } = ta
      const next = value.slice(0, a) + '  ' + value.slice(b)
      setConfigContent(next)
      requestAnimationFrame(() => {
        ta.selectionStart = ta.selectionEnd = a + 2
      })
    }
  }

  return (
    <ModalBackdrop onClose={tryClose} padded>
      <div
        className="w-[820px] max-w-[92vw] bg-card border border-line-input rounded-xl overflow-hidden flex flex-col"
        style={{ boxShadow: '0 24px 60px rgba(0,0,0,.5)', maxHeight: '88vh' }}
      >
        {/* header */}
        <div className="px-5 py-[15px] border-b border-[#1f242f] flex items-center justify-between gap-4 shrink-0">
          <div className="min-w-0">
            <div className="text-[14.5px] font-semibold">{cfg.label || 'Configuration'}</div>
            <div className="font-mono text-[11px] text-fg-dim mt-[2px] truncate">{cfg.path}</div>
          </div>
          <button
            onClick={tryClose}
            className="w-7 h-7 shrink-0 rounded-md bg-transparent border-none text-[#7b818f] flex items-center justify-center cursor-pointer transition-colors hover:bg-hover hover:text-[#cfd4de]"
          >
            <X size={16} strokeWidth={2} />
          </button>
        </div>

        {/* body */}
        <div className="p-4 flex-1 min-h-0 flex flex-col">
          {cfg.loading ? (
            <div className="flex-1 min-h-[360px] flex items-center justify-center text-fg-dim text-[13px] gap-[9px]">
              <Spinner size={15} strokeWidth={2.4} />
              Loading…
            </div>
          ) : (
            <>
              {/* always-visible search toolbar */}
              <div className="mb-3 flex items-center gap-2 shrink-0">
                <div className="relative flex-1">
                  <Search
                    size={13}
                    className="absolute left-[10px] top-1/2 -translate-y-1/2 text-fg-dim pointer-events-none"
                  />
                  <input
                    ref={findRef}
                    value={find}
                    onChange={(e) => {
                      setFind(e.target.value)
                      applyFind(e.target.value, 0)
                    }}
                    onKeyDown={onFindKey}
                    placeholder="Search in file…"
                    spellCheck={false}
                    className="w-full bg-term border border-line-subtle rounded-md pl-[30px] pr-2 py-[7px] text-[12.5px] text-[#cdd2dc] placeholder:text-fg-dim outline-none focus:border-accent"
                  />
                </div>
                <span className="text-[11.5px] text-fg-dim tabular-nums text-center min-w-[52px] select-none">
                  {find ? `${matchCount ? matchIdx + 1 : 0}/${matchCount}` : ''}
                </span>
                <button
                  onClick={() => applyFind(find, matchIdx - 1)}
                  disabled={!matchCount}
                  title="Previous match (Shift+Enter)"
                  className="w-8 h-8 flex items-center justify-center rounded-md border border-line-ghost text-fg-dim transition-colors hover:bg-hover hover:text-[#cfd4de] disabled:opacity-40 disabled:cursor-default"
                >
                  <ChevronUp size={15} strokeWidth={2} />
                </button>
                <button
                  onClick={() => applyFind(find, matchIdx + 1)}
                  disabled={!matchCount}
                  title="Next match (Enter)"
                  className="w-8 h-8 flex items-center justify-center rounded-md border border-line-ghost text-fg-dim transition-colors hover:bg-hover hover:text-[#cfd4de] disabled:opacity-40 disabled:cursor-default"
                >
                  <ChevronDown size={15} strokeWidth={2} />
                </button>
              </div>
              {/* Highlight backdrop (behind) + transparent textarea (on top).
                  Both share identical text metrics + padding so matches line up. */}
              <div className="relative flex-1 min-h-[360px] bg-term border border-line-subtle rounded-[10px] overflow-hidden focus-within:border-accent">
                <div
                  ref={backdropRef}
                  aria-hidden
                  className="absolute inset-0 overflow-hidden px-4 py-[14px] font-mono text-[12.5px] leading-[1.7] text-[#cdd2dc] pointer-events-none"
                  style={{ tabSize: 2, whiteSpace: 'pre', overflowWrap: 'normal' }}
                >
                  {highlighted}
                </div>
                <textarea
                  ref={taRef}
                  value={cfg.content}
                  onChange={(e) => setConfigContent(e.target.value)}
                  onKeyDown={onKeyDown}
                  onScroll={syncScroll}
                  spellCheck={false}
                  autoCapitalize="off"
                  autoCorrect="off"
                  className="absolute inset-0 w-full h-full resize-none bg-transparent px-4 py-[14px] font-mono text-[12.5px] leading-[1.7] outline-none"
                  style={{ tabSize: 2, whiteSpace: 'pre', overflowWrap: 'normal', color: 'transparent', caretColor: '#cdd2dc' }}
                />
              </div>
            </>
          )}

          {cfg.error && (
            <div className="mt-[10px] px-[13px] py-[10px] bg-[rgba(248,81,73,.1)] border border-[rgba(248,81,73,.32)] rounded-lg text-[12px] text-danger font-mono break-words">
              {cfg.error}
            </div>
          )}
        </div>

        {/* footer */}
        <div className="px-5 py-[13px] border-t border-[#1f242f] flex items-center justify-between gap-3 shrink-0">
          <div className="flex items-center gap-3 min-w-0">
            {cfg.generated && (
              <button
                onClick={() => void resetConfig()}
                disabled={cfg.saving}
                className="inline-flex items-center gap-[7px] bg-transparent text-fg-muted border border-line-ghost rounded-md px-[13px] py-[8px] text-[12.5px] font-medium cursor-pointer transition-colors hover:bg-hover2 hover:border-line-hover2 hover:text-[#dfe3ea] disabled:opacity-50 disabled:cursor-default"
              >
                <RotateCcw size={13} strokeWidth={2} />
                Reset to default
              </button>
            )}
            <span className="text-[11.5px] text-fg-dim truncate">
              {dirty ? 'Unsaved changes' : cfg.hint}
            </span>
          </div>

          <div className="flex items-center gap-[10px] shrink-0">
            <button
              onClick={tryClose}
              className="bg-transparent text-fg-muted border border-line-ghost rounded-md px-[15px] py-[9px] text-[13px] font-medium cursor-pointer transition-colors hover:bg-hover hover:text-[#cfd4de]"
            >
              Cancel
            </button>
            {cfg.saved && !dirty ? (
              <button className="inline-flex items-center gap-2 bg-success text-white border-none rounded-md px-4 py-[9px] text-[13px] font-semibold cursor-default">
                <Check size={15} strokeWidth={2.4} />
                Saved
              </button>
            ) : (
              <button
                onClick={() => void saveConfig()}
                disabled={!dirty || cfg.saving}
                className={`${primaryBtn} gap-2 px-4 py-[9px] text-[13px] disabled:opacity-45 disabled:cursor-default`}
              >
                {cfg.saving ? (
                  <Spinner size={14} strokeWidth={2.4} />
                ) : (
                  <Save size={15} strokeWidth={2} />
                )}
                {cfg.saving ? 'Saving…' : 'Save'}
              </button>
            )}
          </div>
        </div>
      </div>
    </ModalBackdrop>
  )
}
