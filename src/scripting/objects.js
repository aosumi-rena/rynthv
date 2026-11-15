function __rpc(method,args) {
  const reply=JSON.parse(__host(JSON.stringify({method:method,args:args||[]})));
  if(reply.error) throw new Error(reply.error);
  return reply.value;
}
function __copy(value){return JSON.parse(JSON.stringify(value));}
function __integer(n){if(!Number.isFinite(n))throw new TypeError('Nonfinite time');return Math.round(n);}
function __cloneData(data) {
  const copy=__copy(data);if(copy.__rynthvId)copy.__rynthvId=__rpc('identity');
  if(copy.uuid)copy.uuid=__rpc('uuid');
  if(copy.notes)copy.notes=copy.notes.map(__cloneData);
  if(copy.mainGroup){copy.mainGroup=__cloneData(copy.mainGroup);copy.mainRef=__cloneData(copy.mainRef);copy.mainRef.groupID=copy.mainGroup.uuid;copy.groups=copy.groups.map(__cloneData);}
  return copy;
}
class Nested {
  constructor(data,parent){this.data=data;this.parent=parent;}
  getParent(){return this.parent;}
  isMemoryManaged(){return !this.parent&&!(this instanceof Project);}
  getIndexInParent(){if(!this.parent)return -1;return this.parent.children().indexOf(this.data);}
  getScriptData(key){const value=(this.data.scriptData||{})[key];return value===undefined?undefined:__copy(value);}
  getScriptDataKeys(){return Object.keys(this.data.scriptData||{});}
  hasScriptData(key){return Object.prototype.hasOwnProperty.call(this.data.scriptData||{},key);}
  setScriptData(key,value){(this.data.scriptData??={})[key]=__copy(value);}
  removeScriptData(key){if(this.data.scriptData)delete this.data.scriptData[key];}
  clearScriptData(){delete this.data.scriptData;}
  clone(){return new this.constructor(__cloneData(this.data),null);}
}
class Note extends Nested {
  getOnset(){return this.data.onset;}
  getDuration(){return this.data.duration;}
  getEnd(){return this.getOnset()+this.getDuration();}
  setOnset(value){this.data.onset=__integer(value);this.parent?.sortNotes();}
  setDuration(value){value=__integer(value);if(value<=0)throw new RangeError('Note duration must be positive');this.data.duration=value;}
  setTimeRange(onset,duration){this.setDuration(duration);this.setOnset(onset);}
  getPitch(){return this.data.pitch;}
  setPitch(value){if(!Number.isInteger(value)||value<0||value>127)throw new RangeError('Invalid MIDI pitch');this.data.pitch=value;}
  getLyrics(){return this.data.lyrics;}
  setLyrics(value){this.data.lyrics=String(value);}
  getPhonemes(){return this.data.phonemes||'';}
  setPhonemes(value){this.data.phonemes=String(value);}
  getDetune(){return this.data.detune||0;}
  setDetune(value){this.data.detune=Number(value);}
  getAttributes(){return __copy(this.data.attributes||{});}
  setAttributes(value){for(const [key,v]of Object.entries(value)){if(v===null)delete this.data.attributes[key];else this.data.attributes[key]=__copy(v);}}
  getMusicalType(){return this.data.musicalType||'singing';}
  setMusicalType(value){this.data.musicalType=String(value);}
  getLanguageOverride(){return this.data.attributes.languageOverride||'';}
  setLanguageOverride(value){this.data.attributes.languageOverride=String(value);}
  getPitchAutoMode(){return this.data.attributes.pitchAutoMode??1;}
  setPitchAutoMode(value){this.data.attributes.pitchAutoMode=__integer(value);}
  getRapAccent(){return this.data.attributes.rapAccent??0;}
  setRapAccent(value){this.data.attributes.rapAccent=Number(value);}
}
class Group extends Nested {
  getIndexInParent(){if(!this.parent)return -1;return __context.project.library.indexOf(this.data);}
  children(){return this.data.notes;}
  sortNotes(){this.data.notes.sort((a,b)=>a.onset-b.onset);}
  getName(){return this.data.name;}
  setName(value){this.data.name=String(value);}
  getUUID(){return this.data.uuid;}
  getNumNotes(){return this.data.notes.length;}
  getNote(index){const data=this.data.notes[index];if(!data)throw new RangeError('Note index outside group');return new Note(data,this);}
  addNote(note){if(!(note instanceof Note)||note.parent)throw new Error('Note already belongs to a group');this.data.notes.push(note.data);note.parent=this;this.sortNotes();return note.getIndexInParent();}
  removeNote(index){if(!this.data.notes[index])throw new RangeError('Note index outside group');this.data.notes.splice(index,1);}
  getParameter(type){const modes=type.startsWith('vocalMode_');const dictionary=modes?(this.data.vocalModes??={}):this.data.parameters;
    const key=modes?type.slice('vocalMode_'.length):type;
    const data=dictionary[key]??={mode:'linear',points:[]};return new Automation(data,this,type);}
  getNumPitchControls(){return (this.data.pitchControls||[]).length;}
}
class Reference extends Nested {
  getTarget(){const project=__context.project;const data=project.library.concat(project.tracks.map(t=>t.mainGroup)).find(g=>g.uuid===this.data.groupID);
    if(!data)throw new Error('Missing note group');return new Group(data,__project);}
  setTarget(group){if(!(group instanceof Group)||!group.parent)throw new Error('Group must belong to project');if(this.data.groupID&&this.data.groupID!==group.getUUID())throw new Error('Group target already set');this.data.groupID=group.getUUID();}
  getTimeOffset(){return this.data.blickOffset||0;}
  setTimeOffset(value){this.data.blickOffset=__integer(value);}
  getPitchOffset(){return this.data.pitchOffset||0;}
  setPitchOffset(value){this.data.pitchOffset=__integer(value);}
  getOnset(){if(this.isInstrumental())return this.parent.data.__audioRange?.[0]||0;const notes=this.getTarget().data.notes;
    const begin=notes.length?Math.min(...notes.map(n=>n.onset))+this.getTimeOffset():this.getTimeOffset();return Math.max(begin,this.data.blickAbsoluteBegin||0);}
  getDuration(){return this.getEnd()-this.getOnset();}
  getEnd(){if(this.isInstrumental())return this.parent.data.__audioRange?.[1]||0;if(this.data.blickAbsoluteEnd>=0)return this.data.blickAbsoluteEnd;
    const notes=this.getTarget().data.notes;return Math.max(0,...notes.map(n=>n.onset+n.duration))+this.getTimeOffset();}
  setTimeRange(onset,duration){this.data.blickAbsoluteBegin=__integer(onset);this.data.blickAbsoluteEnd=__integer(onset+duration);}
  isMain(){return this.parent?.data.mainRef===this.data;}
  isInstrumental(){return !!this.parent?.data.__audioTrack;}
  isMuted(){return !!this.data.mute;}
  setMuted(value){this.data.mute=!!value;}
  getVoice(){return __copy(this.data.voice||{});}
  setVoice(value){Object.assign(this.data.voice??={},__copy(value));}
}
class Track extends Nested {
  children(){const refs=this.data.groups.map(r=>new Reference(r,this));refs.sort((a,b)=>a.getOnset()-b.getOnset());return [this.data.mainRef].concat(refs.map(r=>r.data));}
  getName(){return this.data.name;}
  setName(value){this.data.name=String(value);}
  getNumGroups(){return 1+this.data.groups.length;}
  getGroupReference(index){const data=this.children()[index];if(!data)throw new RangeError('Group reference index outside track');return new Reference(data,this);}
  addGroupReference(reference){if(!(reference instanceof Reference)||reference.parent)throw new Error('Group reference already belongs to a track');reference.parent=this;this.data.groups.push(reference.data);return this.children().indexOf(reference.data);}
  removeGroupReference(index){if(index<1||index>this.data.groups.length)throw new RangeError('Cannot remove main group');const data=this.children()[index];this.data.groups.splice(this.data.groups.indexOf(data),1);}
  getDisplayColor(){return this.data.dispColor;}
  setDisplayColor(value){this.data.dispColor=value;}
  getDisplayOrder(){return this.data.dispOrder;}
  setDisplayOrder(value){this.data.dispOrder=__integer(value);}
  getDuration(){return Math.max(0,...this.children().map(r=>new Reference(r,this).getEnd()));}
  getMixer(){return new TrackMixer(this.data.mixer,this);}
}
class TrackMixer extends Nested {
  getGainDecibel(){return this.data.gainDecibel||0;}
  setGainDecibel(value){if(!Number.isFinite(value))throw new RangeError('Invalid gain');this.data.gainDecibel=value;}
  getPan(){return this.data.pan||0;}
  setPan(value){if(!Number.isFinite(value)||value< -1||value>1)throw new RangeError('Invalid pan');this.data.pan=value;}
  isMuted(){return !!this.data.mute;}setMuted(value){this.data.mute=!!value;}
  isSolo(){return !!this.data.solo;}setSolo(value){this.data.solo=!!value;}
  getIndexInParent(){return 0;}
}
