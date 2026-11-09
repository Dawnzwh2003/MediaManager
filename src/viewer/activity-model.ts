import type { Activation,PlaybackEvidence,PlaybackPhase,PlayResult,ViewSource } from "../api/activity.ts";
// One controller per viewer lifetime; rerenders and resource fallback reuse its identities.
export class ActivationController {
 private ordinal=0; private current:Activation|null=null;
 private viewerId:string;private identity:()=>string;
 constructor(viewerId:string,identity:()=>string=()=>crypto.randomUUID()){this.viewerId=viewerId;this.identity=identity;}
 activate(mediaId:string,contentVersionId:number,source:ViewSource):Activation {
  if(this.current?.mediaId===mediaId&&this.current.contentVersionId===contentVersionId)return this.current;
  const origin=this.ordinal>0&&source==="grid"?"normal_viewer":source;
  this.current={id:this.identity(),viewerId:this.viewerId,ordinal:++this.ordinal,mediaId,contentVersionId,source:origin};return this.current;
 }
 leave(){this.current=null;}
}
export class PlaybackReporter {
 private sequence=0;private pending:PlaybackEvidence[]=[];private running:Promise<void>|null=null;
 private sessionId:string;private send:(e:PlaybackEvidence)=>Promise<PlayResult>;private clock:()=>number;
 constructor(sessionId:string,send:(e:PlaybackEvidence)=>Promise<PlayResult>,clock:()=>number=()=>performance.now()){this.sessionId=sessionId;this.send=send;this.clock=clock;}
 report(phase:PlaybackPhase,position:number,duration:number|null,rate:number):Promise<void>{
  // Never coalesce boundary evidence: pauses, seeks and buffering delimit eligible intervals.
  const evidence:PlaybackEvidence={sessionId:this.sessionId,sequence:++this.sequence,clockMs:this.clock(),position,duration,rate,phase};
  this.pending.push(evidence);return this.flush();
 }
 flush():Promise<void>{if(this.running)return this.running.then(()=>this.flush());if(!this.pending.length)return Promise.resolve();const task=this.drain();this.running=task;void task.then(()=>{this.running=null},()=>{this.running=null});return task;}
 private async drain(){while(this.pending.length){await this.send(this.pending[0]!);this.pending.shift();}}
}
