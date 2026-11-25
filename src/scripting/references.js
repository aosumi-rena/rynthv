class Reference extends Nested{
  getTarget(){
    const data=this.data,owner=this.parent?.parent,project=owner?.data||__context.project;
    if(this._target?.deref())return this._target.deref();
    const library=project?.library.find(g=>g.uuid===this.data.groupID);
    if(library)return __wrap(Group,library,owner||__project);
    const track=project?.tracks.find(t=>t.mainGroup.uuid===this.data.groupID)||this.parent?.data.mainGroup&&this.parent.data;
    if(track&&track.mainGroup.uuid===this.data.groupID)return __wrap(Group,track.mainGroup,this.parent||__wrap(Track,track,__project));
    return undefined;
  }
  setTarget(g){if(!(g instanceof Group))throw new TypeError('NoteGroup required');if(this.data.groupID)throw new Error('Group target already set');this.data.groupID=g.getUUID();this._target=new WeakRef(g);}
  getTimeOffset(){return this.data.blickOffset||0;}setTimeOffset(v){this.data.blickOffset=__integer(v);}
  getPitchOffset(){return this.data.pitchOffset||0;}setPitchOffset(v){this.data.pitchOffset=__integer(v);}
  getOnset(){if(this.isInstrumental())return this.parent?.data.__audioRange?.[0]??this.getTimeOffset();return this.data.blickAbsoluteBegin||0;}
  getDuration(){return this.getEnd()-this.getOnset();}
  getEnd(){if(this.isInstrumental())return this.parent?.data.__audioRange?.[1]??this.getTimeOffset();if(this.data.blickAbsoluteEnd>=0)return this.data.blickAbsoluteEnd;const n=this.getTarget()?.data.notes||[];return Math.max(0,...n.map(n=>n.onset+n.duration))+this.getTimeOffset();}
  setTimeRange(onset,duration){onset=__integer(onset);duration=__integer(duration);if(duration<0)throw new RangeError('Invalid group duration');this.data.blickAbsoluteBegin=onset;this.data.blickAbsoluteEnd=onset+duration;}
  isMain(){return this.parent?.data.mainRef===this.data;}isInstrumental(){return !!this.parent?.data.__audioTrack;}
  isMuted(){return !!this.data.mute;}setMuted(v){this.data.mute=!!v;}
  getVoice(){
    const voice=__copy(this.data.voice||{}),instrument=__instrumentObject(this);
    voice.singers=voice.choirNumStems??1;delete voice.choirNumStems;
    if(instrument)voice.performanceDirection||='default';
    else {voice.spacing=voice.choirSeatingSeparation??Math.fround(.7);delete voice.choirSeatingSeparation;voice.vocalModeParams??={};}
    return voice;
  }
  setVoice(value){
    if(!value||typeof value!=='object'||Array.isArray(value))throw new TypeError('Voice attributes must be an object');
    const voice=__copy(value);
    if('singers' in voice){const n=__number(voice.singers);if(!Number.isInteger(n)||n<1||n>16)throw new RangeError('Invalid singer count');voice.choirNumStems=n;delete voice.singers;}
    if('spacing' in voice){voice.choirSeatingSeparation=Math.fround(__number(voice.spacing));delete voice.spacing;}
    if(voice.performanceDirection==='default')voice.performanceDirection='';
    Object.assign(this.data.voice??={},voice);
  }
  clone(){const copy=super.clone(),target=this.getTarget();if(target)copy._target=new WeakRef(target);return copy;}
}
class Track extends Nested{
  children(){const refs=this.data.groups.map(r=>__wrap(Reference,r,this));refs.sort((a,b)=>a.getOnset()-b.getOnset());return [this.data.mainRef].concat(refs.map(r=>r.data));}
  getName(){return this.data.name;}setName(v){this.data.name=String(v);}
  getNumGroups(){return 1+this.data.groups.length;}
  getGroupReference(i){return __wrap(Reference,this.children()[__index(i,this.getNumGroups())],this);}
  addGroupReference(r){if(!(r instanceof Reference)||r.parent)throw new Error('Group reference already belongs to a track');r.parent=this;this.data.groups.push(r.data);return this.children().indexOf(r.data);}
  removeGroupReference(i){if(i===0)throw new Error('Cannot remove main group');const d=this.children()[__index(i,this.getNumGroups())];this.data.groups.splice(this.data.groups.indexOf(d),1);__destroy(d);}
  getDisplayColor(){return this.data.dispColor==='00000000'?0:this.data.dispColor;}setDisplayColor(v){if(typeof v!=='string'||!/^#?(?:[0-9a-f]{6}|[0-9a-f]{8})$/i.test(v))throw new TypeError('A hex color is required');v=v.replace(/^#/,'');this.data.dispColor=(v.length===6?'ff'+v:v).toLowerCase();}
  getDisplayOrder(){return this.data.dispOrder;}
  getDuration(){return Math.max(0,...this.children().map(r=>__wrap(Reference,r,this).getEnd()));}
  getMixer(){return __wrap(TrackMixer,this.data.mixer,this);}
  isBounced(){return !!this.data.renderEnabled;}setBounced(v){this.data.renderEnabled=!!v;}
}
class TrackMixer extends Nested{
  getGainDecibel(){return this.data.gainDecibel||0;}setGainDecibel(v){this.data.gainDecibel=__number(v);}
  getPan(){return this.data.pan||0;}setPan(v){v=__number(v);if(v< -1||v>1)throw new RangeError('Invalid pan');this.data.pan=v;}
  isMuted(){return !!this.data.mute;}setMuted(v){this.data.mute=!!v;}isSolo(){return !!this.data.solo;}setSolo(v){this.data.solo=!!v;}
  getIndexInParent(){this.data;return 0;}
}
