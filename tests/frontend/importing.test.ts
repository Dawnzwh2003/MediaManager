import assert from "node:assert/strict";
import { test } from "node:test";
import { parseImportJob, parseSources, parseStorage, importProgress, isImportRunning, bytesLabel } from "../../src/importing/model.ts";
const job = { id:"job",sourceRoot:"来源",storageRoot:"data",stage:"import",state:"importing",scannedFiles:10,newMedia:8,existingMedia:1,duplicateFiles:1,newTags:2,conflicts:0,estimatedStorageRequired:1024,processed:2,newlyImported:1,existingMediaReused:1,tagsAdded:2,failedFiles:0,currentFile:"中文.jpg",error:null,failures:[] };
test("import progress and preview counters come from bounded backend DTOs",()=>{
 assert.deepEqual(parseImportJob(job),job);assert.equal(importProgress(job),20);assert.equal(importProgress({...job,scannedFiles:0}),0);assert.equal(importProgress({...job,processed:999}),100);assert.ok(isImportRunning(job));assert.ok(!isImportRunning({...job,state:"preview"}));assert.equal(parseImportJob(null),null);
 assert.equal(bytesLabel(0),"0 B");assert.equal(bytesLabel(8),"8 B");assert.equal(bytesLabel(2048),"2 KiB");
 for(const bad of [{...job,processed:NaN},{...job,estimatedStorageRequired:-1},{...job,state:"unknown"},{...job,failures:Array(21).fill({relativePath:"x",error:"failed"})},{...job,newMedia:Number.MAX_SAFE_INTEGER+1}]) assert.throws(()=>parseImportJob(bad));
});
test("source tree pages and storage responses reject unbounded or unsafe payloads",()=>{
 const node={id:"tag",sourceId:"root",parentId:null,relativePath:"动物/猫",name:"猫",hasChildren:false,root:false};assert.deepEqual(parseSources({items:[node],nextAfter:null}).items,[node]);assert.throws(()=>parseSources({items:Array(101).fill(node),nextAfter:null}));assert.throws(()=>parseSources({items:[{...node,parentId:2}],nextAfter:null}));assert.deepEqual(parseStorage({mediaRoot:"data",availableBytes:1024}),{mediaRoot:"data",availableBytes:1024});assert.throws(()=>parseStorage({mediaRoot:"data",availableBytes:Infinity}));
});
