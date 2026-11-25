class Automation extends Nested {
  constructor(data,parent,type){super(data,parent);this.type=type;}
  getType(){return this.type;}
  getInterpolationMethod(){return (this.data.mode||'linear').toLowerCase();}
  getDefinition(){return __rpc('automation.definition',[this.type,__instrumentObject(this)]);}
  getAllPoints(){const p=this.data.points;return Array.from({length:p.length/2},(_,i)=>[p[2*i],p[2*i+1]]);}
  getPoints(begin,end){begin=__integer(begin);end=__integer(end);return this.getAllPoints().filter(p=>p[0]>=begin&&p[0]<=end);}
  add(time,value){time=__integer(time);value=Math.fround(__number(value));const old=this.getAllPoints(),points=old.filter(p=>p[0]!==time);const added=points.length===old.length;points.push([time,value]);points.sort((a,b)=>a[0]-b[0]);this.data.points=points.flat();return added;}
  remove(begin,end){begin=__integer(begin);end=end===undefined?begin:__integer(end);const points=this.getAllPoints(),kept=points.filter(p=>p[0]<begin||p[0]>end);this.data.points=kept.flat();return kept.length!==points.length;}
  removeAll(){this.data.points=[];}
  get(time){return __rpc('automation.sample',[this.data,this.type,__integer(time),false]);}
  getLinear(time){return __rpc('automation.sample',[this.data,this.type,__integer(time),true]);}
  clone(){return new Automation(__cloneData(this.data),null,this.type);}
  getIndexInParent(){if(!this.parent)return -1;const index=__parameterKeys.indexOf(this.type);return index>=0?index:__parameterKeys.length+Object.keys(this.parent.data.vocalModes||{}).indexOf(this.type.slice(10));}
  simplify(begin,end,threshold){
    begin=__integer(begin);end=__integer(end);
    threshold=threshold===undefined?0.002:__number(threshold);if(threshold<0)throw new RangeError('Invalid simplification threshold');
    const all=this.getAllPoints(),p=all.filter(v=>v[0]>=begin&&v[0]<=end);if(p.length<3)return false;
    const keep=new Set([0,p.length-1]),pending=[[0,p.length-1]];
    while(pending.length){const [a,b]=pending.pop();let error=threshold,index=-1;
      for(let i=a+1;i<b;i++){const fraction=(p[i][0]-p[a][0])/(p[b][0]-p[a][0]);const distance=Math.abs(p[i][1]-(p[a][1]*(1-fraction)+p[b][1]*fraction));if(distance>error){error=distance;index=i;}}
      if(index>=0){keep.add(index);pending.push([a,index],[index,b]);}
    }
    const reduced=p.filter((_,i)=>keep.has(i));if(reduced.length===p.length)return false;
    this.data.points=all.filter(v=>v[0]<begin).concat(reduced,all.filter(v=>v[0]>end)).flat();return true;
  }
}
