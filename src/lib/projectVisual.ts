export interface ProjectVisual {
  mark: string
  markBg: string
  markColor: string
  label: string
  fwColor: string
  fwBg: string
}

/** Derive monogram + framework-badge styling from a project's framework. */
export function projectVisual(framework: string | null): ProjectVisual {
  if (!framework) {
    return {
      mark: 'P',
      markBg: 'rgba(120,127,139,.16)',
      markColor: '#aab1bf',
      label: 'Pure PHP',
      fwColor: '#aab1bf',
      fwBg: 'rgba(120,127,139,.13)',
    }
  }
  const f = framework.toLowerCase()
  if (f.includes('laravel')) {
    return { mark: 'L', markBg: 'rgba(255,45,32,.14)', markColor: '#ff6a5c', label: framework, fwColor: '#ff8a7e', fwBg: 'rgba(255,45,32,.12)' }
  }
  if (f.includes('symfony')) {
    return { mark: 'S', markBg: 'rgba(120,160,210,.15)', markColor: '#9fb6dc', label: framework, fwColor: '#a7bce0', fwBg: 'rgba(120,160,210,.12)' }
  }
  if (f.includes('wordpress')) {
    return { mark: 'W', markBg: 'rgba(29,95,138,.18)', markColor: '#5b9bd0', label: framework, fwColor: '#7fb3e0', fwBg: 'rgba(29,95,138,.14)' }
  }
  return {
    mark: framework[0].toUpperCase(),
    markBg: 'rgba(99,126,201,.16)',
    markColor: '#8aa0e6',
    label: framework,
    fwColor: '#9fb0e0',
    fwBg: 'rgba(99,126,201,.12)',
  }
}
