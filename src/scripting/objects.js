function __rpc(method,args){
  const reply=JSON.parse(__host(JSON.stringify({method:method,args:args||[]})));
  if(reply.error)throw new Error(reply.error);return reply.value;
}
function __copy(value){return value===undefined?undefined:JSON.parse(JSON.stringify(value));}
function __number(value){
  const number=typeof value==='number'?value:typeof value==='string'?parseFloat(value)||0:typeof value==='boolean'?Number(value):0;
  if(!Number.isFinite(number))throw new RangeError('A finite number is required');return number;
}
function __integer(value){const number=typeof value==='string'?parseInt(value,10)||0:Math.trunc(__number(value));if(!Number.isFinite(number))throw new RangeError('A finite integer is required');return number;}
function __index(value,length){value=__integer(value);if(value<0||value>=length)throw new RangeError('Object index outside its parent');return value;}
const __nodes=new WeakMap();
function __state(data){let s=__nodes.get(data);if(!s){s={alive:true,parent:null,wrapper:null,type:null,data:new WeakRef(data),children:new Set(),managed:true,hostOwned:false};__nodes.set(data,s);}return s;}
function __killState(state){state.alive=false;for(const child of state.children)__killState(child);state.children.clear();state.parent?.children.delete(state);}
const __nodeFinalizers=new FinalizationRegistry(state=>{if(state.managed&&!state.wrapper?.deref())__killState(state);});
function __check(state){if(state.parent)__check(state.parent);if(!state.alive)throw new Error('Failed to access a deleted object');}
function __parent(state){const p=state.parent;if(!p)return null;__check(p);return p.wrapper?.deref()||__wrap(p.type,p.data.deref(),__parent(p));}
function __wrap(Type,data,parent,...args){if(!data)return undefined;const s=__state(data);__check(s);return s.wrapper?.deref()||new Type(data,parent,...args);}
function __destroy(data){
  if(!data||typeof data!=='object')return;const s=__nodes.get(data);if(s)__killState(s);
  for(const key of ['notes','pitchControls','groups','tracks','library'])for(const child of data[key]||[])__destroy(child);
  for(const key of ['mainGroup','mainRef','mixer','takes','time'])__destroy(data[key]);
  for(const key of ['parameters','vocalModes'])for(const child of Object.values(data[key]||{}))__destroy(child);
}
function __cloneData(data){
  const copy=__copy(data);
  function renew(v){
    if(!v||typeof v!=='object')return;
    if(v.__rynthvId)v.__rynthvId=__rpc('identity');if(v.uuid)v.uuid=__rpc('uuid');
    if(v.type==='point'||v.type==='curve')v.id=__rpc('identity');
    for(const key of ['notes','pitchControls','groups'])for(const child of v[key]||[])renew(child);
    if(v.mainGroup){renew(v.mainGroup);renew(v.mainRef);v.mainRef.groupID=v.mainGroup.uuid;}
  }
  renew(copy);return copy;
}
class Nested{
  constructor(data,parent){this._data=data;if(data){const s=__state(data);s.type=this.constructor;s.wrapper=new WeakRef(this);this.parent=parent;__nodeFinalizers.register(this,s);} }
  get data(){if(!this._data)throw new Error('No active project');__check(__state(this._data));return this._data;}
  get parent(){return __parent(__state(this.data));}
  set parent(v){const s=__state(this.data);s.parent?.children.delete(s);s.parent=v?__state(v.data):null;s.parent?.children.add(s);s.managed=!v&&!s.hostOwned;}
  get hostOwned(){return __state(this.data).hostOwned;}set hostOwned(v){const s=__state(this.data);s.hostOwned=!!v;s.managed=!s.parent&&!s.hostOwned;}
  getParent(){return this.parent||undefined;}isMemoryManaged(){return __state(this.data).managed;}
  getIndexInParent(){return this.parent?.children?.().indexOf(this.data)??-1;}
  getScriptData(key){return __copy((this.data.scriptData||{})[key]);}
  getScriptDataKeys(){return Object.keys(this.data.scriptData||{});}
  hasScriptData(key){return Object.prototype.hasOwnProperty.call(this.data.scriptData||{},key);}
  setScriptData(key,value){if(value===undefined)throw new TypeError('Script data must be JSON serializable');(this.data.scriptData??={})[key]=__copy(value);}
  removeScriptData(key){if(this.data.scriptData)delete this.data.scriptData[key];}clearScriptData(){delete this.data.scriptData;}
  clone(){return new this.constructor(__cloneData(this.data),null);}
}
class Note extends Nested{
  getOnset(){return this.data.onset;}getDuration(){return this.data.duration;}getEnd(){return this.getOnset()+this.getDuration();}
  setOnset(value){this.data.onset=__integer(value);this.parent?.sortNotes();}
  setDuration(value){value=__integer(value);if(value<=0)throw new RangeError('Note duration must be positive');this.data.duration=value;}
  setTimeRange(onset,duration){this.setDuration(duration);this.setOnset(onset);}
  getPitch(){return this.data.pitch;}setPitch(value){value=__integer(value);if(value<0||value>127)throw new RangeError('Invalid MIDI pitch');this.data.pitch=value;}
  getLyrics(){return this.data.lyrics;}setLyrics(value){this.data.lyrics=String(value);}
  getPhonemes(){return this.data.phonemes||'';}setPhonemes(value){this.data.phonemes=String(value);}
  getDetune(){return this.data.detune||0;}setDetune(value){this.data.detune=__integer(value);}
  getAttributes(){return Object.assign(__instrumentObject(this)?{muted:false,articulations:[],articulationsFixed:false}:{muted:false,evenSyllableDuration:true,languageOverride:'',phoneset:'',phonemes:[]},__copy(this.data.attributes||{}));}
  setAttributes(value){const a=this.data.attributes??={};for(const [k,v]of Object.entries(value)){if(v===null||typeof v==='number'&&Number.isNaN(v))delete a[k];else a[k]=__copy(v);}}
  getMusicalType(){return this.data.musicalType==='rap'?'rap':'sing';}
  setMusicalType(v){if(v!=='sing'&&v!=='rap')throw new RangeError('Note type must be sing or rap');this.data.musicalType=v==='sing'?'singing':'rap';}
  getLanguageOverride(){return this.data.attributes?.languageOverride||'';}setLanguageOverride(v){(this.data.attributes??={}).languageOverride=String(v);}
  getPitchAutoMode(){return this.data.attributes?.pitchAutoMode!==false&&this.data.attributes?.pitchAutoMode!==0;}
  setPitchAutoMode(v){(this.data.attributes??={}).pitchAutoMode=!!v;}
  getRapAccent(){return this.data.accent||'';}
  setRapAccent(v){v=String(v);if(!['','1','2','3','4','5'].includes(v))throw new RangeError('Invalid rap accent');this.data.accent=v;}
  getRetakes(){return __wrap(RetakeList,this.data.takes??={activeTakeId:0,takes:[{id:0,seedDuration:0,seedPitch:0,seedTimbre:0,liked:false}]},this);}
}
class Group extends Nested{
  children(){return this.data.notes;}sortNotes(){this.data.notes.sort((a,b)=>a.onset-b.onset);}
  getName(){return this.data.name;}setName(v){this.data.name=String(v);}getUUID(){return this.data.uuid;}
  getNumNotes(){return this.data.notes.length;}getNote(i){return __wrap(Note,this.data.notes[__index(i,this.getNumNotes())],this);}
  addNote(n){if(!(n instanceof Note)||n.parent)throw new Error('Note already belongs to a group');this.data.notes.push(n.data);n.parent=this;this.sortNotes();return n.getIndexInParent();}
  removeNote(i){__destroy(this.data.notes.splice(__index(i,this.getNumNotes()),1)[0]);}
  getParameter(type){
    type=String(type);const known=__parameterKeys.find(k=>k.toLowerCase()===type.toLowerCase());
    const mode=type.slice(0,10).toLowerCase()==='vocalmode_';if(!known&&!mode)throw new Error('Unknown automation parameter: '+type);
    type=known||'vocalMode_'+type.slice(10);const dict=mode?(this.data.vocalModes??={}):(this.data.parameters??={}),key=mode?type.slice(10):type;
    return __wrap(Automation,dict[key]??={mode:'linear',points:[]},this,type);
  }
  pitchChildren(){return this.data.pitchControls??=[];}sortPitchControls(){this.pitchChildren().sort((a,b)=>a.pos-b.pos);}
  getNumPitchControls(){return this.pitchChildren().length;}
  getPitchControl(i){const d=this.pitchChildren()[__index(i,this.getNumPitchControls())];return __wrap(d.type==='point'?PitchControlPoint:PitchControlCurve,d,this);}
  addPitchControl(c){if(!(c instanceof PitchControlPoint)&&!(c instanceof PitchControlCurve))throw new TypeError('Pitch control required');if(c.parent)throw new Error('Pitch control already has a parent');this.pitchChildren().push(c.data);c.parent=this;this.sortPitchControls();return this.pitchChildren().indexOf(c.data);}
  removePitchControl(i){__destroy(this.pitchChildren().splice(__index(i,this.getNumPitchControls()),1)[0]);}
}
const __parameterKeys=['pitchDelta','vibratoEnv','loudness','tension','breathiness','voicing','gender','toneShift','mouthOpening','rapIntonation','dynamics'];
