use super::{StdBank,Result,ensure,selection::Pair,score::Score};
#[derive(Clone,Copy,Debug,PartialEq)]
pub(super) enum Window {Full,Mid,Left,Right,SemiLeft}
#[derive(Clone)]
pub(super) struct Guide {pub pair:Pair,pub start:f32,pub source_start:f32,pub source_boundary:f32,pub source_end:f32,pub first:f32,pub second:f32,pub silence_a:bool,pub silence_b:bool,pub window:Window,pub width:f32,pub onset:bool,pub phone_index:usize}
pub(super) fn create(bank:&StdBank,score:&Score,selected:&[Pair])->Result<(Vec<Guide>,f32)>{
 let n=selected.len();ensure(n==score.phones.len(),"Incomplete R1 selected path")?;let mut groups:Vec<(f32,Vec<usize>)>=Vec::new();let mut onset=vec![false;n];let mut begin=0;
 for record in &score.records{let mut nucleus=record.phones.iter().position(|p|matches!(bank.category(*p),"vowel"|"diphthong"));
  if let Some(p)=nucleus{if p>=2&&bank.category(record.phones[p-1])=="semivowel"{nucleus=Some(p-1);}}
  for p in 0..record.phones.len(){let i=begin+p;if let Some(v)=nucleus{if p<v{if groups.is_empty(){groups.push((0.,Vec::new()));}groups.last_mut().unwrap().1.push(i);onset[i]=true;}
    else if p==v{groups.push((record.duration,vec![i]));}else{groups.last_mut().unwrap().1.push(i);}}
   else{groups.push((record.duration/record.phones.len()as f32,vec![i]));}}begin+=record.phones.len();}
 let duration=|a|{let s=bank.syllable(a);s.end[a.phone]-s.start[a.phone]};let mut baseline=vec![0.;n];
 for i in 0..n{let a=&selected[i];let current=duration(a.first);let prior=if i==0{current}else{duration(selected[i-1].second)};
  let target=score.phone(i);let first=bank.phone(a.first)==target;let previous=i>0&&bank.phone(selected[i-1].second)==target;
  baseline[i]=if i>0&&!previous&&first{current}else if i>0&&previous&&!first{prior}else if matches!(bank.category(target),"stop"|"affricate"){current}else{(current+prior)/2.};}
 let flagged=|i:usize|matches!(bank.category(score.phone(i)),"vowel"|"diphthong"|"coda"|"silence");let mut durations=vec![0.;n];
 for(total,indices)in groups{let(mut vowel,mut other)=(0.,0.);for i in &indices{if flagged(*i){vowel+=baseline[*i];}else{other+=baseline[*i];}}
  let budget=(total-other).max(0.3*total);let other_budget=if vowel==0.{total}else{total-budget};
  for i in indices{let (weight,denominator)=if flagged(i){(budget,vowel)}else{(other_budget,other)};ensure(denominator>0.,"R1 duration allocation has an empty populated class")?;durations[i]=baseline[i]*weight/denominator;}}
 for i in 0..n.saturating_sub(1){let pair=&selected[i];let s=bank.syllable(pair.first);let delta=s.end[pair.first.phone]-s.onset_end[pair.first.phone];if delta>0.{let denominator=duration(pair.second);ensure(denominator>0.,"R1 seam has a zero successor span")?;
  let transfer=(delta/denominator*durations[i+1]).min(delta);durations[i]+=transfer;durations[i+1]-=transfer;}}
 let mut guides:Vec<Guide>=Vec::with_capacity(n);let mut cursor=0.;
 for i in 0..n{let pair=selected[i].clone();let(sa,sb)=(bank.syllable(pair.first),bank.syllable(pair.second));let(a,b)=(bank.category(bank.phone(pair.first)),bank.category(bank.phone(pair.second)));
  let(mut d1,mut d2)=(durations[i],durations.get(i+1).copied().unwrap_or(0.));let(mut t0,t1,mut t2)=(sa.start[pair.first.phone],sb.start[pair.second.phone],sb.end[pair.second.phone]);let mut start=cursor;
  ensure(t1>t0&&t2>t1&&d1>=0.&&d2>=0.,"Nonpositive R1 guide span")?;
  if matches!(a,"vowel"|"silence")&&d1<t1-t0{t0=t1-d1;}if a=="silence"&&t1-t0<d1{let retained=t1-t0;start=start+d1-retained;d1=retained;}
  if b=="silence"{if t2-t1<d2{d2=t2-t1;}else{t2=t1+d2;}}
  let mut window=Window::Full;let mut width=0.;
  if i>0&&matches!(a,"stop"|"affricate"){let previous=&guides[i-1];let prior=bank.phone(previous.pair.second);let mut prior_ratio=0.5;
   if matches!(bank.category(prior),"stop"|"affricate"){let span=previous.source_end-previous.source_boundary;ensure(span!=0.,"R1 stop guide has zero prior span")?;let s=bank.syllable(previous.pair.second);prior_ratio=s.blend[previous.pair.second.phone]/span;}
   let current_ratio=sa.blend[pair.first.phone]/(t1-t0);let prior09=prior_ratio*0.9;let current09=current_ratio*0.9;
   let mut x=prior_ratio*previous.second-current09*d1;if prior!=bank.phone(pair.first){x=x.max(0.);}
   let mut anchor=i as isize-1;while anchor>=0&&!matches!(bank.category(score.phone(anchor as usize)),"vowel"|"diphthong"|"silence"){anchor-=1;}
   let mut shift=0.;if anchor>=0{let at=anchor as usize;let before=guides[at].first;shift=(-x).max(0.05-before);
    for row in guides.iter_mut().take(i).skip(at+1){row.start+=shift;}guides[at].first=before+shift;if at>0{guides[at-1].second=(guides[at-1].second+shift).max(0.);}}
   ensure(prior09!=0.,"R1 stop guide has zero alignment coefficient")?;window=Window::SemiLeft;width=current09*d1;guides[i-1].second=((width-shift)/prior09+0.02).max(0.);
  }
  if i>0&&!matches!(a,"stop"|"semivowel"){let prior=bank.phone(guides[i-1].pair.second);if prior!=bank.phone(pair.first){width=0.15_f32.min(d1*0.5);window=if score.phone(i)==prior{Window::Right}else{Window::Left};}}
  if window==Window::Full&&d1>0.2{width=0.1_f32.min((d1-0.2)/2.);window=Window::Mid;}
  ensure([start,t0,t1,t2,d1,d2,width].iter().all(|v|v.is_finite()&&*v>=0.),"R1 guide left its finite nonnegative domain")?;
  guides.push(Guide{pair,start,source_start:t0,source_boundary:t1,source_end:t2,first:d1,second:d2,
   silence_a:score.phone(i)==bank.silence,silence_b:i+1==n||score.phone(i+1)==bank.silence,window,width,onset:onset[i],phone_index:i});cursor+=durations[i];}
 let duration=guides.last().map(|r|r.start+r.first).unwrap_or(0.);ensure(duration>=1.,"R1 guide sequence lacks body duration")?;
 let first=score.records.iter().take(2).map(|s|s.phones.len()).sum::<usize>()-1;
 let offset=score.note_start-score.origin-1.;
 let mut retained=guides.into_iter().skip(first).collect::<Vec<_>>();for row in &mut retained{row.start=(row.start as f64+offset)as f32;}
 Ok((retained,(duration as f64+offset)as f32))
}
