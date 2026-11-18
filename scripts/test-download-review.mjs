import fs from 'node:fs'
import assert from 'node:assert/strict'
import ts from 'typescript'
const code = ts.transpileModule(fs.readFileSync(new URL('../src/lib/download-review.ts', import.meta.url),'utf8'), {compilerOptions:{module:ts.ModuleKind.ES2022,target:ts.ScriptTarget.ES2022}}).outputText
const {reviewTracks, taskForTrack, deliveredAudio, knownDowngrade} = await import('data:text/javascript;base64,'+Buffer.from(code).toString('base64'))
const track = {variants:[{source:'netease',id:'1'}]}
const task = {id:'a',source:'netease',sourceId:'1',createdAt:1,state:'completed',quality:'lossless',format:'mp3',bitrate:128}
assert.equal(taskForTrack(track,[{...task,source:'qq'}]),undefined)
assert.equal(taskForTrack(track,[{...task,sourceId:'2'}]),undefined)
assert.equal(taskForTrack(track,[task,{...task,id:'retry',state:'failed',createdAt:2}]).id,'retry')
const review = reviewTracks([track,{variants:[{source:'qq',id:'2'}]}],[task])
assert.equal(review.completed,1);assert.equal(review.missing.length,1);assert.equal(review.failed.length,0)
assert.equal(reviewTracks([track],[{...task,state:'failed'}]).failed.length,1)
assert.equal(reviewTracks([track],[{...task,state:'paused'}]).active,1)
assert.equal(deliveredAudio({...task,state:'queued'}),null)
assert.equal(deliveredAudio(task),'MP3 · 128 kbps')
assert.equal(knownDowngrade(task),true)
assert.equal(knownDowngrade({...task,format:'flac'}),false)
assert.equal(knownDowngrade({...task,state:'queued'}),false)
assert.equal(knownDowngrade({...task,quality:'exquisite',bitrate:192}),true)
assert.equal(knownDowngrade({...task,quality:'standard'}),false)
console.log('Download review regression checks passed')
