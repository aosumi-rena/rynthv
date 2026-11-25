var __finished=false,__timers=[],__timerSequence=0,__dialogs=new Map(),__dialogId=0;
function __runTimers(){
  const now=Date.now();__timers.sort((a,b)=>a.at-b.at||a.id-b.id);
  while(!__finished&&__timers.length&&__timers[0].at<=now){const timer=__timers.shift();timer.callback();__commit();}
  __publishPanel();
}
function __nextWait(){
  if(__finished)return -1;
  if(__panel||__dialogs.size||__mainSelection.selectionCallbacks.length||__mainSelection.clearCallbacks.length||__arrangementSelection.selectionCallbacks.length||__arrangementSelection.clearCallbacks.length)
    return Math.min(50,...__timers.map(t=>Math.max(0,t.at-Date.now())));
  return __timers.length?Math.min(50,...__timers.map(t=>Math.max(0,t.at-Date.now()))):-1;
}
function __dialogForm(form){form.widgets=__list(form.widgets).map(__widgetLists);return form;}
function __dialog(form){__commit();return __rpc('dialog',[__dialogForm(form)]);}
function __dialogAsync(form,callback){
  __commit();const id=++__dialogId;__dialogs.set(id,callback||function(){});
  __rpc('dialog.async',[__dialogForm(form),id]);
}
function __computed(kind,group,args){
  if(!(group instanceof Reference)||!group.parent)throw new TypeError('A group reference in the current project is required');
  __commit();return __rpc('computed',[kind,group.parent.data.__rynthvId,group.data.__rynthvId,args||[]]);
}
var SV={
  QUARTER:705600000,
  getProject(){return __project;},getMainEditor(){return __mainEditor;},getArrangement(){return __arrangement;},
  getPlayback(){return __playback;},
  getHostInfo(){return {osType:__context.os,osName:__context.osName||__context.os,hostName:'RynthV',hostVersion:'2.3.0',hostVersionNumber:0x020300,languageCode:__context.language};},
  getLanguageCode(){return __context.language;},
  T(text){
    if(typeof getTranslations==='function'){const translations=getTranslations(__context.language);if(Array.isArray(translations)){const item=translations.find(p=>p[0]===text);if(item)return item[1];}}
    return __context.translations?.[text]||__rpc('translation',[String(text)]);
  },
  create(type,...args){
    switch(type){
      case 'WidgetValue':return new WidgetValue();
      case 'Automation':if(typeof args[0]!=='string')throw new TypeError('An automation type is required');return new Automation({mode:'linear',points:[]},null,__canonicalParameter(args[0]));
      case 'PitchControlPoint':return new PitchControlPoint({type:'point',pos:0,pitch:0,id:__rpc('identity')},null);
      case 'PitchControlCurve':return new PitchControlCurve({type:'curve',pos:0,pitch:0,id:__rpc('identity'),points:[]},null);
      case 'NoteGroupReference':{const r=new Reference(__rpc('create',[type]),null);if(args[0])r.setTarget(args[0]);return r;}
      default:{const data=__rpc('create',[type]);if(__context.instrument){
        if(type==='Note')data.attributes={muted:false,articulations:[],articulationsFixed:false};
        if(type==='Track')data.instrument={selection:null,ambientSeed:0};
        const groups=type==='NoteGroup'?[data]:type==='Track'?[data.mainGroup]:type==='Project'?data.tracks.map(t=>t.mainGroup):[];
        for(const group of groups)for(const curve of Object.values(group.parameters))curve.mode='linear';
      }const Type={Note,NoteGroup:Group,Track,TrackMixer,TimeAxis,Project}[type];if(!Type)throw new Error('Unknown object type: '+type);return new Type(data,null);}
    }
  },
  finish(){__finished=true;__timers=[];__dialogs.clear();},
  setTimeout(milliseconds,callback){if(typeof callback!=='function')throw new TypeError('Callback required');milliseconds=__integer(milliseconds);if(milliseconds<0)throw new RangeError('Invalid delay');__timers.push({at:Date.now()+milliseconds,callback,id:++__timerSequence});},
  showCustomDialog:__dialog,showCustomDialogAsync:__dialogAsync,
  showMessageBox(title,message){__dialog({title,message,buttons:'Ok',widgets:[]});},
  showMessageBoxAsync(title,message,callback){__dialogAsync({title,message,buttons:'Ok',widgets:[]},()=>callback?.());},
  showOkCancelBox(title,message){return __dialog({title,message,buttons:'OkCancel',widgets:[]}).status==='Ok';},
  showYesNoCancelBox(title,message){return __dialog({title,message,buttons:'YesNoCancel',widgets:[]}).status.toLowerCase();},
  showInputBox(title,message,value){return __dialog({title,message,buttons:'Ok',widgets:[{name:'value',type:'TextBox',default:value||''}]}).answers.value;},
  showInputBoxAsync(title,message,value,callback){__dialogAsync({title,message,buttons:'Ok',widgets:[{name:'value',type:'TextBox',default:value||''}]},r=>callback(r.answers.value));},
  showOkCancelBoxAsync(title,message,callback){__dialogAsync({title,message,buttons:'OkCancel',widgets:[]},r=>callback(r.status==='Ok'));},
  showYesNoCancelBoxAsync(title,message,callback){__dialogAsync({title,message,buttons:'YesNoCancel',widgets:[]},r=>callback(r.status.toLowerCase()));},
  refreshSidePanel(){__panelRefresh=true;},
  getPhonemesForGroup(group){return __computed('phonemes',group);},
  getComputedAttributesForGroup(group){return __computed('attributes',group);},
  getComputedPitchForGroup(group,start,interval,count){return __computed('pitch',group,[__number(start),__number(interval),__integer(count)]);},
  getHostClipboard(){return __rpc('clipboard.get');},setHostClipboard(text){__rpc('clipboard.set',[String(text)]);},
  blick2Quarter(v){return v/this.QUARTER;},quarter2Blick(v){return Math.round(v*this.QUARTER);},
  blick2Seconds(v,bpm){return v/this.QUARTER*60/bpm;},seconds2Blick(v,bpm){return Math.round(v*bpm/60*this.QUARTER);},
  freq2Pitch(v){return 69+12*Math.log2(v/440);},pitch2freq(v){return 440*Math.pow(2,(v-69)/12);},pitch2Freq(v){return this.pitch2freq(v);},
  blackKey(pitch){return [1,3,6,8,10].includes(((Math.round(pitch)%12)+12)%12);},
  blickRoundDiv(v,interval){return Math.round(v/interval);},blickRoundTo(v,interval){return Math.round(v/interval)*interval;},
  print(...args){__rpc('print',[args.map(String).join('\t')]);},
  linear2dB(v){return 20*Math.log10(v);},dB2Linear(v){return Math.pow(10,v/20);}
};
function __canonicalParameter(type){return __parameterKeys.find(k=>k.toLowerCase()===type.toLowerCase())||(type.slice(0,10).toLowerCase()==='vocalmode_'?'vocalMode_'+type.slice(10):(()=>{throw new Error('Unknown automation parameter: '+type);})());}
