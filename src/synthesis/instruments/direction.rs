pub const ITEMS:[(&str,&str);6]=[("","Automatic"),("adagio","Adagio"),("allegro","Allegro"),("conFuoco","Con Fuoco"),("pop","Pop"),("ballad","Ballad")];
pub fn index(value:&str)->Result<Option<usize>,String> {
    if value.is_empty(){return Ok(None);}
    ITEMS[1..].iter().position(|(key,_)|*key==value).map(Some)
        .ok_or_else(||format!("Unknown instrument performance direction: {value}"))
}
