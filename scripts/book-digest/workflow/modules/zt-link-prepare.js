// ==== module zt-link-prepare v3 ====
// The link pass preparation: prepare-link-pass.ps1 writes each group's link file and lists the groups still
// pending (a group whose anchor already states its place in the book is skipped, so a rerun links the rest).
// Fails the stage when the script fails. No stage reads the account-wide usage ledger: run usage is recorded
// per provider run from that run's own result (R343-R346).
defineModule({
  id: 'zt-link-prepare', version: '3', requires: ['zt-digest-lib'],
  summary: 'ZT digest: prepare the cross-part link pass (prepare-link-pass.ps1); output the pending groups.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const linkPrep = await Z.zAgent(
      `Prepare the cross-part link pass of a ZET book digest. Do exactly this step.
1. Run this PowerShell command (one call): ${Z.tool('prepare-link-pass')} -Staging "${Z.OUT}"
   On a large book it takes several minutes. Run it in the foreground with timeout_ms: 600000 and wait for it; never in the background. If it fails because the ZT service is not up, run \`zt up\` once and run the command again. Its last line is JSON {"pending":[n…],"groups":[{"n","owned","done"}],"unowned","cards_all"}. If it fails, return ok=false with the error.
Return ok, pending (the "pending" array exactly), owned_total (sum of owned), unowned.`,
      {
        label: 'prepare link pass', phase: 'Link', model: 'haiku',
        schema: {
          type: 'object',
          properties: {
            ok: { type: 'boolean' }, error: { type: 'string' }, pending: { type: 'array', items: { type: 'number' } },
            owned_total: { type: 'number' }, unowned: { type: 'number' },
          },
          required: ['ok', 'pending'],
        },
      },
    )
    if (!linkPrep || !linkPrep.ok) {
      log(`Link preparation failed: ${linkPrep && linkPrep.error}`)
      throw new Error(`link preparation failed: ${(linkPrep && linkPrep.error) || 'no report'}`)
    }
    const pending = linkPrep.pending.filter(n => n >= 1 && n <= p.groups)
    log(`Link pass: ${pending.length} of ${p.groups} groups pending; ${linkPrep.owned_total} owned Cards; ${linkPrep.unowned} unowned (hubs, structure Cards, Model parts).`)
    return { pending, owned_total: linkPrep.owned_total, unowned: linkPrep.unowned }
  },
})
// ==== end module zt-link-prepare ====
