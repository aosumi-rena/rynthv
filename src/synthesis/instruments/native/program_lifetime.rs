use super::{Result, program::Node};

pub fn counts(nodes:&[Node],outputs:&[(usize,u32)],bound:&[u32])->Result<Vec<usize>> {
    let mut uses=vec![0;nodes.len()];
    let mut visit:Vec<_>=outputs.iter().map(|(i,_)|*i).collect();
    while let Some(i)=visit.pop() {
        let node=nodes.get(i).ok_or("Instrument programme lifetime bounds")?;
        uses[i]+=1;
        if uses[i]!=1 {continue;}
        if bound.contains(&node.tag) || (node.opcode==0 && node.kind==3) {continue;}
        for p in node.ports.iter().copied().filter(|p|*p!=65535).map(usize::from) {
            if p>=i {return Err("Instrument programme dependency order".into());}
            if nodes[p].kind!=2 {visit.push(p);}
        }
    }
    Ok(uses)
}
