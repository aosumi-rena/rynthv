pub(super) fn positions(center:[f64;2],count:usize)->Result<Vec<[f64;2]>,String> {
    if !(1..=128).contains(&count) {return Err("Invalid Hall player count".into());}
    let rows:Vec<usize>=match count {
        9=>vec![4,5],10=>vec![5,5],11=>vec![5,6],12=>vec![6,6],13=>vec![6,7],14=>vec![6,8],
        15=>vec![4,5,6],16=>vec![5,6,5],_=>vec![count],
    };
    let mut seats=Vec::with_capacity(count);
    for (row,&columns) in rows.iter().enumerate() {for column in 0..columns {
        seats.push((column as f32-(columns as f32-1.)*0.5,row,column));
    }}
    seats.sort_by(|a,b|a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut start=0;let mut forward=true;
    while start<seats.len() {
        let mut end=start+1;
        while end<seats.len() && (seats[end].0-seats[start].0).abs()<=0.01 {end+=1;}
        if !forward {seats[start..end].reverse();}forward=!forward;start=end;
    }
    let spacing=0.7f32;
    Ok(seats.into_iter().map(|(_,row,column)| {
        let y=(((1-rows.len() as i32) as f32*0.5)*0.5)+row as f32*0.5;
        let x=(-((rows[row] as f32-1.)*spacing)*0.5)+column as f32*spacing;
        [center[0]+f64::from(x),center[1]+f64::from(y)]
    }).collect())
}
