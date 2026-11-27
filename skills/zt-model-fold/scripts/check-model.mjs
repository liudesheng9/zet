// node check-model.mjs <modelHead> [--stage fold|settings|restructure|final] [--json]
// Thin wrapper over zt-model-restructure/scripts/check-model.mjs, defaulting to Stage 1 (`fold`).
import { runCli } from '../../zt-model-restructure/scripts/check-model.mjs'
runCli('fold')
