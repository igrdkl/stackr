import { AlertTriangle, Info, X } from 'lucide-react'
import { useStore } from '../../store/useStore'

/** Transient bottom-right notification (service errors, etc.). */
export function Toaster() {
  const toast = useStore((s) => s.toast)
  const dismiss = useStore((s) => s.dismissToast)
  if (!toast) return null

  const err = toast.kind === 'error'
  const accent = err ? '#f1645a' : '#7a9bff'

  return (
    <div className="fixed bottom-5 right-5 z-[80] max-w-[440px]">
      <div
        className="flex items-start gap-[11px] bg-card border rounded-[10px] pl-[14px] pr-[10px] py-[12px]"
        style={{
          borderColor: err ? 'rgba(248,81,73,.34)' : '#2a3140',
          boxShadow: '0 16px 40px rgba(0,0,0,.45)',
        }}
      >
        <span className="mt-[1px] shrink-0" style={{ color: accent }}>
          {err ? <AlertTriangle size={16} strokeWidth={2} /> : <Info size={16} strokeWidth={2} />}
        </span>
        <div className="text-[12.5px] leading-[1.5] text-[#d4d9e2] break-words flex-1 min-w-0">
          {toast.msg}
        </div>
        <button
          onClick={dismiss}
          className="w-[26px] h-[26px] shrink-0 rounded-md bg-transparent border-none text-[#7b818f] flex items-center justify-center cursor-pointer transition-colors hover:bg-hover hover:text-[#cfd4de]"
        >
          <X size={15} strokeWidth={2} />
        </button>
      </div>
    </div>
  )
}
