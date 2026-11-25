const __arityOptional={addNoteGroup:[1,2],remove:[1,2],simplify:[2,3],create:[1,2],generateTake:[2,3],showMessageBoxAsync:[2,3],print:[1,Infinity]};
const __mathArguments={blick2Quarter:1,quarter2Blick:1,blick2Seconds:2,seconds2Blick:2,freq2Pitch:1,pitch2Freq:1,pitch2freq:1,blackKey:1,blickRoundDiv:2,blickRoundTo:2,linear2dB:1,dB2Linear:1};
function __guardMethods(object,host){
  for(const name of Object.getOwnPropertyNames(object)){
    const descriptor=Object.getOwnPropertyDescriptor(object,name),method=descriptor.value;
    if(typeof method!=='function'||name==='constructor'||name.startsWith('_')||!host&&!__publicMethods.has(name))continue;
    const [min,max]=__arityOptional[name]||[method.length,method.length];
    const wrapped=function(...args){
      if(args.length<min||args.length>max)throw new TypeError(name+' expects '+(min===max?min:min+'..'+max)+' arguments; received '+args.length);
      if(host&&__mathArguments[name])for(let i=0;i<__mathArguments[name];i++)args[i]=__number(args[i]);
      return method.apply(this,args);
    };
    Object.defineProperty(wrapped,'length',{value:method.length});object[name]=wrapped;
  }
}
for(const Type of [Nested,Note,Group,Reference,Track,TrackMixer,Project,TimeAxis,PitchControlPoint,PitchControlCurve,Automation,RetakeList,Selection,EditorView,CoordinateSystem,WidgetValue])__guardMethods(Type.prototype,false);
__guardMethods(SV,true);__guardMethods(__playback,true);
