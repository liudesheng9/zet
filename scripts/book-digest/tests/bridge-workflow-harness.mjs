import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import assert from 'node:assert/strict'
const root=process.cwd(), folder=path.join(root,'scripts/book-digest/workflow')
const lib=await import(pathToFileURL(path.join(os.homedir(),'.claude/skills/workflow-framework-protocol/tools/lib.mjs')))
const AsyncFunction=Object.getPrototypeOf(async function(){}).constructor
async function run(kind,args,options={}){
 const calls=[]
 const l=lib.loadBlueprint(path.join(folder,`zt-${kind}-digest.blueprint.json`)), source=lib.build(l.bp,lib.loadModules(l.dirs)).script
 const agent=async(prompt,opts)=>{
  const cmd=(String(prompt).match(/pwsh -NoProfile -File[^\n]*/)||[''])[0];calls.push({prompt:String(prompt),...opts,cmd})
  if(opts.label==='choose DeepSeek source')return {source:'official',errors:''}
  if(/Run every job below/.test(prompt))return {ok:true,results:[{id:'br-001',session_id:'judge-1',status:options.failJudge?'failed':'done',answer:'{}'}]}
  if(/^Clean up:/m.test(prompt))return {ok:true,results:[],deleted:1}
  if(/bridge-compiler/.test(cmd)){
   const plan=/-Step plan/.test(cmd)
   if(options.failPlan&&plan)return {ok:false,output:'read failure',results:[]}
   return {ok:true,output:JSON.stringify({ok:true,step:plan?'plan':(/-Apply\b/.test(cmd)?'apply':'report'),status:'done',citation_key:'Key',plan_id:'p-frozen',report_id:'r-frozen',counts:{selected:1,new:1,earlier:0},jobs:plan?[{id:'br-001',files:['prompt','job'],prompt:'judge'}]:[],outcomes:[],report:'D:\\work\\bridge\\report.md'}),results:[]}
  }
  throw Error(`unexpected stage ${opts.label}: ${cmd}`)
 }
 let result,error
 try{result=await new AsyncFunction('args','agent','phase','log','parallel','pipeline',source.replace(/^export /m,''))(args,agent,()=>{},()=>{},fns=>Promise.all(fns.map(fn=>fn())),()=>{})}catch(e){error=e}
 return {result:result?.result||result,error,calls,halted:result?.halted}
}
for(const kind of ['econ','math']){
 const args={mode:'bridge',out:'D:\\work',citationKey:'Key'}
 const discovery=await run(kind,args)
 assert(!discovery.error,`${kind} supports bridge mode: ${discovery.error}`)
 assert.equal(discovery.result?.bridge?.status,'done',`${kind} returns its Bridge summary: ${JSON.stringify(discovery.result)} ${discovery.calls.map(c=>c.label)}`)
 assert(discovery.calls.some(c=>/-Step plan/.test(c.cmd)))
 assert(!discovery.calls.some(c=>/-Apply\b/.test(c.cmd)))
 assert(discovery.calls.some(c=>/^Clean up:/m.test(c.prompt)),`${kind} cleans judge conversations`)
 const apply=await run(kind,{...args,bridgeReport:'r-frozen',bridgeApply:['b001']})
 assert(!apply.error,`${kind} apply succeeds: ${apply.error}`)
 assert.equal(apply.result?.bridge?.status,'done')
 assert.equal(apply.calls.length,1,`${kind} apply runs only its script command, with no probe or judge`)
 assert(/-ReportId 'r-frozen'.*-Accept 'b001'.*-Apply/.test(apply.calls[0].cmd))
 for(const selection of [{bridgeApply:'all'},{bridgeApplyEarlier:['b002']},{bridgeApplyEarlier:'all'}]){
  const r=await run(kind,{...args,bridgeReport:'r-frozen',...selection})
  assert(!r.error&&r.result?.bridge?.status==='done'&&r.calls.length===1,`${kind} supports explicit all and earlier-only acceptance`)
 }
 for(const value of [true,false,['b001',true],['b001','b001'],'b001']){
  const r=await run(kind,{...args,bridgeReport:'r-frozen',bridgeApply:value})
  assert(r.error||r.halted||r.result?.stage==='args'||r.result?.bridge?.status==='failed',`${kind} refuses malformed selection ${JSON.stringify(value)}`)
  assert(!r.calls.some(c=>c.label==='choose DeepSeek source'||/Run every job below/.test(c.prompt)),`${kind} does not probe or judge malformed acceptance`)
 }
 for(const extra of [{bridgeMax:1.5},{bridgeMax:'12'},{bridgeCandidates:9},{bridgeBooks:true},{bridgeReport:42,bridgeApply:'all'}]){
  const r=await run(kind,{...args,...extra})
  assert(r.error||r.halted||r.result?.stage==='args'||r.result?.bridge?.status==='failed',`${kind} rejects invalid Bridge argument types and bounds`)
 }
 assert(discovery.calls.filter(c=>/Run every job below/.test(c.prompt)).every(c=>/zettelkasten/.test(c.prompt)&&/never -Apply/.test(c.prompt)),`${kind} judges load the skills and have read-only approvals`)
 assert(discovery.calls.some(c=>/^Clean up:/m.test(c.prompt)&&/Keep the folder and its files/.test(c.prompt)),`${kind} retains review artifacts during cleanup`)
 const judged=await run(kind,args,{failJudge:true})
 assert.equal(judged.result?.bridge?.status,'failed',`${kind} fails the stage when a judge job fails`)
 assert(!judged.calls.some(c=>/-Step report/.test(c.cmd)),`${kind} publishes no report after a failed judge job`)
 assert(judged.calls.some(c=>/^Clean up:/m.test(c.prompt)),`${kind} cleans judge conversations after a failed judge job`)
 const failedPlan=await run(kind,args,{failPlan:true})
 assert(!failedPlan.calls.some(c=>/-Step report/.test(c.cmd))&&failedPlan.calls.some(c=>/^Clean up:/m.test(c.prompt)),`${kind} publishes no report after failed planning and still cleans up`)
 const missing=await run(kind,{...args,bridgeApply:'all'})
 assert(missing.error||missing.halted||missing.result?.stage==='args'||missing.result?.bridge?.status==='failed',`${kind} refuses acceptance without report: ${JSON.stringify(missing.result)}`)
 console.log(`ok: ${kind} discovery and explicit apply are separate, bounded paths`)
}
