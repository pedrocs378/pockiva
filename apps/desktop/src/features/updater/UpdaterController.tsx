import { UpdatePrompt } from './UpdatePrompt'
import { useUpdater } from './use-updater'

export const UpdaterController = () => <UpdatePrompt updater={useUpdater()} />
