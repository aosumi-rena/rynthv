class Automation extends Nested {
  constructor(data,parent,type){super(data,parent);this.type=type;}
  getType(){return this.type;}
  getInterpolationMethod(){return this.data.mode;}
  getDefinition(){return __rpc('automation.definition',[this.type]);}
  getAllPoints(){const p=this.data.points;return Array.from({length:p.length/2},(_,i)=>[p[2*i],p[2*i+1]]);}
  getPoints(begin,end){return this.getAllPoints().filter(p=>p[0]>=begin&&p[0]<=end);}
  add(time,value){time=__integer(time);if(!Number.isFinite(value))throw new RangeError('Invalid automation value');const points=this.getAllPoints().filter(p=>p[0]!==time);points.push([time,value]);points.sort((a,b)=>a[0]-b[0]);this.data.points=points.flat();return true;}
  remove(begin,end){end??=begin;const points=this.getAllPoints(),kept=points.filter(p=>p[0]<begin||p[0]>end);this.data.points=kept.flat();return kept.length!==points.length;}
  removeAll(){this.data.points=[];}
  get(time){return __rpc('automation.sample',[this.data,this.type,time,false]);}
  getLinear(time){return __rpc('automation.sample',[this.data,this.type,time,true]);}
  clone(){return new Automation(__cloneData(this.data),null,this.type);}
}
