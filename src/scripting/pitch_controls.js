class PitchControlPoint extends Nested{
  get type(){return 'PitchControlPoint';}
  isTemporary(){return !!this.data.temporary;}
  getPosition(){return this.data.pos;}setPosition(v){this.data.pos=__integer(v);this.parent?.sortPitchControls();}
  getPitch(){return this.data.pitch;}setPitch(v){this.data.pitch=Math.fround(__number(v));}
  getIndexInParent(){return this.parent?.pitchChildren().indexOf(this.data)??-1;}
}
class PitchControlCurve extends PitchControlPoint{
  get type(){return 'PitchControlCurve';}
  getPoints(){const p=this.data.points;return Array.from({length:p.length/2},(_,i)=>[p[2*i],p[2*i+1]]);}
  setPoints(points){
    if(!Array.isArray(points))throw new TypeError('Pitch control points must be an array');
    points=points.map(p=>{if(!Array.isArray(p)||p.length!==2)throw new TypeError('Each point must contain time and pitch');return [__integer(p[0]),Math.fround(__number(p[1]))];});
    points.sort((a,b)=>a[0]-b[0]);if(points.some((p,i)=>i&&p[0]===points[i-1][0]))throw new RangeError('Duplicate pitch control position');
    this.data.points=points.flat();
  }
  getValueAt(time){
    time=__number(time)-this.getPosition();const p=this.getPoints();if(!p.length)return this.getPitch();
    if(time<=p[0][0])return Math.fround(this.getPitch()+p[0][1]);const i=p.findIndex(v=>v[0]>=time);if(i<0)return Math.fround(this.getPitch()+p[p.length-1][1]);
    const a=p[i-1],b=p[i],f=Math.fround((time-a[0])/(b[0]-a[0]));
    const delta=Math.fround(b[1]-a[1]),value=Math.fround(a[1]+Math.fround(f*delta));
    return Math.fround(this.getPitch()+value);
  }
}
class RetakeList extends Nested{
  getNumTakes(){return this.data.takes.length;}
  generateTake(...values){
    const instrument=__instrumentObject(this);if(values.length!==(instrument?2:3))throw new TypeError(instrument?'Pitch and timbre variations are required':'Duration, pitch and timbre variations are required');
    const [duration,pitch,timbre]=instrument?[false,...values]:values;
    const result=__rpc('retake.generate',[this.data,!!duration,!!pitch,!!timbre]);
    this.data.activeTakeId=result.activeTakeId;this.data.takes=result.takes;return result.activeTakeId;
  }
  setActiveTake(id){id=__integer(id);__index(this.data.takes.findIndex(t=>t.id===id),this.getNumTakes());this.data.activeTakeId=id;}
  deleteTake(id){
    id=__integer(id);
    if(id===0)throw new RangeError('The default retake cannot be deleted');
    const i=this.data.takes.findIndex(t=>t.id===id);__index(i,this.getNumTakes());this.data.takes.splice(i,1);
    if(this.data.activeTakeId===id)this.data.activeTakeId=0;
  }
  getIndexInParent(){this.data;return 0;}
}
