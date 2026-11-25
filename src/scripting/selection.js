class Selection extends Nested{
  constructor(view,parent){super({},parent);this.view=view;this.hostOwned=true;this.selectionCallbacks=[];this.clearCallbacks=[];}
  getIndexInParent(){return 0;}
  registerSelectionCallback(fn){if(typeof fn!=='function')throw new TypeError('Callback required');this.selectionCallbacks.push(fn);}
  registerClearCallback(fn){if(typeof fn!=='function')throw new TypeError('Callback required');this.clearCallbacks.push(fn);}
  hasUnfinishedEdits(){return !!__context.editing;}
  getSelectedGroups(){
    return (__context.groups||[]).filter(pair=>this.view==='arrangement'||pair[0]===__context.track).map(([track,id])=>{
      const t=__context.project.tracks.find(t=>t.__rynthvId===track);if(!t)return undefined;
      const wrapper=__wrap(Track,t,__project),r=wrapper.children().find(r=>r.__rynthvId===id);return __wrap(Reference,r,wrapper);
    }).filter(Boolean);
  }
  hasSelectedGroups(){return this.getSelectedGroups().length>0;}
  selectGroup(reference){if(!(reference instanceof Reference)||!(reference.parent instanceof Track))throw new TypeError('A project group reference is required');if(this.view!=='arrangement'&&reference.parent.data.__rynthvId!==__context.track)throw new Error('Group is not in the current track');const p=[reference.parent.data.__rynthvId,reference.data.__rynthvId];const a=__context.groups??=[];if(!a.some(x=>x[0]===p[0]&&x[1]===p[1]))a.push(p);}
  unselectGroup(r){const a=__context.groups||[];__context.groups=a.filter(x=>x[1]!==r.data.__rynthvId);return a.length!==__context.groups.length;}
  clearGroups(){const a=__context.groups||[];__context.groups=this.view==='arrangement'?[]:a.filter(x=>x[0]!==__context.track);return a.length!==__context.groups.length;}
  getSelectedNotes(){if(this.view==='arrangement')return [];const g=__reference()?.getTarget();return g?(__context.selected||[]).map(id=>g.data.notes.find(n=>n.__rynthvId===id)).filter(Boolean).map(n=>__wrap(Note,n,g)):[];}
  hasSelectedNotes(){return this.getSelectedNotes().length>0;}
  selectNote(n){if(!(n instanceof Note)||n.parent?.data!==__reference()?.getTarget()?.data)throw new Error('Note is not in the current note group');const a=__context.selected??=[];if(!a.includes(n.data.__rynthvId))a.push(n.data.__rynthvId);}
  unselectNote(n){const a=__context.selected||[];__context.selected=a.filter(id=>id!==n.data.__rynthvId);return a.length!==__context.selected.length;}
  clearNotes(){const changed=this.hasSelectedNotes();__context.selected=[];return changed;}
  getSelectedPitchControls(){const g=__reference()?.getTarget();return g?g.pitchChildren().filter(c=>(__context.pitchControls||[]).includes(c.id)).map(c=>__wrap(c.type==='point'?PitchControlPoint:PitchControlCurve,c,g)):[];}
  hasSelectedPitchControls(){return this.getSelectedPitchControls().length>0;}
  selectPitchControls(controls){const g=__reference()?.getTarget();for(const c of controls){if(c.parent?.data!==g?.data)throw new Error('Pitch control is not in the current group');const a=__context.pitchControls??=[];if(!a.includes(c.data.id))a.push(c.data.id);}}
  unselectPitchControls(controls){__context.pitchControls=(__context.pitchControls||[]).filter(id=>!controls.some(c=>c.data.id===id));}
  clearPitchControls(){const changed=this.hasSelectedPitchControls();__context.pitchControls=[];return changed;}
  getSelectedPoints(type){return (__context.points||{})[__reference()?.getTarget()?.getParameter(type).getType()]?.slice()||[];}
  selectPoints(type,positions){type=__reference().getTarget().getParameter(type).getType();const a=(__context.points??={})[type]??=[];for(const p of positions){const v=__integer(p);if(!a.includes(v))a.push(v);}a.sort((a,b)=>a-b);}
  unselectPoints(type,positions){type=__reference().getTarget().getParameter(type).getType();positions=positions.map(__integer);if(__context.points?.[type])__context.points[type]=__context.points[type].filter(v=>!positions.includes(v));}
  hasSelectedContent(){return this.hasSelectedGroups()||(this.view!=='arrangement'&&(this.hasSelectedNotes()||this.hasSelectedPitchControls()||Object.values(__context.points||{}).some(a=>a.length)));}
  clearAll(){const changed=this.hasSelectedContent();this.clearGroups();if(this.view!=='arrangement'){this.clearNotes();this.clearPitchControls();__context.points={};}return changed;}
}
function __notifySelections(before,after){
  for(const s of [__mainSelection,__arrangementSelection]){
    const kinds=s.view==='arrangement'?['groups']:['notes','groups','pitchControls','points'];
    for(const kind of kinds){
      const a=before[kind]||[],b=after[kind]||[];if(JSON.stringify(a)===JSON.stringify(b))continue;
      const count=x=>Array.isArray(x)?x.length:Object.values(x).reduce((n,v)=>n+v.length,0);
      if(!count(b))for(const fn of s.clearCallbacks)fn(kind==='points'?'parameters':kind);
      else{const old=new Set((Array.isArray(a)?a:Object.values(a).flat()).map(JSON.stringify));const added=(Array.isArray(b)?b:Object.values(b).flat()).some(v=>!old.has(JSON.stringify(v)));for(const fn of s.selectionCallbacks)fn(kind==='pitchControls'?'pitchControl':kind==='groups'?'group':kind==='points'?'point':'note',added);}
    }
  }
}
