import { cn } from '../../lib/cn'

interface ToggleProps {
  on: boolean
  onClick: () => void
  disabled?: boolean
}

/** Track 34×20, knob 14px — matches design toggle exactly. */
export function Toggle({ on, onClick, disabled }: ToggleProps) {
  return (
    <div
      onClick={() => !disabled && onClick()}
      className={cn(
        'relative w-[34px] h-5 rounded-full shrink-0 transition-colors duration-150',
        disabled ? 'cursor-default opacity-60' : 'cursor-pointer',
        on ? 'bg-accent' : 'bg-[#2b313d]',
      )}
    >
      <div
        className={cn(
          'absolute top-[3px] left-[3px] w-[14px] h-[14px] rounded-full bg-white transition-transform duration-150',
          on && 'translate-x-[14px]',
        )}
      />
    </div>
  )
}
