import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import assert from 'node:assert/strict'
const root=process.cwd(), folder=path.join(root,'scripts/book-digest/workflow')
const lib=await import(pathToFileURL(path.join(os.homedir(),'.claude/skills/workflow-framework-protocol/tools/lib.mjs')))
const AsyncFunction=Object.getPrototypeOf(async function(){}).constructor
const json=(v)=>JSON.stringify(v)
// #176: the bridge stage's steps are one book-steps.ps1 call each, run by a DeepSeek drive job. The stand-in
// answers the drive manager's prompt with the step's one JSON line and the content manager's job list.
const drive=(text)=>{
  const escaped=(/ -Json '([^']*)'/.exec(text)||[])[1]||'{}'
  let options={}
  try{options=JSON.parse(JSON.parse(`"${escaped}"`))}catch(e){options={}}
  const step=(/ -Step ([\w-]+) -Out /.exec(text)||[])[1]||''
  return {step,options}
}
async function run(kind,args,options={}){
 const calls=[]
 const l=lib.loadBlueprint(path.join(folder,`zt-${kind}-digest.blueprint.json`)), source=lib.build(l.bp,lib.loadModules(l.dirs)).script
 const agent=async(prompt,opts)=>{
  const text=String(prompt)
  const cmd=(text.match(/pwsh -NoProfile -File[^\n]*|zt-digest bridge -[^\n]*/)||[''])[0]
  if(opts.label==='choose DeepSeek source')return {source:'official',errors:''}
  // #176: a content manager runs the judge job of the plan's job file.
  if(text.startsWith('You are a DeepSeek subagent manager for ZET ')){
   calls.push({prompt:text,...opts,cmd,kind:'content'})
   return {ok:true,deleted:1,not_deleted:[],results:[{id:'br-001',session_id:'judge-1',status:options.failJudge?'failed':'done',answer:'{}',registered:true,evidence:true,deleted:true}]}
  }
  if(text.startsWith('You are a DeepSeek subagent manager for a ZET workflow step (')){
   const {step,options:o}=drive(text)
   calls.push({prompt:text,...opts,cmd,step,options:o,kind:'drive'})
   if(step==='bridge-start'){
    if(options.failPlan)return {answer:json({ok:false,step:'bridge-start',error:'read failure'})}
    return {answer:json({ok:true,step:'plan',status:'done',plan_id:'p-frozen',report_id:'r-frozen',counts:{selected:1,new:options.newCount??1,earlier:1},jobs:1,file:'D:\\work\\bridge\\out\\jobs.json'})}
   }
   if(step==='bridge-report')return {answer:json({ok:true,step:'report',status:'done',citation_key:'Key',plan_id:'p-frozen',report_id:'r-frozen',counts:{selected:1,new:options.newCount??1,earlier:1},jobs:[],report:'D:\\work\\bridge\\report.md'})}
   if(step==='bridge-apply')return {answer:json({ok:true,step:'apply',status:'done',citation_key:'Key',plan_id:'p-frozen',report_id:'r-frozen',counts:{selected:1,new:1,earlier:1},outcomes:[],publication:{unit:'bridge/book@3',revision:3,coverage:{accepted:1,open:1}}})}
   if(step==='publish')return {answer:json({ok:true,layer:'bridge',unit:'bridge/book@2',revision:2,state:'done',complete:true,disposition:'published',coverage:{open:1}})}
   if(step==='cleanup-start')return {answer:json({ok:true,kind:'cleanup_run',step:'plan',deletes:0,workspaces:0,plan_file:'D:\\work\\cleanup\\settle-plan.json',results_file:'D:\\work\\cleanup\\settle-results.json'})}
   if(step==='cleanup-apply')return {answer:json({ok:true,fallback:false,provider:{kind:'cleanup_run',step:'apply',deleted:0,absent:0,failed_count:0,workspaces_removed:1,workspaces_kept:0},ready:{kind:'cleanup_ready',claimed:1,done:1,partial:0,retry_count:0,failed_count:0,book:{unit:'cleanup/book@1',ok:true,status:'complete',cleanup_complete:true,content_complete:true}}})}
   if(step==='cleanup-report')return {answer:json({ok:true,kind:'run_report',completeness:{complete:true},counts:{},gaps:[],usage:{}})}
   throw Error(`unexpected step ${step}`)
  }
  // zt-dsh-cleanup: the one settle call the manager makes; it runs no command.
  if(/^Settle the DeepSeek Harness conversations and workspaces/.test(text)){calls.push({prompt:text,...opts,kind:'settle'});return {settle:'{"ops":0}',ok:true}}
  throw Error(`unexpected stage ${opts.label}: ${cmd}`)
 }
 let result,error
 try{result=await new AsyncFunction('args','agent','phase','log','parallel','pipeline',source.replace(/^export /m,''))(args,agent,()=>{},()=>{},fns=>Promise.all(fns.map(fn=>fn())),()=>{})}catch(e){error=e}
 return {result:result?.result||result,error,calls,halted:result?.halted}
}
const bridgeCalls=(r)=>r.calls.filter(c=>c.kind==='drive'||c.kind==='content'||c.kind==='settle'?c.label&&/^bridge: |^cleanup: |^report the run/.test(c.label||''):false)
for(const kind of ['econ','math']){
 const args={mode:'bridge',out:'D:\\work',citationKey:'Key'}
 const discovery=await run(kind,args)
 assert(!discovery.error,`${kind} supports bridge mode: ${discovery.error}`)
 assert.equal(discovery.result?.bridge?.status,'done',`${kind} returns its Bridge summary: ${JSON.stringify(discovery.result)} ${discovery.calls.map(c=>c.label)}`)
 assert(discovery.calls.some(c=>c.step==='bridge-start'),`${kind} plans through the bridge-start step`)
 // A discovery applies every new-carrier proposal of its own report at once: no review waits for the user.
 // Earlier-carrier proposals write into another book's tree (usually locked) and stay for an explicit apply.
 const auto=discovery.calls.filter(c=>c.step==='bridge-apply')
 assert.equal(auto.length,1,`${kind} applies the new proposals once after the report`)
 assert(auto[0].options.ReportId==='r-frozen'&&auto[0].options.Accept==='all'&&!auto[0].options.AcceptEarlier,`${kind} accepts all new-carrier proposals only: ${JSON.stringify(auto[0].options)}`)
 assert(discovery.calls.findIndex(c=>c.step==='bridge-report')<discovery.calls.indexOf(auto[0]),`${kind} applies after the report`)
 assert(!discovery.calls.some(c=>c.step==='publish'),`${kind} lets the apply publish the Bridge revision`)
 assert.equal(discovery.result?.bridge?.applied?.publication?.revision,3,`${kind} returns the apply summary: ${JSON.stringify(discovery.result?.bridge)}`)
 const none=await run(kind,args,{newCount:0})
 assert(!none.calls.some(c=>c.step==='bridge-apply'),`${kind} applies nothing when the report has no new proposal`)
 assert.equal(none.result?.bridge?.publication?.state,'done',`${kind} publishes its Bridge revision after a report with nothing to apply`)
 assert(discovery.calls.some(c=>/ledger resource .*-RegisterId/.test(c.prompt)),`${kind} records the judge conversation for the run-level cleanup`)
 const apply=await run(kind,{...args,bridgeReport:'r-frozen',bridgeApply:['b001']})
 assert(!apply.error,`${kind} apply succeeds: ${apply.error}`)
 assert.equal(apply.result?.bridge?.status,'done')
 const applySteps=apply.calls.filter(c=>c.step)
 assert.equal(applySteps.length,1,`${kind} apply runs only its one step, with no probe or judge`)
 assert(/-Step bridge-apply/.test(applySteps[0].cmd)&&applySteps[0].options.ReportId==='r-frozen'&&applySteps[0].options.Accept==='b001',`${kind} apply names the report and the selection: ${JSON.stringify(applySteps[0].options)}`)
 for(const selection of [{bridgeApply:'all'},{bridgeApplyEarlier:['b002']},{bridgeApplyEarlier:'all'}]){
  const r=await run(kind,{...args,bridgeReport:'r-frozen',...selection})
  assert(!r.error&&r.result?.bridge?.status==='done'&&r.calls.filter(c=>c.step).length===1,`${kind} supports explicit all and earlier-only acceptance`)
 }
 for(const value of [true,false,['b001',true],['b001','b001'],'b001']){
  const r=await run(kind,{...args,bridgeReport:'r-frozen',bridgeApply:value})
  assert(r.error||r.halted||r.result?.stage==='args'||r.result?.bridge?.status==='failed',`${kind} refuses malformed selection ${JSON.stringify(value)}`)
  assert(!r.calls.some(c=>c.label==='choose DeepSeek source'||c.step==='bridge-start'),`${kind} does not probe or judge malformed acceptance`)
 }
 for(const extra of [{bridgeMax:1.5},{bridgeMax:'12'},{bridgeCandidates:9},{bridgeBooks:true},{bridgeReport:42,bridgeApply:'all'}]){
  const r=await run(kind,{...args,...extra})
  assert(r.error||r.halted||r.result?.stage==='args'||r.result?.bridge?.status==='failed',`${kind} rejects invalid Bridge argument types and bounds`)
 }
 assert(discovery.calls.filter(c=>c.kind==='content').every(c=>/zettelkasten/.test(c.prompt)&&/never -Apply/.test(c.prompt)),`${kind} judges load the skills and have read-only approvals`)
 assert(!discovery.calls.some(c=>/^Clean up:/m.test(c.prompt)),`${kind} leaves deletion to the run-level cleanup`)
 const judged=await run(kind,args,{failJudge:true})
 assert.equal(judged.result?.bridge?.status,'failed',`${kind} fails the stage when a judge job fails`)
 assert(!judged.calls.some(c=>c.step==='bridge-report'),`${kind} publishes no report after a failed judge job`)
 assert(judged.calls.some(c=>/ledger resource .*-RegisterId/.test(c.prompt)),`${kind} records the judge conversation even after a failed judge job`)
 const failedPlan=await run(kind,args,{failPlan:true})
 assert(!failedPlan.calls.some(c=>c.step==='bridge-report'),`${kind} publishes no report after failed planning`)
 const missing=await run(kind,{...args,bridgeApply:'all'})
 assert(missing.error||missing.halted||missing.result?.stage==='args'||missing.result?.bridge?.status==='failed',`${kind} refuses acceptance without report: ${JSON.stringify(missing.result)}`)
 console.log(`ok: ${kind} discovery applies its new proposals; explicit apply stays a separate, bounded path`)
}
