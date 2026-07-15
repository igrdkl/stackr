import { openUrl } from '@tauri-apps/plugin-opener'
import { Copy } from 'lucide-react'
import { ScreenHeader } from '../components/ui/ScreenHeader'
import { EngineInstallCard, EngineVersionCard } from '../components/ui/ServiceCards'
import { useStore } from '../store/useStore'
import { isTauri } from '../lib/api'
import { MAIL_ENGINES } from '../data/catalog'
import { ghostBtnAlt, sectionLabel } from '../lib/styles'

const UNINSTALL_NOTE = 'Mailpit is stopped and its binary removed. Caught mail is in-memory, so nothing is left on disk.'

// Laravel-style .env block; the same host/port work for any framework's SMTP config.
const ENV_SNIPPET = [
  'MAIL_MAILER=smtp',
  'MAIL_HOST=127.0.0.1',
  'MAIL_PORT=1025',
  'MAIL_USERNAME=null',
  'MAIL_PASSWORD=null',
  'MAIL_ENCRYPTION=null',
].join('\n')

export function Mail() {
  const mail = useStore((s) => s.mail)
  const installed = mail.length > 0

  const openInbox = (port: number) => {
    if (isTauri()) void openUrl(`http://127.0.0.1:${port}`)
    else window.open(`http://127.0.0.1:${port}`, '_blank')
  }

  return (
    <div className="max-w-[920px] w-full">
      <ScreenHeader
        title="Mail"
        subtitle="Catch every email your projects send into one local inbox — nothing leaves your machine."
        className="mb-6"
      />

      <div className="flex flex-col gap-3">
        {mail.map((svc) => (
          <EngineVersionCard
            key={svc.id}
            svc={svc}
            engines={MAIL_ENGINES}
            uninstallNote={UNINSTALL_NOTE}
            openAction={{ label: 'Open inbox', onClick: () => openInbox(svc.port) }}
          />
        ))}

        {installed ? (
          <div className="bg-card border border-line rounded-[10px] p-[18px]">
            <div className="flex items-center justify-between gap-3 mb-2">
              <label className={sectionLabel}>Point your app at Mailpit</label>
              <button
                onClick={() => void navigator.clipboard.writeText(ENV_SNIPPET)}
                className={`${ghostBtnAlt} inline-flex items-center gap-[6px] px-[12px] py-[6px] text-[12px]`}
              >
                <Copy size={13} strokeWidth={2} />
                Copy .env
              </button>
            </div>
            <div className="text-[12px] text-fg-dim leading-[1.6] mb-3">
              Add these to your project's <span className="font-mono">.env</span> (Laravel shown; any
              framework's SMTP settings work). Mailpit accepts mail on{' '}
              <span className="font-mono">127.0.0.1:1025</span> and shows it at{' '}
              <span className="font-mono">127.0.0.1:8025</span>.
            </div>
            <pre className="bg-control border border-line-input rounded-md px-[12px] py-[10px] font-mono text-[12px] text-[#cfd4de] overflow-x-auto">
              {ENV_SNIPPET}
            </pre>
          </div>
        ) : (
          <EngineInstallCard
            title="Install Mailpit"
            hint="A single-binary mail catcher with a web inbox — SMTP on 1025, UI on 8025."
            engines={MAIL_ENGINES}
            services={mail}
          />
        )}
      </div>
    </div>
  )
}
