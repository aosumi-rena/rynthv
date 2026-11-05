use super::{Result,StdError,ensure,StdBank,bank::Address,models::Encoding,score::Score};
use std::{collections::{HashMap,HashSet},sync::{Arc,atomic::AtomicBool}};
#[derive(Clone)]
pub(super) struct Pair {pub first:Address,pub second:Address,pub a:[f32;3],pub b:[f32;3],pub rank:f32,pub local:f32}
pub(super) fn folded(category:&str)->&str{if category=="diphthong"{"vowel"}else{category}}
fn mean(values:&[f32],power:f32)->f32{(values.iter().map(|x|x.powf(power)).sum::<f32>()/values.len()as f32).powf(1./power)}
fn context_encoding(bank:&StdBank,context:[usize;3],cache:&mut HashMap<[usize;3],Arc<Encoding>>)->Result<Arc<Encoding>>{
 if let Some(value)=cache.get(&context){return Ok(Arc::clone(value));}
 let value=Arc::new(bank.qsa.encode(context)?);if cache.len()<16384{cache.insert(context,Arc::clone(&value));}Ok(value)
}
fn encoding(bank:&StdBank,a:Address,cache:&mut HashMap<[usize;3],Arc<Encoding>>)->Result<Arc<Encoding>>{
 if bank.syllable(a).encodings.is_some(){return Err(StdError::Unsupported("R1 stored-context encoding arrays require a separate decoder".into()));}
 context_encoding(bank,bank.context(a),cache)
}
fn category_id(category:&str)->u16{match category{"vowel"=>1,"diphthong"=>2,"coda"=>3,"stop"=>4,"fricative"=>5,"affricate"=>6,"aspirate"=>7,"nasal"=>8,"liquid"=>9,"semivowel"=>10,"silence"=>11,_=>0}}
fn modify(bank:&StdBank,a:[usize;3],b:[usize;3],cost:&mut[f32;3]){
 let ac=a.map(|p|category_id(bank.category(p)));let bc=b.map(|p|category_id(bank.category(p)));
 for rule in &bank.rules{let mut matches=true;for j in 0..6{let p=j%3;let(l,r,lc,rc)=if j<3{(a[p],b[p],ac[p],bc[p])}else{(b[p],a[p],bc[p],ac[p])};let pass=match rule.ops[j]{0=>true,1=>lc==3,2=>lc==4,3=>lc==10,4=>lc==1,5=>lc==2,6=>lc!=1&&lc!=2&&lc!=11,7=>l!=r,8=>l==r,9=>lc!=rc,10=>lc==rc,11=>l==rule.arguments[j]as usize,_=>false};if !pass{matches=false;break;}}
  if matches{for(j,v)in cost.iter_mut().enumerate(){*v=if rule.replace[j]!=0{rule.values[j]}else if rule.values[j]>0.{*v+rule.values[j]}else{*v*rule.values[j].exp2()};}break;}}
}
fn cost(bank:&StdBank,address:Address,target:[usize;3],encoded:&Encoding,cache:&mut HashMap<[usize;3],Arc<Encoding>>)->Result<[f32;3]>{
 let e=encoding(bank,address,cache)?;let current=bank.phone(address);if folded(bank.category(current))!=folded(bank.category(target[1])){return Ok([1.;3]);}
 let mut out=bank.qsa.compare(encoded,&e)?;if current!=target[1]{for v in &mut out{*v+=0.1;}}modify(bank,target,bank.context(address),&mut out);Ok(out)
}
fn pool(bank:&StdBank,phone:usize)->&[Address]{&bank.pools[phone]}
fn candidates(bank:&StdBank,score:&Score,i:usize)->Result<Vec<Address>>{let c=score.context(i,bank.silence);
 let forward=|phone|pool(bank,phone).iter().copied().filter(|a|bank.neighbor(*a,true).is_some()).collect::<Vec<_>>();
 let backward=|phone|pool(bank,phone).iter().copied().filter_map(|a|bank.neighbor(a,false)).collect::<Vec<_>>();
 let mut out=forward(c[1]);if out.is_empty(){out=forward(bank.silence);}let ids=out.iter().copied().collect::<HashSet<_>>();
 if i+1<score.phones.len(){let mut back=backward(c[2]);if back.is_empty(){back=backward(bank.silence);}out.extend(back.into_iter().filter(|a|!ids.contains(a)));}
 let filtered=out.iter().copied().filter(|a|{let x=bank.context(*a);folded(bank.category(x[1]))==folded(bank.category(c[1]))&&folded(bank.category(x[2]))==folded(bank.category(c[2]))}).collect::<Vec<_>>();
 if !filtered.is_empty(){out=filtered;}ensure(!out.is_empty()&&out.len()<=32768,"No bounded R1 candidate group")?;Ok(out)
}
fn transition(bank:&StdBank,previous:&Pair,current:&Pair,target:usize,cache:&mut HashMap<[usize;3],Arc<Encoding>>)->Result<f32>{
 let left=bank.phone(previous.second);let right=bank.phone(current.first);if left!=target&&right!=target{return Ok(10.);}
 let lc=folded(bank.category(left));let rc=folded(bank.category(right));let tc=folded(bank.category(target));
 let a=if lc!=tc{[1.;3]}else{previous.b};let b=if rc!=tc{[1.;3]}else{current.a};
 let align=if current.first==previous.second{[0.;3]}else if lc!=rc{[1.;3]}else{
  let x=bank.syllable(previous.second);let y=bank.syllable(current.first);
  if let(Some(kg),Some(x),Some(y))=(&bank.kg,&x.features,&y.features){kg.compare(&x[previous.second.phone],&y[current.first.phone])?}
  else{bank.qsa.compare(encoding(bank,previous.second,cache)?.as_ref(),encoding(bank,current.first,cache)?.as_ref())?}};
 let avg=|x:[f32;3]|(x[0]+x[1]+x[2])/3.;let m=mean(&[avg(a),avg(b),avg(align)],3.);
 Ok(if left==right{m}else{m+if left==target{b[2]}else{a[0]}})
}
fn sort(nodes:&mut[Pair]){fn order(v:&mut[Pair],mut l:usize,mut u:usize){while l<u{if v[u].rank<v[l].rank{v.swap(l,u);}if u-l==1{return;}let p=(l+u)/2;if v[p].rank<v[l].rank{v.swap(p,l);}else if v[u].rank<v[p].rank{v.swap(p,u);}if u-l==2{return;}
 let rank=v[p].rank;v.swap(p,u-1);let(mut a,mut b)=(l,u-1);loop{a+=1;while v[a].rank<rank{a+=1;}b-=1;while rank<v[b].rank{b-=1;}if b<a{v.swap(u-1,a);break;}v.swap(a,b);}
 if a-l<u-a{if a>l{order(v,l,a-1);}l=a+1;}else{if a<u{order(v,a+1,u);}if a==0{return;}u=a-1;}}}if nodes.len()>1{order(nodes,0,nodes.len()-1);}}
pub(super) fn select(bank:&StdBank,score:&Score,cancel:&AtomicBool)->Result<Vec<Pair>>{
 let mut cache=bank.encoding_cache.lock().map_err(|_|StdError::Native("R1 encoding cache is poisoned".into()))?;let mut layers=Vec::new();let mut total=0;
 for i in 0..score.phones.len(){super::cancel(cancel)?;let target=score.context(i,bank.silence);let forward=if i+1==score.phones.len(){[target[1],bank.silence,bank.silence]}else{score.context(i+1,bank.silence)};
  let x=context_encoding(bank,target,&mut cache)?;let y=context_encoding(bank,forward,&mut cache)?;let mut layer=Vec::new();for first in candidates(bank,score,i)?{let second=bank.neighbor(first,true).ok_or_else(||StdError::Unsupported("R1 selected candidate lacks a physical successor".into()))?;
   let a=cost(bank,first,target,&x,&mut cache)?;let b=cost(bank,second,forward,&y,&mut cache)?;let rank=mean(&[a[1],b[1]],2.);
   let syllable_duration=|a:Address|{let s=bank.syllable(a);s.end[s.end.len()-1]-s.start[0]};
   let ratio=|a:f32,b:f32|->Result<f32>{ensure(a>0.&&b>0.,"Nonpositive R1 duration operand")?;Ok((a.max(b)/a.min(b)).ln())};
   let mut local=if i==0{a[1]}else{0.};if bank.category(target[1])!="silence"{local+=0.1*mean(&[ratio(syllable_duration(first),score.duration(i))?,ratio(syllable_duration(second),score.duration((i+1).min(score.phones.len()-1)))?],2.);}
   ensure(rank.is_finite()&&local.is_finite(),"Nonfinite R1 ranking cost")?;layer.push(Pair{first,second,a,b,rank,local});}
  sort(&mut layer);layer.truncate(20);total+=layer.len();ensure(total<=32768,"R1 candidate graph capacity exceeded")?;layers.push(layer);}
 let mut scores=layers.iter().map(|l|vec![0.;l.len()]).collect::<Vec<_>>();let mut back=layers.iter().map(|l|vec![0usize;l.len()]).collect::<Vec<_>>();for(n,node)in layers[0].iter().enumerate(){scores[0][n]=node.local*node.local;}
 for i in 1..layers.len(){super::cancel(cancel)?;for j in 0..layers[i].len(){let mut best=1e10_f32;let mut predecessor=0;
   for p in 0..layers[i-1].len(){let prefix=scores[i-1][p];if prefix>=best{continue;}let edge=transition(bank,&layers[i-1][p],&layers[i][j],score.phone(i),&mut cache)?;let value=prefix+edge*edge;if value<best{best=value;predecessor=p;}}
   scores[i][j]=if bank.category(score.phone(i))=="silence"{layers[i][j].local*layers[i][j].local}else{best};back[i][j]=predecessor;}}
 let last=scores.last().unwrap();let(mut selected,_)=last.iter().enumerate().min_by(|a,b|a.1.total_cmp(b.1)).ok_or_else(||StdError::Invalid("Empty R1 selected path".into()))?;
 let mut indices=vec![0;layers.len()];for i in(0..layers.len()).rev(){indices[i]=selected;if i>0{selected=back[i][selected];}}
 Ok(layers.into_iter().zip(indices).map(|(layer,index)|layer[index].clone()).collect())
}
