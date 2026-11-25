class Project extends Nested{
  children(){return this.data.tracks;}
  getFileName(){return this===__project?__context.fileName||'':'';}
  getNumTracks(){return this.data.tracks.length;}
  getTrack(i){return __wrap(Track,this.data.tracks[__index(i,this.getNumTracks())],this);}
  addTrack(t){if(!(t instanceof Track)||t.parent)throw new Error('Track already belongs to a project');
    if(!t.data.name){let i=1;const stem=SV.T('Unnamed Track')+' ';while(this.data.tracks.some(t=>t.name===stem+i))i++;t.data.name=stem+i;}
    if(t.data.dispOrder<0)t.data.dispOrder=Math.max(-1,...this.data.tracks.map(t=>t.dispOrder))+1;
    if(t.data.dispColor==='00000000')t.data.dispColor=__rpc('track.color',[this.data]);
    this.data.tracks.push(t.data);t.parent=this;return this.getNumTracks()-1;}
  removeTrack(i){__destroy(this.data.tracks.splice(__index(i,this.getNumTracks()),1)[0]);}
  getNumNoteGroupsInLibrary(){return this.data.library.length;}
  getNoteGroup(id){const data=typeof id==='string'?this.data.library.find(g=>g.uuid===id):this.data.library[__index(id,this.getNumNoteGroupsInLibrary())];return __wrap(Group,data,this);}
  addNoteGroup(g,index){if(!(g instanceof Group)||g.parent)throw new Error('Group already belongs to a project');index=index===undefined?this.data.library.length:Math.min(Math.max(__integer(index),0),this.data.library.length);this.data.library.splice(index,0,g.data);g.parent=this;return index;}
  removeNoteGroup(i){
    const g=this.data.library[__index(i,this.getNumNoteGroupsInLibrary())];
    for(const t of this.data.tracks)t.groups=t.groups.filter(r=>{if(r.groupID!==g.uuid)return true;__destroy(r);return false;});
    this.data.library.splice(i,1);__destroy(g);
  }
  getTimeAxis(){return __wrap(TimeAxis,this.data.time,this);}
  getDuration(){let end=0;for(let i=0;i<this.getNumTracks();i++)end=Math.max(end,this.getTrack(i).getDuration());return end;}
  newUndoRecord(){if(this===__project){__commit();__rpc('undo.boundary');}}
}
class TimeAxis extends Nested{
  getSecondsFromBlick(v){return __rpc('time.seconds',[this.data,v]);}
  getBlickFromSeconds(v){return __rpc('time.blick',[this.data,v]);}
  tempo(t){return {position:t.position,positionSeconds:this.getSecondsFromBlick(t.position),bpm:t.bpm};}
  meter(m){return {position:m.index,positionBlick:this.getBlickFromMeasure(m.index),numerator:m.numerator,denominator:m.denominator};}
  getAllTempoMarks(){return this.data.tempo.map(t=>this.tempo(t));}
  getTempoMarkAt(v){return this.tempo(this.data.tempo.filter(t=>t.position<=v).slice(-1)[0]||this.data.tempo[0]);}
  addTempoMark(position,bpm){position=__integer(position);if(position<0||__number(bpm)<=0)throw new RangeError('Invalid tempo mark');this.data.tempo=this.data.tempo.filter(t=>t.position!==position);this.data.tempo.push({position,bpm});this.data.tempo.sort((a,b)=>a.position-b.position);}
  removeTempoMark(position){if(position===0)return false;const old=this.data.tempo.length;this.data.tempo=this.data.tempo.filter(t=>t.position!==position);return old!==this.data.tempo.length;}
  getAllMeasureMarks(){return this.data.meter.map(m=>this.meter(m));}
  getMeasureMarkAt(i){return this.meter(this.data.meter.filter(m=>m.index<=i).slice(-1)[0]||this.data.meter[0]);}
  getMeasureMarkAtBlick(v){return this.getMeasureMarkAt(this.getMeasureAt(v));}
  getBlickFromMeasure(i){return __rpc('time.measure',[this.data,i]);}
  getMeasureAt(v){return __rpc('time.measureAt',[this.data,v]);}
  addMeasureMark(index,numerator,denominator){
    index=__integer(index);if(index<0||!Number.isInteger(numerator)||numerator<1||numerator>64||!Number.isInteger(denominator)||denominator<1||denominator>64||(denominator&(denominator-1)))throw new RangeError('Invalid measure mark');
    this.data.meter=this.data.meter.filter(m=>m.index!==index);this.data.meter.push({index,numerator,denominator});this.data.meter.sort((a,b)=>a.index-b.index);
  }
  removeMeasureMark(index){if(index===0)return false;const n=this.data.meter.length;this.data.meter=this.data.meter.filter(m=>m.index!==index);return n!==this.data.meter.length;}
  getIndexInParent(){this.data;return 0;}
}
var __project=new Project(__context.project,null);if(__context.project)__project.hostOwned=true;
function __track(){const tracks=__context.project?.tracks||[];const i=tracks.findIndex(t=>t.__rynthvId===__context.track);return tracks.length?__project.getTrack(Math.max(i,0)):undefined;}
function __reference(){const t=__track();if(!t)return undefined;const i=t.children().findIndex(r=>r.__rynthvId===__context.reference);return t.getGroupReference(Math.max(i,0));}
function __selectionState(){return {notes:__context.selected||[],groups:__context.groups||[],pitchControls:__context.pitchControls||[],points:__context.points||{}};}
var __dirtyProject=false,__dirtySelection=false;
var __projectCommit=JSON.stringify(__context.project),__selectionCommit=JSON.stringify(__selectionState());
function __commit(){
  if(!__context.project||!__dirtyProject&&!__dirtySelection)return;
  const project=__dirtyProject?JSON.stringify(__context.project):__projectCommit,selection=JSON.stringify(__selectionState());
  if(project!==__projectCommit||selection!==__selectionCommit){
    const args=[__context.project,__dirtySelection?__selectionState():null,__context.revision];
    let result=__rpc('commit',args);
    if(result.needBase){args.push(JSON.parse(__projectCommit));result=__rpc('commit',args);}
    __context.revision=result.revision;
    if(result.project)__reconcile(__context.project,result.project);
    __projectCommit=result.project?JSON.stringify(__context.project):project;__selectionCommit=selection;
  }
  __dirtyProject=false;__dirtySelection=false;
}
function __reconcile(target,source){
  if(Array.isArray(target)&&Array.isArray(source)){
    const key=v=>v&&typeof v==='object'?(v.__rynthvId||v.uuid||(v.type&&v.id)):undefined;
    const old=new Map(target.filter(v=>key(v)).map(v=>[key(v),v]));
    const result=source.map((v,i)=>{const prev=key(v)?old.get(key(v)):target[i];return prev&&v&&typeof prev==='object'&&typeof v==='object'?__reconcile(prev,v):v;});
    for(const v of target)if(key(v)&&!result.includes(v))__destroy(v);target.splice(0,target.length,...result);return target;
  }
  if(target&&source&&typeof target==='object'&&typeof source==='object'&&!Array.isArray(target)&&!Array.isArray(source)){
    for(const k of Object.keys(target))if(!(k in source)){__destroy(target[k]);delete target[k];}
    for(const k of Object.keys(source)){const v=source[k];target[k]=target[k]&&v&&typeof target[k]==='object'&&typeof v==='object'?__reconcile(target[k],v):v;}
    return target;
  }
  return source;
}
function __sync(next){
  if(!next)return;
  if(next.revision!==undefined&&BigInt(next.revision)<BigInt(__context.revision))return;
  const prior=__selectionState();
  if(next.project)__reconcile(__context.project,next.project);
  for(const k of Object.keys(next))if(k!=='project')__context[k]=next[k];
  if(next.project)__projectCommit=JSON.stringify(__context.project);
  __selectionCommit=JSON.stringify(__selectionState());
  __notifySelections(prior,__selectionState());
}
