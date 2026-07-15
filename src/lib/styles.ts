// Shared className fragments mirroring the design's reusable controls.

// Primary accent button (padding supplied by caller).
export const primaryBtn =
  'inline-flex items-center bg-accent hover:bg-accent-hover text-white border-none rounded-md font-semibold cursor-pointer transition-colors'

// Ghost button — bordered #262c38 (PHP page, extensions, install modal).
export const ghostBtn =
  'bg-[#1a1e28] text-[#c2c7d2] border border-line-input rounded-md px-3 py-[7px] text-[12.5px] font-medium cursor-pointer transition-colors hover:bg-hover2 hover:border-line-hover2 hover:text-[#dfe3ea]'

// Ghost button — bordered #2a3140 (server/db/cache install, settings, clear).
export const ghostBtnAlt =
  'bg-[#1a1e28] text-[#d4d9e2] border border-line-ghost rounded-md font-medium cursor-pointer transition-colors hover:bg-hover2 hover:border-line-hover2'

// Danger ghost button (uninstall).
export const dangerBtn =
  'bg-transparent text-fg-muted border border-line-input rounded-md px-3 py-[7px] text-[12.5px] font-medium cursor-pointer transition-colors hover:bg-[rgba(248,81,73,.12)] hover:border-[rgba(248,81,73,.4)] hover:text-danger'

// 30px transparent icon button.
export const iconBtn =
  'w-[30px] h-[30px] rounded-md bg-transparent border-none text-[#7b818f] flex items-center justify-center cursor-pointer transition-colors hover:bg-hover hover:text-[#cfd4de]'

// Uppercase section / field label.
export const sectionLabel =
  'block text-[11px] uppercase tracking-[.07em] text-fg-dim font-semibold'

// Custom select base (chevron is overlaid separately).
export const selectBase =
  'appearance-none w-full bg-control border border-line-input rounded-md text-[#d4d9e2] font-mono text-[13px] cursor-pointer outline-none focus:border-accent'

// Text/mono inputs.
export const inputBase =
  'w-full bg-control border border-line-input rounded-md text-[#d4d9e2] font-mono text-[13px] px-[11px] py-[9px] outline-none focus:border-accent'
