// SPDX-License-Identifier: MIT
use super::super::{Error, Result};

pub(super) struct Ring {
    words: Vec<f32>,
    head: usize,
}
impl Ring {
    pub fn new(length: usize) -> Self {
        Self { words: vec![0.0; length], head: 0 }
    }
    fn begin(&self, lag: isize, count: usize) -> Result<usize> {
        let begin = self.head as isize + self.words.len() as isize + lag;
        if begin < 0 || count > self.words.len() {
            return Err(Error::Format("P3 ring relative span"));
        }
        Ok(begin as usize)
    }
    pub fn advance(&mut self, count: usize, clear: bool) -> Result<()> {
        if count > self.words.len() { return Err(Error::Format("P3 ring advance")); }
        for _ in 0..count {
            if clear { self.words[self.head] = 0.0; }
            self.head = (self.head + 1) % self.words.len();
        }
        Ok(())
    }
    pub fn read(&self, lag: isize, count: usize) -> Result<Vec<f32>> {
        let begin = self.begin(lag, count)?;
        Ok((0..count).map(|i| self.words[(begin+i)%self.words.len()]).collect())
    }
    pub fn write(&mut self, lag: isize, values: &[f32], add: bool) -> Result<()> {
        let begin = self.begin(lag, values.len())?;
        for (i, &value) in values.iter().enumerate() {
            let index = (begin+i)%self.words.len();
            self.words[index] = if add { value+self.words[index] } else { value };
        }
        Ok(())
    }
}
