mod capture;
mod curves;
mod groups;
mod plan;
mod regions;

use super::Editor;
use crate::model::*;
use plan::PastePlan;
use rynthv::clipboard::{self, Content};
type NoteClipboard = Content;

impl Editor {
    pub fn copy(&self, parameters: bool) -> Result<Option<String>, String> {
        let data = self.copy_content(parameters)?;
        data.as_ref().map(clipboard::write).transpose()
    }
    pub fn copy_content(&self, parameters: bool) -> Result<Option<Content>, String> {
        if self.group_focus {
            self.capture_groups()
        } else {
            self.capture_note_selection(parameters)
        }
    }
    pub fn paste_text(&mut self, text: &str, at: i64) -> Result<usize, String> {
        self.paste_content(clipboard::read(text)?, at)
    }
    pub fn paste_content(&mut self, data: Content, at: i64) -> Result<usize, String> {
        data.validate()?;
        if !data.groups.is_empty()
            && (!data.notes.is_empty()
                || !data.pitch_controls.is_empty()
                || !data.parameters.is_empty())
        {
            let origin = data.origin().ok_or("剪贴板数据无效。")?;
            let selection = self.selection();
            let mut next = Editor::new(self.project.clone());
            next.restore_selection(selection.clone());
            let mut groups = data.clone();
            groups.notes.clear();
            groups.pitch_controls.clear();
            groups.parameters.clear();
            let group_at = at
                .checked_add(
                    groups
                        .origin()
                        .unwrap()
                        .checked_sub(origin)
                        .ok_or("粘贴位置超出可用范围。")?,
                )
                .ok_or("粘贴位置超出可用范围。")?;
            let mut count = next.paste_groups(&groups, group_at)?;
            next.restore_selection(selection);
            let mut inner = data;
            inner.groups.clear();
            inner.library.clear();
            inner.tracks.clear();
            let inner_at = at
                .checked_add(
                    inner
                        .origin()
                        .ok_or("剪贴板数据无效。")?
                        .checked_sub(origin)
                        .ok_or("粘贴位置超出可用范围。")?,
                )
                .ok_or("粘贴位置超出可用范围。")?;
            count += next.paste_content(inner, inner_at)?;
            let selected = next.selection();
            self.finish();
            self.begin("粘贴");
            self.project = next.project;
            self.restore_selection(selected);
            self.finish();
            return Ok(count);
        }
        if !data.groups.is_empty() {
            return self.paste_groups(&data, at);
        }
        if data.notes.is_empty() {
            let reference = self.active_ref().ok_or("请先选择音符组。")?.clone();
            let delta = at
                .checked_sub(reference.blick_offset)
                .and_then(|v| v.checked_sub(data.origin()?))
                .ok_or("粘贴位置超出可用范围。")?;
            let mut group = self.group().ok_or("请先选择音符组。")?.clone();
            let minimum = reference
                .blick_offset
                .checked_neg()
                .ok_or("粘贴位置超出可用范围。")?;
            let count = curves::paste(&mut group, &data, delta, minimum)?;
            self.finish();
            self.begin("粘贴");
            *self.group_mut().unwrap() = group;
            self.finish();
            return Ok(count);
        }
        let plan = PastePlan::prepare(self, &data, at, false)?;
        self.finish();
        self.begin("粘贴音符");
        let count = plan.apply(self);
        self.finish();
        Ok(count)
    }
    pub fn duplicate_selected(&mut self, parameters: bool) -> Result<usize, String> {
        let Some(data) = self.capture_note_selection(parameters)? else {
            return Ok(0);
        };
        let reference = self.active_ref().ok_or("请先选择音符组。")?;
        let at = data
            .notes
            .iter()
            .map(|n| {
                n.onset
                    .checked_add(n.duration)
                    .and_then(|t| t.checked_add(reference.blick_offset))
                    .ok_or("所选音符的时间跨度超出可用范围。")
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .max()
            .ok_or("请先选择音符。")?;
        let plan = PastePlan::prepare(self, &data, at, true)?;
        self.finish();
        self.begin("重复音符");
        let count = plan.apply(self);
        self.finish();
        Ok(count)
    }
}
