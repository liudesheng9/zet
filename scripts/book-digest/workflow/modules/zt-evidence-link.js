// ==== module zt-evidence-link v1 ====
// docs/empirical-models-proposal.md §8.2 step 9: after the model layer and the Findings layer, ONE DeepSeek drive job
// runs `zt-digest evidence-link`, the deterministic step that links each Finding Card of the book to the estimate part
// of an empirical model that reports the same number in the same group (`依赖 [[估计]]` in the Finding, `对应 [[Finding]]`
// in the estimate). An unmatched or ambiguous Finding is reported, never forced; a second run writes nothing.
defineModule({
  id: 'zt-evidence-link', version: '1', requires: ['@dsmp', 'zt-workflow-lib'],
  summary: 'Evidence link: one DeepSeek drive job links the Finding Cards to the estimates of the empirical models (a script step, no content job).',
  params: {
    out: { type: 'string', required: true, description: 'the prepared run\'s staging folder' },
    key: { type: 'string', required: true, description: 'the book\'s Citation key' },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const OUT = String(p.out).replace(/[\/]+$/, '')
    const tail = (text) => String(text || '').trim().slice(0, 400)
    const n = (v) => Number(v || 0)
    const deepseek = { deleted: 0, not_deleted: [] }
    const command = `zt-digest evidence-link -CitationKey "${p.key}" -Out "${OUT}"`
    const json = await ztDrive(ctx, { label: `${ctx.stage}: evidence link`, step: 'evidence-link', command, out: OUT, cwd: OUT, phase: ctx.phase, ledger: 'zt-digest ledger', deepseek })
    if (json.ok !== true) return { status: 'failed', book: p.key, error: tail(json.error || 'the evidence-link step failed'), deepseek }
    log(`Evidence link: ${n(json.linked)} of ${n(json.findings)} Finding Card(s) linked to ${n(json.estimates)} estimate(s); ${n(json.unmatched)} unmatched, ${n(json.ambiguous)} ambiguous (${json.file || 'no report file'}).`)
    return { status: 'complete', book: p.key, findings: n(json.findings), estimates: n(json.estimates), linked: n(json.linked), unmatched: n(json.unmatched), ambiguous: n(json.ambiguous), file: json.file || null, deepseek }
  },
})
// ==== end module zt-evidence-link ====
