import { useEffect, useState } from 'react'
import { PROTOCOL_VERSION } from '@gameboy/protocol'
import { IconDeviceMobile, IconQrcode } from '@tabler/icons-react'
import { QRCodeSVG } from 'qrcode.react'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardFooter, CardHeader } from '@/components/ui/card'
import { type RemoteSessionClient, tauriRemoteSessionClient } from './remote-client'
import type { RemoteErrorCode, RemoteSnapshot } from './remote-types'
import { useRemoteSession } from './use-remote-session'

const errorHeadings: Record<RemoteErrorCode, string> = {
  'no-lan-address': 'No local network address was found',
  'bind-failed': 'The controller server could not start',
  'assets-unavailable': 'The mobile controller files are unavailable',
  'server-failed': 'The controller session stopped',
  'runtime-unavailable': 'The emulator runtime is unavailable',
  'invalid-lifecycle': 'The controller session is busy'
}

const formatExpiry = (expiresAtUnixMs: number) =>
  new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit' }).format(new Date(expiresAtUnixMs))

const Latency = ({ snapshot }: { snapshot: RemoteSnapshot }) => {
  if (!snapshot.latency || snapshot.latency.samples === 0) return null
  return <p className="remote-latency">Local input p95: {snapshot.latency.p95Ms} ms</p>
}

export type RemoteControllerPanelProps = {
  client?: RemoteSessionClient
}

export const RemoteControllerPanel = ({ client = tauriRemoteSessionClient }: RemoteControllerPanelProps) => {
  const { snapshot, busy, start, end } = useRemoteSession(client)
  const [now, setNow] = useState(Date.now)
  const waitingExpiry = snapshot.phase === 'waiting' ? snapshot.expiresAtUnixMs : null
  const expired = waitingExpiry !== null && waitingExpiry <= now

  useEffect(() => {
    if (waitingExpiry === null) return
    const remainingMs = waitingExpiry - Date.now()
    if (remainingMs <= 0) {
      setNow(Date.now())
      return
    }
    const timer = window.setTimeout(() => setNow(Date.now()), remainingMs + 1)
    return () => window.clearTimeout(timer)
  }, [waitingExpiry])

  return (
    <Card className="remote-controller-panel" role="region" aria-labelledby="remote-controller-title">
      <CardHeader className="remote-controller-header">
        <div>
          <p className="eyebrow">Remote play</p>
          <h2 id="remote-controller-title" className="remote-controller-title" aria-live="polite">
            {snapshot.phase === 'off' && 'Mobile controller is off'}
            {snapshot.phase === 'waiting' && !expired && 'Scan to connect'}
            {snapshot.phase === 'connected' && !expired && 'Mobile controller connected'}
            {expired && 'Pairing link expired'}
            {snapshot.phase === 'error' && 'Mobile controller unavailable'}
          </h2>
        </div>
        <Badge variant="outline">Protocol {PROTOCOL_VERSION}</Badge>
      </CardHeader>

      <CardContent className="remote-controller-content">
        {snapshot.phase === 'off' && (
          <div className="remote-controller-summary">
            <IconDeviceMobile aria-hidden="true" size={24} />
            <p>Start a private session to use one phone on this local network.</p>
          </div>
        )}

        {snapshot.phase === 'waiting' && !expired && (
          <div className="remote-pairing">
            <div className="remote-qr">
              <QRCodeSVG
                value={snapshot.pairingUrl}
                size={176}
                level="M"
                marginSize={2}
                title="Mobile controller pairing QR Code"
              />
            </div>
            <div className="remote-pairing-details">
              <p>Scan this code with a phone connected to the same local network.</p>
              <textarea
                aria-label="Pairing URL"
                autoComplete="off"
                className="remote-pairing-url"
                readOnly
                rows={3}
                spellCheck={false}
                value={snapshot.pairingUrl}
                wrap="soft"
              />
              <p>Pairing expires at {formatExpiry(snapshot.expiresAtUnixMs)}.</p>
            </div>
          </div>
        )}

        {snapshot.phase === 'connected' && !expired && (
          <div className="remote-controller-summary">
            <IconDeviceMobile aria-hidden="true" size={24} />
            <div>
              <p>Controller ID</p>
              <strong>{snapshot.controllerId}</strong>
            </div>
          </div>
        )}

        {expired && (
          <Alert>
            <IconQrcode aria-hidden="true" />
            <AlertTitle>This pairing link is no longer valid</AlertTitle>
            <AlertDescription>End this session and start a new one to generate another QR code.</AlertDescription>
          </Alert>
        )}

        {snapshot.phase === 'error' && (
          <Alert variant="destructive">
            <IconDeviceMobile aria-hidden="true" />
            <AlertTitle>{errorHeadings[snapshot.error.code]}</AlertTitle>
            <AlertDescription>
              <p>{snapshot.error.message}</p>
              <p>Keyboard controls remain available while you retry or continue playing.</p>
            </AlertDescription>
          </Alert>
        )}

        <Latency snapshot={snapshot} />
      </CardContent>

      <CardFooter className="remote-controller-actions">
        {(snapshot.phase === 'off' || snapshot.phase === 'error') && (
          <Button type="button" onClick={() => void start()} disabled={busy !== null}>
            {busy === 'starting'
              ? 'Starting mobile controller…'
              : snapshot.phase === 'error'
                ? 'Try mobile controller again'
                : 'Start mobile controller'}
          </Button>
        )}
        {(snapshot.phase === 'waiting' || snapshot.phase === 'connected') && (
          <Button type="button" variant="secondary" onClick={() => void end()} disabled={busy !== null}>
            {busy === 'ending' ? 'Ending session…' : 'End session'}
          </Button>
        )}
      </CardFooter>
    </Card>
  )
}
