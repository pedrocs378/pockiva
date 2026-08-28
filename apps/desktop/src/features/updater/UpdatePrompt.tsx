import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '@/components/ui/dialog'
import type { UpdaterView } from './use-updater'

type UpdatePromptProps = {
  updater: UpdaterView
}

const isOpen = (phase: UpdaterView['state']['phase']) => phase !== 'idle' && phase !== 'checking'

export const UpdatePrompt = ({ updater }: UpdatePromptProps) => {
  const { state } = updater
  const canDismiss = state.phase === 'available' || state.phase === 'error'

  return (
    <Dialog
      open={isOpen(state.phase)}
      onOpenChange={(open) => {
        if (!open && canDismiss) void updater.dismiss()
      }}
    >
      <DialogContent showCloseButton={false}>
        {state.phase === 'available' ? (
          <>
            <DialogHeader>
              <DialogTitle>Pockiva {state.version} is available</DialogTitle>
              <DialogDescription>
                Install the update now, or keep playing and update the next time Pockiva starts.
              </DialogDescription>
            </DialogHeader>
            <section aria-label="Release notes" className="space-y-2">
              <h3 className="text-sm font-semibold">What changed</h3>
              <p className="whitespace-pre-wrap text-sm text-[var(--muted-foreground)]">
                {state.notes ?? 'No release notes were provided for this version.'}
              </p>
            </section>
            <DialogFooter>
              <Button type="button" variant="secondary" onClick={() => void updater.dismiss()}>
                Later
              </Button>
              <Button type="button" onClick={() => void updater.install()}>
                Update now
              </Button>
            </DialogFooter>
          </>
        ) : null}

        {state.phase === 'downloading' ? (
          <>
            <DialogHeader>
              <DialogTitle>Downloading Pockiva {state.version}</DialogTitle>
              <DialogDescription>
                {state.progress.percent === null
                  ? 'Downloading update…'
                  : `Downloading update… ${state.progress.percent}%`}
              </DialogDescription>
            </DialogHeader>
            <progress
              aria-label="Update download progress"
              className="w-full"
              max={100}
              value={state.progress.percent ?? undefined}
            />
          </>
        ) : null}

        {state.phase === 'installing' ? (
          <DialogHeader>
            <DialogTitle>Installing Pockiva {state.version}</DialogTitle>
            <DialogDescription>Installing update… Pockiva will restart when it is ready.</DialogDescription>
          </DialogHeader>
        ) : null}

        {state.phase === 'error' ? (
          <>
            <DialogHeader>
              <DialogTitle>Update failed</DialogTitle>
              <DialogDescription>{state.message}</DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button type="button" variant="secondary" onClick={() => void updater.dismiss()}>
                Close
              </Button>
            </DialogFooter>
          </>
        ) : null}
      </DialogContent>
    </Dialog>
  )
}
