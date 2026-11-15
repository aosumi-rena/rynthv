var __finished=false,__timers=[],__timerSequence=0;
function __drainTimer(){
  if(__finished||!__timers.length)return false;
  __timers.sort((a,b)=>a.at-b.at||a.id-b.id);const timer=__timers.shift();
  __rpc('sleep',[Math.max(0,timer.at-Date.now())]);
  if(!__finished){timer.callback();__commit();}return !__finished&&__timers.length>0;
}
var SV={
  QUARTER:705600000,
  getProject(){return __project;},getMainEditor(){return {getCurrentTrack:__track,getCurrentGroup:__reference,getSelection:()=>__selection};},
  getPlayback(){return __playback;},getHostInfo(){return {osType:__context.os,osName:__context.os,hostName:'RynthV',hostVersion:'2.3.0',hostVersionNumber:0x020300,languageCode:__context.language};},
  getLanguageCode(){return __context.language;},
  T(text){if(typeof getTranslations==='function'){const translation=getTranslations(__context.language);if(Array.isArray(translation)){const item=translation.find(p=>p[0]===text);if(item)return item[1];}}return text;},
  create(type){const data=__rpc('create',[type]);switch(type){case'Note':return new Note(data,null);case'NoteGroup':return new Group(data,null);case'NoteGroupReference':return new Reference(data,null);case'Track':return new Track(data,null);default:throw new Error('Unsupported object: '+type);}},
  finish(){__finished=true;__timers=[];},
  setTimeout(milliseconds,callback){if(typeof callback!=='function')throw new TypeError('Callback required');if(!Number.isFinite(milliseconds)||milliseconds<0)throw new RangeError('Invalid delay');__timers.push({at:Date.now()+milliseconds,callback:callback,id:++__timerSequence});},
  showCustomDialog(form){return __rpc('dialog',[form]);},
  showCustomDialogAsync(form,callback){this.setTimeout(0,()=>callback(this.showCustomDialog(form)));},
  showMessageBox(title,message){this.showCustomDialog({title:title,message:message,buttons:'Ok',widgets:[]});},
  showMessageBoxAsync(title,message,callback){this.showCustomDialogAsync({title:title,message:message,buttons:'Ok',widgets:[]},()=>callback?.());},
  showOkCancelBox(title,message){return this.showCustomDialog({title:title,message:message,buttons:'OkCancel',widgets:[]}).status;},
  showYesNoCancelBox(title,message){return this.showCustomDialog({title:title,message:message,buttons:'YesNoCancel',widgets:[]}).status;},
  showInputBox(title,message,value){return this.showCustomDialog({title:title,message:message,buttons:'Ok',widgets:[{name:'value',type:'TextBox',default:value||''}]}).answers.value;},
  showInputBoxAsync(title,message,value,callback){this.setTimeout(0,()=>callback(this.showInputBox(title,message,value)));},
  showOkCancelBoxAsync(title,message,callback){this.setTimeout(0,()=>callback(this.showOkCancelBox(title,message)));},
  showYesNoCancelBoxAsync(title,message,callback){this.setTimeout(0,()=>callback(this.showYesNoCancelBox(title,message)));},
  getHostClipboard(){return __rpc('clipboard.get');},setHostClipboard(text){__rpc('clipboard.set',[String(text)]);},
  blick2Quarter(value){return value/this.QUARTER;},quarter2Blick(value){return Math.round(value*this.QUARTER);},
  blick2Seconds(value,bpm){return value/this.QUARTER*60/bpm;},seconds2Blick(value,bpm){return Math.round(value*bpm/60*this.QUARTER);},
  freq2Pitch(value){return 69+12*Math.log2(value/440);},pitch2freq(value){return 440*Math.pow(2,(value-69)/12);},
  blackKey(pitch){return [1,3,6,8,10].includes(((Math.round(pitch)%12)+12)%12);},
  blickRoundDiv(value,interval){return Math.round(value/interval);},blickRoundTo(value,interval){return Math.round(value/interval)*interval;},
  print(value){__rpc('print',[String(value)]);},
  linear2dB(value){return 20*Math.log10(value);},dB2Linear(value){return Math.pow(10,value/20);}
};
