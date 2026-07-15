import { useEffect, useState } from 'react'
import { AlertTriangle, Check } from 'lucide-react'
import { ModalBackdrop } from './ui/ModalBackdrop'
import { useStore } from '../store/useStore'
import { primaryBtn } from '../lib/styles'

const cancelBtn =
  'bg-transparent text-fg-muted border border-line-ghost rounded-md px-[15px] py-[9px] text-[13px] font-medium cursor-pointer transition-colors hover:bg-hover hover:text-[#cfd4de]'
const dangerSolid =
  'inline-flex items-center bg-[#d2403a] hover:bg-[#dd4b45] text-white border-none rounded-md font-semibold cursor-pointer transition-colors px-4 py-[9px] text-[13px]'

/** Styled, app-themed replacement for window.confirm (see store askConfirm). */
export function ConfirmDialog() {
  const confirm = useStore((s) => s.confirm)
  const resolve = useStore((s) => s.resolveConfirm)
  const [checked, setChecked] = useState(false)

  // Reset the opt-in checkbox each time a new dialog opens.
  useEffect(() => {
    setChecked(confirm?.checkbox?.defaultChecked ?? false)
  }, [confirm])

  if (!confirm) return null

  const { danger, checkbox } = confirm

  return (
    <ModalBackdrop onClose={() => resolve(false, checked)}>
      <div
        className="w-[420px] max-w-[92vw] bg-card border border-line-input rounded-xl overflow-hidden"
        style={{ boxShadow: '0 24px 60px rgba(0,0,0,.5)' }}
      >
        <div className="p-5 flex items-start gap-[13px]">
          {danger && (
            <span
              className="shrink-0 w-9 h-9 rounded-full flex items-center justify-center"
              style={{ background: 'rgba(248,81,73,.12)', color: '#f1645a' }}
            >
              <AlertTriangle size={18} strokeWidth={2} />
            </span>
          )}
          <div className="min-w-0">
            <div className="text-[15px] font-semibold mb-[5px]">{confirm.title}</div>
            <div className="text-[13px] leading-[1.55] text-fg-muted break-words">
              {confirm.message}
            </div>

            {checkbox && (
              <label className="mt-[14px] flex items-center gap-[9px] cursor-pointer select-none">
                <span
                  onClick={() => setChecked((c) => !c)}
                  className="shrink-0 w-[17px] h-[17px] rounded-[5px] border flex items-center justify-center transition-colors"
                  style={{
                    background: checked ? '#d2403a' : 'transparent',
                    borderColor: checked ? '#d2403a' : '#39414f',
                  }}
                >
                  {checked && <Check size={12} strokeWidth={3} className="text-white" />}
                </span>
                <span className="text-[12.5px] text-fg-muted">{checkbox.label}</span>
              </label>
            )}
          </div>
        </div>

        <div className="px-5 py-[14px] border-t border-[#1f242f] flex justify-end gap-[10px]">
          <button onClick={() => resolve(false, checked)} className={cancelBtn}>
            {confirm.cancelLabel}
          </button>
          <button
            onClick={() => resolve(true, checked)}
            className={danger ? dangerSolid : `${primaryBtn} px-4 py-[9px] text-[13px]`}
          >
            {confirm.confirmLabel}
          </button>
        </div>
      </div>
    </ModalBackdrop>
  )
}
