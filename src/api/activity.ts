import { invoke,isTauri } from "@tauri-apps/api/core";
import { toApiError } from "./contracts.ts";
export type ViewSource="grid"|"normal_viewer"|"quick_browser"|"search"|"history";
export interface Activation {id:string;viewerId:string;ordinal:number;mediaId:string;contentVersionId:number;source:ViewSource}
export type PlaybackPhase="playing"|"paused"|"waiting"|"seeking"|"ended"|"closed";
export interface PlaybackEvidence {sessionId:string;sequence:number;clockMs:number;position:number;duration:number|null;rate:number;phase:PlaybackPhase}
export interface ActivityResult {id:string;likeCount:number;viewCount:number;playCount:number}
export interface PlayResult {sessionId:string;qualified:boolean;watchedSeconds:number;finalized:boolean}
export interface StatisticsJob {id:string;state:string;phase:number;error:string|null}
function invalid():never {throw {code:"invalid_response",message:"活动服务返回的数据格式不正确。",retryable:true}}
const object=(v:unknown):v is Record<string,unknown>=>!!v&&typeof v==="object";
const count=(v:unknown):v is number=>typeof v==="number"&&Number.isSafeInteger(v)&&v>=0;
export function parseActivity(v:unknown):ActivityResult {if(!object(v)||typeof v.id!=="string"||!count(v.likeCount)||!count(v.viewCount)||!count(v.playCount))invalid();return v as unknown as ActivityResult;}
export function parsePlay(v:unknown):PlayResult {if(!object(v)||typeof v.sessionId!=="string"||typeof v.qualified!=="boolean"||typeof v.finalized!=="boolean"||typeof v.watchedSeconds!=="number"||!Number.isFinite(v.watchedSeconds)||v.watchedSeconds<0)invalid();return v as unknown as PlayResult;}
export function parseJob(v:unknown):StatisticsJob {if(!object(v)||typeof v.id!=="string"||!count(v.phase)||!['running','cleaning','complete','cancelled','failed'].includes(String(v.state))||!(v.error===null||typeof v.error==="string"))invalid();return v as unknown as StatisticsJob;}
async function call<T>(command:string,args:Record<string,unknown>):Promise<T>{if(!isTauri())throw {code:"desktop_required",message:"请在桌面端记录活动。",retryable:false};try{return await invoke<T>(command,args)}catch(e){throw toApiError(e)}}
export const activityApi={
 statistics:(mediaId:string)=>call<unknown>("get_statistics",{mediaId}).then(parseActivity),
 addLike:(mediaId:string,requestId:string)=>call<unknown>("add_like",{mediaId,requestId}).then(parseActivity),
 activate:(token:string,activation:Activation)=>call<unknown>("activate_media",{token,activation}).then(parseActivity),
 openPlayer:(activationId:string)=>call<string>("open_player_session",{activationId}),
 report:(evidence:PlaybackEvidence)=>call<unknown>("report_playback",{evidence}).then(parsePlay),
 close:(activationId:string)=>call<void>("close_activity",{activationId}),
 rebuild:(kind="all",mediaId:string|null=null,requestId=crypto.randomUUID())=>call<unknown>("rebuild_statistics",{requestId,kind,mediaId}).then(parseJob),
 job:(id:string)=>call<unknown>("statistics_job",{id}).then(parseJob),
 cancel:(id:string)=>call<void>("cancel_statistics_rebuild",{id}),
};
