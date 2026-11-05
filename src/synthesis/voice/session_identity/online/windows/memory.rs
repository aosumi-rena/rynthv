use std::ffi::c_void;

use zeroize::Zeroizing;

use super::{Result, VoiceError, native::*};

const MAXIMUM_FIELD_BYTES: usize = 256;
pub(super) fn session_field(process: &Handle, slot: usize, identity_offset: usize) -> Result<Zeroizing<Vec<u8>>> {
    let object = pointer(process, slot)?;
    let field_pointer = object
        .checked_add(identity_offset)
        .ok_or_else(|| VoiceError::new("原编辑器的模型上下文地址无效。"))?;
    let text = pointer(process, field_pointer)?;

    let mut field = Zeroizing::new(vec![0u8; MAXIMUM_FIELD_BYTES]);
    for offset in 0..MAXIMUM_FIELD_BYTES {
        let address = text
            .checked_add(offset)
            .ok_or_else(|| VoiceError::new("原编辑器的模型上下文地址无效。"))?;
        read(process, address, &mut field[offset..offset + 1])?;
        if field[offset] == 0 {

            let mut confirmation = Zeroizing::new(vec![0u8; offset + 1]);
            read(process, text, &mut confirmation)?;
            if pointer(process, slot)? != object || pointer(process, field_pointer)? != text {
                return Err(VoiceError::new(
                    "原编辑器的在线会话在读取时发生变化，请重试。",
                ));
            }
            if confirmation[..] != field[..offset + 1] {
                return Err(VoiceError::new(
                    "原编辑器的在线上下文在读取时发生变化，请重试。",
                ));
            }
            if offset == 0 {
                return Err(VoiceError::new(
                    "原编辑器尚未准备好在线模型上下文，请先登录并加载歌声。",
                ));
            }
            if std::str::from_utf8(&field[..offset]).is_err() {
                return Err(VoiceError::new("原编辑器的模型上下文不是有效的 UTF-8。"));
            }
            field.truncate(offset);
            return Ok(field);
        }
    }
    Err(VoiceError::new(
        "原编辑器的模型上下文超过安全读取上限，已停止导入。",
    ))
}

fn pointer(process: &Handle, address: usize) -> Result<usize> {
    let mut bytes = [0; 8];
    read(process, address, &mut bytes)?;
    Ok(u64::from_le_bytes(bytes) as usize)
}

fn read(process: &Handle, address: usize, output: &mut [u8]) -> Result<()> {
    if address < 0x10000
        || address
            .checked_add(output.len())
            .is_none_or(|end| end >= 0x7fff_ffff_ffff)
    {
        return Err(VoiceError::new("原编辑器的模型上下文地址不可用。"));
    }
    let mut received = 0;

    let success = unsafe {
        ReadProcessMemory(
            process.0,
            address as *const c_void,
            output.as_mut_ptr().cast(),
            output.len(),
            &mut received,
        )
    };
    if success == 0 || received != output.len() {
        return Err(VoiceError::new(
            "无法完整读取原编辑器的在线模型上下文，请确认会话仍在运行。",
        ));
    }
    Ok(())
}
