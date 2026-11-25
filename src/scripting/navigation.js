class CoordinateSystem extends Nested{
  constructor(view,parent){super({},parent);this.view=view;this.hostOwned=true;}
  call(method,args){return __rpc('navigation',[this.view,method,args||[]]);}
  getTimePxPerUnit(){return this.call('timeScale');}getValuePxPerUnit(){return this.call('valueScale');}
  getTimeViewRange(){return this.call('timeRange');}getValueViewRange(){return this.call('valueRange');}
  setTimeLeft(t){this.call('timeLeft',[__number(t)]);}setTimeRight(t){this.call('timeRight',[__number(t)]);}
  setTimeScale(s){s=__number(s);if(s<=0)throw new RangeError('Scale must be positive');this.call('setTimeScale',[s]);}
  setValueCenter(v){this.call('valueCenter',[__number(v)]);}
  snap(t){return this.call('snap',[__number(t)]);}t2x(t){return this.call('t2x',[__number(t)]);}
  x2t(x){return this.call('x2t',[__number(x)]);}v2y(v){return this.call('v2y',[__number(v)]);}y2v(y){return this.call('y2v',[__number(y)]);}
  getIndexInParent(){return 0;}
}
class EditorView extends Nested{
  constructor(view){super({},null);this.hostOwned=true;this.view=view;this.selection=new Selection(view,this);this.navigation=new CoordinateSystem(view,this);}
  getNavigation(){return this.navigation;}getSelection(){return this.selection;}
  getCurrentTrack(){return __track();}getCurrentGroup(){return __reference();}
}
var __mainEditor=new EditorView('piano'),__arrangement=new EditorView('arrangement');
var __mainSelection=__mainEditor.selection,__arrangementSelection=__arrangement.selection;
var __playback={
  getPlayhead(){return __rpc('playback.status').seconds;},getStatus(){return __rpc('playback.status').status;},
  play(){__rpc('playback.play');},pause(){__rpc('playback.pause');},stop(){__rpc('playback.stop');},
  seek(t){__rpc('playback.seek',[__number(t)]);},
  loop(begin,end){begin=__number(begin);end=__number(end);if(begin<0||end<=begin)throw new RangeError('Invalid loop range');__rpc('playback.loop',[begin,end]);}
};
