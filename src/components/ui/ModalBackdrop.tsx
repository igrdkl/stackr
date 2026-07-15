import type { ReactNode } from 'react'

interface ModalBackdropProps {
  onClose: () => void
  padded?: boolean
  /** When false, a backdrop click does NOT close the modal (use the explicit
   * close/cancel controls instead). Defaults to true. */
  dismissable?: boolean
  children: ReactNode
}

export function ModalBackdrop({ onClose, padded, dismissable = true, children }: ModalBackdropProps) {
  // Close only when the press starts on the backdrop itself. Using onMouseDown
  // (not onClick) also avoids the click that *opens* the modal from immediately
  // closing it via event propagation.
  return (
    <div
      onMouseDown={(e) => {
        if (dismissable && e.target === e.currentTarget) onClose()
      }}
      className="fixed inset-0 z-[60] flex items-center justify-center bg-[rgba(6,8,12,.66)] backdrop-blur-[3px]"
      style={padded ? { padding: 24 } : undefined}
    >
      <div>{children}</div>
    </div>
  )
}
