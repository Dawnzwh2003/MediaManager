import "./activity.css";
import {useEffect,useState} from "react";
import {activityApi} from "../api/activity.ts";
import {toApiError} from "../api/contracts.ts";
export function LikeButton({mediaId,initialCount=0}:{mediaId:string;initialCount?:number}){
 const [count,setCount]=useState(initialCount),[error,setError]=useState("");
 const [failed,setFailed]=useState<string[]>([]);
 useEffect(()=>setCount(initialCount),[mediaId,initialCount]);
 useEffect(()=>{const listener=(event:Event)=>{const value=(event as CustomEvent<{id:string;count:number}>).detail;if(value.id===mediaId)setCount(current=>Math.max(current,value.count))};window.addEventListener("like-recorded",listener);return()=>window.removeEventListener("like-recorded",listener)},[mediaId]);
 const send=(requestId:string)=>{void activityApi.addLike(mediaId,requestId).then(result=>{setFailed(current=>current.filter(id=>id!==requestId));window.dispatchEvent(new CustomEvent("like-recorded",{detail:{id:mediaId,count:result.likeCount}}));},e=>{setFailed(current=>current.includes(requestId)?current:[...current,requestId]);setError(toApiError(e).message);});};
 return <span className="like-action"><button type="button" data-like={mediaId} aria-label={`Like ${mediaId}`} title="Like (L)，每次操作新增一次" onClick={()=>send(crypto.randomUUID())}>♡ {count.toLocaleString()}</button>{failed.length>0&&<span role="alert">{error}<button onClick={()=>{for(const id of failed)send(id)}}>重试 Like</button></span>}</span>;
}
