use super::{Graph, PhoneConversion, invalid,limit};
use crate::synthesis::voice::Result;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Path { cost: i32, output: Vec<usize> }

impl PhoneConversion {
    pub fn convert(&self, language: &str, tokens: &[String]) -> Result<Vec<String>> {
        self.direction(1,language,tokens)
    }
    pub fn direction(&self, direction: usize, language: &str, tokens: &[String]) -> Result<Vec<String>> {
        if tokens.len()>65536 {return Err(limit())}
        let graph=self.graphs.get(direction).ok_or_else(invalid)?;
        let marker=format!("<{language}>");
        let Some((&language,members))=self.ids.get(&marker).and_then(|id|graph.members.get(id).map(|members|(id,members))) else {return Ok(tokens.to_vec())};
        let mut output=Vec::new();let mut run=Vec::new();
        for token in tokens {
            let id=self.ids.get(token).copied();
            if !matches!(token.as_str(),"sil"|"br") && id.is_some_and(|id|members[id]) {
                run.push(id.unwrap());
            } else {
                self.flush(graph,language,&mut run,&mut output)?;
                output.push(token.clone());
            }
        }
        self.flush(graph,language,&mut run,&mut output)?;
        Ok(output)
    }
    fn flush(&self, graph: &Graph, language: usize, run: &mut Vec<usize>, output: &mut Vec<String>) -> Result<()> {
        if run.is_empty() {return Ok(())}
        let input=std::iter::once(language).chain(std::iter::once(self.boundary)).chain(run.iter().copied()).chain(std::iter::once(self.boundary));
        let converted=graph.apply(input)?;
        output.extend(converted.as_deref().unwrap_or(run).iter().map(|&id|self.symbols[id].clone()));
        run.clear();
        Ok(())
    }
}

impl Graph {
    fn apply(&self, input: impl Iterator<Item=usize>) -> Result<Option<Vec<usize>>> {
        let mut paths=vec![None;self.states.len()];paths[self.start]=Some(Path{cost:0,output:Vec::new()});
        let mut steps=0_usize;
        for token in input {
            crate::synthesis::execution::checkpoint()?;
            let mut next=vec![None;self.states.len()];
            for (state,path) in self.states.iter().zip(&paths) {
                let Some(path)=path else {continue};
                for arc in &state.arcs {
                    steps+=1;if steps>16777216 {return Err(limit())}
                    if arc.input!=token {continue}
                    let mut candidate=path.clone();candidate.cost=candidate.cost.wrapping_add(arc.cost);
                    if arc.output!=0 {candidate.output.push(arc.output);}
                    let slot=&mut next[arc.target];
                    if slot.as_ref().is_none_or(|old|candidate<*old) {*slot=Some(candidate);}
                }
            }
            paths=next;
        }
        let best=self.states.iter().zip(paths).filter_map(|(state,path)| {
            let mut path=path?;path.cost=path.cost.wrapping_add(state.final_cost?);Some(path)
        }).min();
        Ok(best.map(|path|path.output))
    }
}
