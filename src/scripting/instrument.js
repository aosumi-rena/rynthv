const __articulations=['Staccato','Tenuto','Accent','Slur','Portamento','Tremolo','Breath','Harmonics','Scoop','Doit','Fall','Open','Stopped','Straight','Cup','Stem','Senza Sordino','Con Sordino','Arco','C. Legno','Pizz.','Bridge','Center','Fingerb.','Trill Major','Trill Minor'];
const __scales=['Major','NaturalMinor','HarmonicMinor','MelodicMinor','MajorPentatonic','MinorPentatonic','MajorBlues','MinorBlues','Dorian','Phrygian','Lydian','Mixolydian','Locrian','WholeTone','Octatonic'];
const __scaleNotes=['C','C#','D','D#','E','F','F#','G','G#','A','A#','B'];
function __instrumentObject(o){
  for(let p=o;p;p=p.parent)if(p instanceof Track)return !!p.data.instrument;
  return !!__context.instrument;
}
Object.assign(Note.prototype,{
  getDynamic(){return this.data.attributes?.dynamic;},
  setDynamic(value){(this.data.attributes??={}).dynamic=Math.fround(Math.min(1,Math.max(0,__number(value))));},
  getArticulations(){return (this.data.attributes?.articulations||[]).slice();},
  setArticulations(values){
    if(!Array.isArray(values))throw new TypeError('Articulations must be an array');
    const result=values.map(v=>{const name=__articulations.find(n=>n.toLowerCase()===String(v).toLowerCase());if(!name)throw new Error('Unknown articulation: '+v);return name;});
    Object.assign(this.data.attributes??={},{articulations:result,articulationsFixed:true});
  }
});
Object.assign(Group.prototype,{
  getAutomation(type){return this.getParameter(type);},
  getScale(){return __copy(this.data.musicalScale||{root:'C',type:'Major'});},
  setScale(scale){
    if(!scale||!__scaleNotes.includes(scale.root)||!__scales.includes(scale.type))throw new RangeError('Invalid musical scale');
    this.data.musicalScale={root:scale.root,type:scale.type};
  },
  getIndexInParent(){if(!this.parent)return -1;if(this.parent instanceof Project)return this.parent.data.library.indexOf(this.data);return this.parent instanceof Track&&this.parent.data.mainGroup===this.data?0:-1;}
});
Object.assign(SV,{
  articulationTypes(){return __articulations.slice();},
  automationTypes(){return __context.instrument?['vibratoEnv','loudness','breathiness']:__parameterKeys.filter(k=>k!=='dynamics');},
  parameterTypes(){return this.automationTypes();},
  dynamicLevels(){return ['ppp','pp','p','mp','mf','f','ff','fff'].map((name,i)=>({name,value:Math.fround([0.075,0.2,0.3,0.4,0.5,0.625,0.775,0.925][i])}));},
  performanceDirections(){return ['default','adagio','allegro','conFuoco','pop','ballad'];},
  scaleTypes(){return __scales.slice();},scaleNotes(){return __scaleNotes.slice();}
});
