function __bridgeKind(v){return v===SV||v===__playback||v instanceof Nested||v instanceof WidgetValue;}
for(const Type of [Nested,Note,Group,Reference,Track,TrackMixer,Project,TimeAxis,PitchControlPoint,PitchControlCurve,Automation,RetakeList,Selection]) {
  for(const key of Object.getOwnPropertyNames(Type.prototype)) {
    if(!/^(set|add|remove|clear|generateTake|deleteTake|select|unselect)/.test(key)||typeof Type.prototype[key]!=='function')continue;
    const method=Type.prototype[key],wrapped=function(...args){const result=method.apply(this,args);if(this instanceof Selection)__dirtySelection=true;else __dirtyProject=true;return result;};
    Object.defineProperty(wrapped,'length',{value:method.length});Type.prototype[key]=wrapped;
  }
}
function __bridgeMembers(v){
  const keys=new Set();for(let p=v;p&&p!==Object.prototype;p=Object.getPrototypeOf(p)){
    for(const name of Object.getOwnPropertyNames(p))if(name!=='constructor'&&!name.startsWith('_')&&name!=='data'&&name!=='parent'&&name!=='hostOwned'&&
      (typeof v[name]==='function'&&(v===SV||v===__playback||__publicMethods.has(name))||name==='QUARTER'||name==='type'))keys.add(name);
  }return Array.from(keys);
}
function __luaCall(object,name,args){
  const inputIndices=['getNote','removeNote','getPitchControl','removePitchControl','getTrack','removeTrack','getGroupReference','removeGroupReference','removeNoteGroup','getNoteGroup'];
  if(inputIndices.includes(name)&&!(name==='getNoteGroup'&&typeof args[0]==='string')&&args.length)args[0]=__integer(args[0])-1;
  if(name==='addNoteGroup'&&args.length>1)args[1]=__integer(args[1])-1;
  if(name==='setAttributes'&&args[0]){
    for(const key of ['phonemes','dur','strength','alt','articulations']){
      if(key in args[0])args[0][key]=__list(args[0][key]);
    }
  }
  for(const i of (name==='setPoints'||name==='setArticulations'||name==='selectPitchControls'||name==='unselectPitchControls'?[0]:name==='selectPoints'||name==='unselectPoints'?[1]:[]))if(args[i]&&Object.keys(args[i]).length===0)args[i]=[];
  const result=object[name].apply(object,args);
  return ['getIndexInParent','getDisplayOrder','addNote','addPitchControl','addTrack','addNoteGroup','addGroupReference'].includes(name)&&typeof result==='number'?result+1:result;
}
