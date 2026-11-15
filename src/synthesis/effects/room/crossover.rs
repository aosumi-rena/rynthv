const FILTERS: [[f32;5];6]=[
    [0.4651530683040619,-0.9303061366081238,0.4651530683040619,-0.6202040910720825,0.24040821194648743],
    [0.15505102276802063,0.31010204553604126,0.15505102276802063,-0.6202040910720825,0.24040821194648743],
    [0.8308980464935303,-1.6617960929870605,0.8308980464935303,-1.632993221282959,0.6905989050865173],
    [0.014401440508663654,0.02880288101732731,0.014401440508663654,-1.632993221282959,0.6905989050865173],
    [0.954774022102356,-1.909548044204712,0.954774022102356,-1.9075015783309937,0.9115945100784302],
    [0.0010232176864519715,0.002046435372903943,0.0010232176864519715,-1.9075015783309937,0.9115945100784302],
];

#[derive(Default)]
pub(super) struct Crossover { state:[[f32;4];6], initialized:bool }
impl Crossover {
    fn cascade(&mut self,index:usize,input:f32)->f32 {
        let [b0,b1,b2,a1,a2]=FILTERS[index];let state=&mut self.state[index];
        let first=b0*input+state[0];
        state[0]=(b1*input+state[1])-a1*first;
        state[1]=b2*input-a2*first;
        let second=b0*first+state[2];
        state[2]=(b1*first+state[3])-a1*second;
        state[3]=b2*first-a2*second;
        second
    }
    pub fn process(&mut self,input:f32)->[f32;4] {
        if !self.initialized {
            let b0=FILTERS[0][0];
            self.state[0]=[input*(-b0),input*b0,input*(-b0),input*b0];
            self.initialized=true;
        }
        let high=self.cascade(0,input);
        let low=self.cascade(1,input);
        let middle_high=self.cascade(2,low);
        let low=self.cascade(3,low);
        let middle_low=self.cascade(4,low);
        let low=self.cascade(5,low);
        [low,middle_low,middle_high,high]
    }
}
