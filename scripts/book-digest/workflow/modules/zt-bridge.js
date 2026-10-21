// ==== module zt-bridge v7 ====
defineModule({
  id: 'zt-bridge', version: '7', requires: ['zt-digest-lib'],
  summary: 'Bounded cross-book discovery publishes a report; an identified accepted selection applies through guarded script edits.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'run arguments' },
    mode: { type: 'string', required: true, description: 'run mode' },
    model: { type: 'any', description: 'resolved judge model; unused for apply' },
    profile: { type: 'string', required: true, description: 'book or math' },
  },
  run: async (ctx, p) => {
    const Z=zdMake(ctx,p), A=Z.A, work=`${Z.OUT}\\bridge`
    if (await Z.settledOnResume('bridge', ['bridge'])) return { status: 'done', reused: true }
    for(const [key,low,high] of [['bridgeMax',1,120],['bridgeCandidates',1,8]])if(A[key]!==undefined&&(!Number.isInteger(A[key])||A[key]<low||A[key]>high))return {status:'failed',error:`${key} must be an integer in ${low}..${high}.`}
    if((A.bridgeBooks!==undefined&&typeof A.bridgeBooks!=='string')||(A.bridgeReport!==undefined&&typeof A.bridgeReport!=='string'))return {status:'failed',error:'bridgeBooks and bridgeReport must be strings.'}
    const q=v=>`'${String(v).replace(/'/g,"''")}'`
    // The stage publishes its own book revision (`zt-digest finalize -Step stage`): the ledger binds its artifacts
    // by hash and checks the coverage it derives. On grossman2002integration trial 8 no stage published, so the
    // revision stayed ready and the run could never be content-complete.
    const publish = async (layer, more) => {
      const { run, json } = await Z.runScript(`${layer}: publish`, 'Bridge', `${Z.DIGEST} finalize -Step stage -Out "${Z.OUT}" -Layer ${layer}${more}`, ' (it records the stage publication in the ledger)')
      log(json && json.state ? `${layer}: publication ${json.unit || ''} ${json.state}${json.disposition ? ` (${json.disposition})` : ''}.` : `${layer}: the publication failed: ${(run && run.output) || 'no output'}`)
      return json || { layer, state: 'failed', error: (run && run.output) || 'no output' }
    }
    const selection=v=>{
      if(v===undefined)return ''
      if(v==='all')return 'all'
      if(!Array.isArray(v)||v.some(id=>typeof id!=='string'||!/^b[0-9]{3}$/.test(id))||new Set(v).size!==v.length)throw Error('Bridge selection must be all or a unique array of proposal ids.')
      return v.join(',')
    }
    let accept,earlier
    try{accept=selection(A.bridgeApply);earlier=selection(A.bridgeApplyEarlier)}catch(e){return {status:'failed',error:String(e)}}
    const applying=!!(A.bridgeReport||accept||earlier)
    if(applying&&(p.mode!=='bridge'||!A.bridgeReport||!(accept||earlier)))
      return {status:'failed',error:'Bridge apply requires mode bridge, bridgeReport, and a nonempty acceptance section.'}
    const step=(s,more='')=>`${Z.tool('bridge-compiler')} -Step ${s} -Staging ${q(Z.OUT)} -CitationKey ${q(Z.KEY)} -Work ${q(work)}${more}`
    if(applying){
      const extra=` -ReportId ${q(A.bridgeReport)}${accept?` -Accept ${q(accept)}`:''}${earlier?` -AcceptEarlier ${q(earlier)}`:''} -Apply`
      const result=await Z.runScript('bridge: apply','Bridge',step('apply',extra),'; filing must stay quiet: no insertion, movement, deletion, key rename, retype, or Organize during apply')
      return result.json||{status:'failed',error:result.run&&result.run.output}
    }
    const opts={dsh:work,skills:['zettelkasten','asd-ste100-zh'],effort:Z.DSH_EFFORT,phase:'Bridge',writes:`${work}\\out`,readOnly:true,remove:'delete',keep:true}
    // A report freezes its plan and proposals. So publish only after a plan whose jobs all answered;
    // otherwise keep the folder unpublished, so a rerun of mode bridge on the same out resumes it.
    // The cleanup (delete every judge conversation and the DSH workspace) always runs, even after a throw.
    let planned,reported,error
    try{
      planned=await Z.drive('bridge: plan',{...opts,command:step('plan',` -Profile ${q(p.profile)} -MaxSources ${q(A.bridgeMax??60)} -Candidates ${q(A.bridgeCandidates??5)}${A.bridgeBooks?` -Books ${q(A.bridgeBooks)}`:''}`)})
      const plan=planned&&planned.ok?zdLastJson(planned.output):null
      const failedJobs=((planned&&planned.results)||[]).filter(r=>r.status!=='done').map(r=>r.id)
      if(!plan||plan.ok===false)error=`Bridge planning failed: ${(plan&&(plan.errors||plan.error))?JSON.stringify(plan.errors||plan.error):((planned&&(planned.error||planned.output))||'no output')}`
      else if(failedJobs.length)error=`Bridge judge jobs failed: ${failedJobs.join(', ')}. The report is not published; rerun mode bridge with the same out to resume.`
      else reported=await Z.drive('bridge: report',{...opts,command:step('report')})
    }catch(e){error=error||String(e)}finally{
      try{await Z.cleanupDsh('bridge: clean up',opts)}catch(e){log(`bridge: clean up failed: ${e}`)}
    }
    const summary=zdLastJson((reported&&reported.output)||'')
    if(error)return {status:'failed',error}
    if(!summary||!summary.step||!summary.report_id)return {status:'failed',error:'No Bridge report summary (step and report_id missing).'}
    // The report's proposals stay open for the user's review; the publication records the report itself.
    const publication=await publish('bridge',` -Report "${work}\\proposals.json"`)
    return {...summary,publication}
  },
})
// ==== end module zt-bridge ====
