// SPDX-License-Identifier: MIT
use std::cmp::Ordering;
use super::{frame::FrameRequest,word32::{StdNumericError,StdWord32},word64::{StdWideError,StdWord64}};

const ONE_POINT_ZERO_ONE64:u64=0x3ff0_28f5_c28f_5c29;

#[derive(Clone,Debug)]
pub(crate) enum PositionError {
    FrameCount {actual:usize,required:usize},
    OriginalDomain {bits:u32},
    Neighbor {left:i32,frame_count:usize},
    RatioDomain {bits:u32},
    Numeric(StdWideError),
}
impl From<StdWideError> for PositionError {fn from(e:StdWideError)->Self{Self::Numeric(e)}}
impl From<StdNumericError> for PositionError {fn from(e:StdNumericError)->Self{Self::Numeric(e.into())}}

#[derive(Clone,Debug,Default)]
pub(crate) struct PositionState {
    pub frame_count:usize,pub original:u32,pub initial_floor:Option<i32>,pub upper_boundary:Option<u32>,pub upper_clipped:bool,pub lower_clipped:bool,pub double_count:Option<u64>,pub clip_difference:Option<u64>,pub effective:Option<u32>,pub left:Option<i32>,pub left_word:Option<u32>,pub ratio:Option<u32>,pub returned:bool,
}

pub(crate) fn derive(frame_count:usize,original:u32,state:&mut PositionState)->Result<FrameRequest,PositionError>{
    *state=PositionState::default();state.frame_count=frame_count;state.original=original;
    if !(2..=super::frame::MAX_SOURCE_FRAMES).contains(&frame_count){
        return Err(PositionError::FrameCount{actual:frame_count,required:2});
    }
    if original&0x7f80_0000==0x7f80_0000{return Err(PositionError::OriginalDomain{bits:original});}
    let input=StdWord32::from_bits(original)?;
    state.initial_floor=input.trunc_i32().ok();
    let boundary=StdWord32::from_i32((frame_count-1)as i32)?;state.upper_boundary=Some(boundary.bits());let mut position=input;
    if position.numeric_cmp(boundary)!=Ordering::Less {
        state.upper_clipped=true;
        let count=StdWord64::widen(StdWord32::from_i32(frame_count as i32)?)?;state.double_count=Some(count.bits());
        let difference=count.sub(StdWord64::from_bits(ONE_POINT_ZERO_ONE64)?)?;state.clip_difference=Some(difference.bits());position=difference.narrow()?;
    }
    let zero=StdWord32::from_bits(0)?;
    if position.numeric_cmp(zero)==Ordering::Less{state.lower_clipped=true;position=zero;}
    state.effective=Some(position.bits());let left=position.trunc_i32()?;state.left=Some(left);
    if left<0||(left as usize)+1>=frame_count{return Err(PositionError::Neighbor{left,frame_count});}
    let left_word=StdWord32::from_i32(left)?;state.left_word=Some(left_word.bits());let ratio=position.sub(left_word)?;state.ratio=Some(ratio.bits());
    if ratio.numeric_cmp(zero)==Ordering::Less
        || ratio.numeric_cmp(StdWord32::from_bits(0x3f80_0000)?)==Ordering::Greater {
        return Err(PositionError::RatioDomain{bits:ratio.bits()});
    }
    state.returned=true;Ok(FrameRequest{original,effective:position.bits(),ratio:ratio.bits()})
}
