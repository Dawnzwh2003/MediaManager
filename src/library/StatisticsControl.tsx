import {useEffect,useState} from "react";
import {activityApi} from "../api/activity.ts";
import type {StatisticsJob} from "../api/activity.ts";
import {toApiError} from "../api/contracts.ts";
export function StatisticsControl({finished}:{finished:()=>void}){
 const [job,setJob]=useState<StatisticsJob|null>(null),[error,setError]=useState("");
 const running=job?.state==="running"||job?.state==="cleaning";
 useEffect(()=>{if(!running||!job)return;let alive=true;const timer=setInterval(()=>{void activityApi.job(job.id).then(value=>{if(alive){setJob(value);if(value.state==="complete")finished();}},e=>{if(alive)setError(toApiError(e).message)});},500);return()=>{alive=false;clearInterval(timer)}},[job?.id,running,finished]);
 return <span className="statistics-control"><button className="text-button" disabled={running} onClick={()=>{setError("");void activityApi.rebuild().then(setJob,e=>setError(toApiError(e).message))}}>重建统计</button>{running?<><span role="status">后台修复统计…</span><button onClick={()=>{void activityApi.cancel(job!.id).catch(e=>setError(toApiError(e).message))}}>取消</button></>:job&&<span role="status">{job.state==="complete"?"统计已修复":job.error??job.state}</span>}{error&&<span role="alert">{error}</span>}</span>
}
