var __widgets=new Map(),__widgetId=0,__panel=false,__panelDirty=false,__panelRefresh=false,__panelLayout=null;
function __list(value){if(value===undefined||value&&typeof value==='object'&&!Array.isArray(value)&&!Object.keys(value).length)return [];if(!Array.isArray(value))throw new TypeError('An array is required');return value;}
function __widgetLists(widget){if(widget.choices!==undefined)widget.choices=__list(widget.choices);return widget;}
function __panelLists(panel){panel.rows=__list(panel.rows);for(const row of panel.rows)if(row.type==='Container')row.columns=__list(row.columns).map(__widgetLists);return panel;}
const __widgetFinalizers=new FinalizationRegistry(id=>__widgets.delete(id));
class WidgetValue{
  constructor(){this.id=++__widgetId;this.value=undefined;this.enabled=true;this.callback=null;__widgets.set(this.id,new WeakRef(this));__widgetFinalizers.register(this,this.id);}
  getValue(){return this.value;}setValue(v){this.value=v;__panelDirty=true;}
  getEnabled(){return this.enabled;}setEnabled(v){this.enabled=!!v;__panelDirty=true;}
  setValueChangeCallback(fn){if(typeof fn!=='function'&&fn!==undefined&&fn!==null)throw new TypeError('Callback required');this.callback=fn;}
  toJSON(){return {id:this.id,value:this.value,enabled:this.enabled};}
}
function __publishPanel(){
  if(!__panel)return;
  if(__panelRefresh){__panelLayout=__panelLists(getSidePanelSectionState());__panelRefresh=false;__panelDirty=true;}
  if(__panelDirty){__rpc('panel',[__panelLayout]);__panelDirty=false;}
}
function __event(event){
  if(__finished)return;
  if(event.type==='context')__sync(event.context);
  if(event.type==='widget'){
    const widget=__widgets.get(event.id)?.deref();if(widget&&widget.enabled){widget.value=event.value;widget.callback?.(widget.value);__panelDirty=true;}
  }
  if(event.type==='dialog'){
    const fn=__dialogs.get(event.id);__dialogs.delete(event.id);fn?.(event.result);
  }
}
