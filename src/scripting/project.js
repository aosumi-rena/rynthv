class Project extends Nested {
  children(){return this.data.tracks;}
  getFileName(){return __context.fileName||'';}
  getNumTracks(){return this.data.tracks.length;}
  getTrack(index){const data=this.data.tracks[index];if(!data)throw new RangeError('Track index outside project');return new Track(data,this);}
  addTrack(track){if(!(track instanceof Track)||track.parent)throw new Error('Track already belongs to project');this.data.tracks.push(track.data);track.parent=this;return this.data.tracks.length-1;}
  removeTrack(index){if(!this.data.tracks[index]||this.data.tracks.length===1)throw new RangeError('Cannot remove track');this.data.tracks.splice(index,1);}
  getNumNoteGroupsInLibrary(){return this.data.library.length;}
  getNoteGroup(index){const data=typeof index==='string'?this.data.library.find(g=>g.uuid===index):this.data.library[index];if(!data)throw new RangeError('Group outside library');return new Group(data,this);}
  addNoteGroup(group){if(!(group instanceof Group)||group.parent)throw new Error('Group already belongs to project');this.data.library.push(group.data);group.parent=this;return this.data.library.length-1;}
  removeNoteGroup(index){if(!this.data.library[index])throw new RangeError('Group index outside library');const id=this.data.library[index].uuid;
    if(this.data.tracks.some(t=>t.groups.some(g=>g.groupID===id)))throw new Error('Group still has references');this.data.library.splice(index,1);}
  getTimeAxis(){return new TimeAxis(this.data.time,this);}
  getDuration(){let end=0;for(let t=0;t<this.getNumTracks();t++){let track=this.getTrack(t);for(let g=0;g<track.getNumGroups();g++)end=Math.max(end,track.getGroupReference(g).getEnd());}return end;}
  newUndoRecord(){__commit();}
}
class TimeAxis extends Nested {
  getSecondsFromBlick(value){return __rpc('time.seconds',[this.data,value]);}
  getBlickFromSeconds(value){return __rpc('time.blick',[this.data,value]);}
  getAllTempoMarks(){return __copy(this.data.tempo);}
  getTempoMarkAt(value){return __copy(this.data.tempo.filter(t=>t.position<=value).slice(-1)[0]||this.data.tempo[0]);}
  addTempoMark(position,bpm){position=__integer(position);if(position<0||!Number.isFinite(bpm)||bpm<=0)throw new RangeError('Invalid tempo mark');
    this.data.tempo=this.data.tempo.filter(t=>t.position!==position);this.data.tempo.push({position:position,bpm:bpm});this.data.tempo.sort((a,b)=>a.position-b.position);}
  removeTempoMark(position){if(position===0)return false;const old=this.data.tempo.length;this.data.tempo=this.data.tempo.filter(t=>t.position!==position);return old!==this.data.tempo.length;}
  getAllMeasureMarks(){return __copy(this.data.meter);}
  getMeasureMarkAt(index){return __copy(this.data.meter.filter(m=>m.index<=index).slice(-1)[0]||this.data.meter[0]);}
  getMeasureMarkAtBlick(blick){return this.getMeasureMarkAt(this.getMeasureAt(blick));}
  getBlickFromMeasure(index){return __rpc('time.measure',[this.data,index]);}
  getMeasureAt(blick){return __rpc('time.measureAt',[this.data,blick]);}
  addMeasureMark(index,numerator,denominator){index=__integer(index);if(index<0)throw new RangeError('Invalid measure');
    this.data.meter=this.data.meter.filter(m=>m.index!==index);this.data.meter.push({index:index,numerator:numerator,denominator:denominator});this.data.meter.sort((a,b)=>a.index-b.index);}
  removeMeasureMark(index){if(index===0)return false;const old=this.data.meter.length;this.data.meter=this.data.meter.filter(m=>m.index!==index);return old!==this.data.meter.length;}
}
var __project=new Project(__context.project,null);
function __track(){const index=__context.project.tracks.findIndex(t=>t.__rynthvId===__context.track);return __project.getTrack(Math.max(index,0));}
function __reference(){const track=__track();const index=track.children().findIndex(r=>r.__rynthvId===__context.reference);return track.getGroupReference(Math.max(index,0));}
var __selection={
  getSelectedNotes(){const group=__reference().getTarget();return group.data.notes.filter(n=>__context.selected.includes(n.__rynthvId)).map(n=>new Note(n,group));},
  hasSelectedNotes(){return this.getSelectedNotes().length>0;},
  hasSelectedContent(){return this.hasSelectedNotes();},
  hasUnfinishedEdits(){return false;},
  selectNote(note){if(!note.data.__rynthvId)throw new Error('Commit new note before selecting');if(!__context.selected.includes(note.data.__rynthvId))__context.selected.push(note.data.__rynthvId);},
  unselectNote(note){__context.selected=__context.selected.filter(id=>id!==note.data.__rynthvId);},
  clearNotes(){__context.selected=[];},clearAll(){this.clearNotes();}
};
var __playback={
  getPlayhead(){return __rpc('playback.status').seconds;},getStatus(){return __rpc('playback.status').status;},
  play(){__rpc('playback.play');},pause(){__rpc('playback.pause');},stop(){__rpc('playback.stop');},seek(seconds){__rpc('playback.seek',[seconds]);}
};
var __lastCommit=JSON.stringify([__context.project,__context.selected]);
function __commit(){
  if(!__context.project)return;
  const current=JSON.stringify([__context.project,__context.selected]);
  if(current!==__lastCommit){__rpc('commit',[__context.project,__context.selected]);__lastCommit=current;}
}
