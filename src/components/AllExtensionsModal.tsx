import { Search, X } from 'lucide-react'
import { ModalBackdrop } from './ui/ModalBackdrop'
import { ExtensionRow } from '../pages/PHP'
import { useStore } from '../store/useStore'

export function AllExtensionsModal() {
  const open = useStore((s) => s.extModalOpen)
  const close = useStore((s) => s.closeAllExtensions)
  const extensions = useStore((s) => s.extensions)
  const version = useStore((s) => s.extVersion)
  const search = useStore((s) => s.extModalSearch)
  const setSearch = useStore((s) => s.setExtModalSearch)

  if (!open) return null

  const enabled = extensions.filter((e) => e.enabled).length
  const q = search.trim().toLowerCase()
  const matched = q
    ? extensions.filter((e) => e.name.includes(q) || e.description.toLowerCase().includes(q))
    : extensions
  const trimmed = search.trim()

  return (
    <ModalBackdrop onClose={close} padded>
      <div
        className="w-[880px] max-w-[92vw] bg-card border border-line-input rounded-xl overflow-hidden flex flex-col"
        style={{ boxShadow: '0 24px 60px rgba(0,0,0,.5)', maxHeight: '88vh' }}
      >
        {/* header */}
        <div className="px-5 py-[15px] border-b border-[#1f242f] flex items-center justify-between gap-4 shrink-0">
          <div className="min-w-0">
            <div className="text-[14.5px] font-semibold">
              All extensions
              {version && (
                <span className="font-mono font-medium text-fg-muted"> · PHP {version}</span>
              )}
            </div>
            <div className="text-[11.5px] text-fg-dim mt-[2px]">
              {enabled} of {extensions.length} enabled · toggle to enable or disable, install PECL extras
            </div>
          </div>
          <button
            onClick={close}
            className="w-7 h-7 shrink-0 rounded-md bg-transparent border-none text-[#7b818f] flex items-center justify-center cursor-pointer transition-colors hover:bg-hover hover:text-[#cfd4de]"
          >
            <X size={16} strokeWidth={2} />
          </button>
        </div>

        {/* search */}
        <div className="px-5 py-3 border-b border-[#1f242f] shrink-0">
          <div className="relative">
            <Search
              size={14}
              strokeWidth={2}
              className="absolute left-[11px] top-1/2 -translate-y-1/2 text-fg-dim pointer-events-none"
            />
            <input
              autoFocus
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search all extensions…"
              className="w-full bg-control border border-line-input rounded-[7px] text-[#d4d9e2] text-[12.5px] outline-none focus:border-accent pl-[33px] pr-[11px] py-[9px]"
            />
          </div>
        </div>

        {/* body */}
        <div className="p-4 flex-1 min-h-0 overflow-y-auto">
          {matched.length > 0 ? (
            <div className="grid grid-cols-2 gap-2">
              {matched.map((e) => (
                <ExtensionRow key={e.name} e={e} />
              ))}
            </div>
          ) : (
            <div className="py-[40px] text-center text-[12.5px] text-fg-dim">
              {trimmed ? `No extensions match "${trimmed}"` : 'No extensions found for this build.'}
            </div>
          )}
        </div>
      </div>
    </ModalBackdrop>
  )
}
