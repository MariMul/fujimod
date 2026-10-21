//! ARMv7-R word exclusive transfers. Other widths explicitly refuse.
use super::*;
enum Instruction {
    Clear,
    Load {
        base: usize,
        data: usize,
        offset: u32,
    },
    Store {
        base: usize,
        data: usize,
        status: usize,
        offset: u32,
    },
}
fn arm(word: u32) -> Result<Option<Instruction>, String> {
    if word == 0xf57ff01f {
        return Ok(Some(Instruction::Clear));
    }
    let base = ((word >> 16) & 15) as usize;
    let target = ((word >> 12) & 15) as usize;
    if word & 0x0ff00fff == 0x01900f9f {
        if base == 15 || target == 15 {
            return Err("unpredictable R-profile LDREX register".into());
        }
        return Ok(Some(Instruction::Load {
            base,
            data: target,
            offset: 0,
        }));
    }
    if word & 0x0ff00ff0 == 0x01800f90 {
        let data = (word & 15) as usize;
        if base == 15 || data == 15 || target == 15 || target == base || target == data {
            return Err("unpredictable R-profile STREX register".into());
        }
        return Ok(Some(Instruction::Store {
            base,
            data,
            status: target,
            offset: 0,
        }));
    }
    if super::atomic(word) {
        return Err("R-profile nonword exclusive execution is not implemented".into());
    }
    Ok(None)
}
fn thumb(word: u32) -> Result<Option<Instruction>, String> {
    if !super::thumb_atomic(word) {
        return Ok(None);
    }
    let first = word as u16;
    let second = (word >> 16) as u16;
    if (first, second) == (0xf3bf, 0x8f2f) {
        return Ok(Some(Instruction::Clear));
    }
    let base = (first & 15) as usize;
    let data = (second >> 12) as usize;
    let status = ((second >> 8) & 15) as usize;
    let offset = (second as u32 & 255) * 4;
    let load = first & 0xfff0 == 0xe850;
    if !load && first & 0xfff0 != 0xe840 {
        return Err("R-profile nonword exclusive execution is not implemented".into());
    }
    if base == 15
        || matches!(data, 13 | 15)
        || (!load && (matches!(status, 13 | 15) || status == base || status == data))
    {
        return Err("unpredictable R-profile Thumb exclusive register".into());
    }
    Ok(Some(if load {
        Instruction::Load { base, data, offset }
    } else {
        Instruction::Store {
            base,
            data,
            status,
            offset,
        }
    }))
}
impl Drop for Execution {
    fn drop(&mut self) {
        self.memory
            .monitors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unregister(self.monitor);
        self.memory.forget(self.id.get());
    }
}
impl Execution {
    pub(super) fn clear_exclusive(&self) -> Result<(), String> {
        self.memory
            .monitors
            .lock()
            .map_err(|_| "exclusive monitors poisoned")?
            .clear(self.monitor);
        Ok(())
    }
    pub(super) fn exclusive_instruction(
        &self,
        word: u32,
        address: u64,
        size: u32,
        cpsr: u32,
    ) -> Result<bool, String> {
        let instruction = if self.thumb.get() {
            thumb(word)?
        } else {
            arm(word)?
        };
        let Some(instruction) = instruction else {
            return Ok(false);
        };
        match instruction {
            Instruction::Clear => self.clear_exclusive()?,
            Instruction::Load { base, data, offset }
            | Instruction::Store {
                base, data, offset, ..
            } => {
                let target = self.scalar(arm::CORE[base])?.wrapping_add(offset) as u64;
                if target & 3 != 0 {
                    return Err("unaligned R-profile exclusive".into());
                }
                let physical = self.memory.canonical(self.id.get(), target, 4)?;
                let load = matches!(instruction, Instruction::Load { .. });
                let mut bytes = [0; 4];
                self.cpu.read_virtual(
                    target,
                    if load { Access::Read } else { Access::Write },
                    &mut bytes,
                )?;
                if load {
                    // Software LDREX is a guest read, just like a backend
                    // load. Host/DMA inspection never enters these hooks.
                    for observer in self.observers.borrow().values() {
                        if let Hook::Memory {
                            callback,
                            data,
                            read: true,
                            range,
                            ..
                        } = observer.hook
                            && includes(range, target)
                        {
                            unsafe {
                                callback(Access::Read, target, 4, 0, data);
                            }
                        }
                    }
                    self.memory.watch_exclusive(self.id.get(), target, 4)?;
                    self.memory
                        .monitors
                        .lock()
                        .map_err(|_| "exclusive monitors poisoned")?
                        .load(self.monitor, physical, 4);
                    self.set(
                        arm::CORE[data],
                        if cpsr & 0x200 == 0 {
                            u32::from_le_bytes(bytes)
                        } else {
                            u32::from_be_bytes(bytes)
                        },
                    )?;
                } else if let Instruction::Store { status, .. } = instruction {
                    let success = self
                        .memory
                        .monitors
                        .lock()
                        .map_err(|_| "exclusive monitors poisoned")?
                        .store(self.monitor, physical, 4);
                    if success {
                        let value = self.scalar(arm::CORE[data])?;
                        let bytes = if cpsr & 0x200 == 0 {
                            value.to_le_bytes()
                        } else {
                            value.to_be_bytes()
                        };
                        // Successful software exclusives pass through the same explicit
                        // memory observers as native guest stores. No failed STREX store
                        // is reported. Observers may stop before a declared store bound.
                        for observer in self.observers.borrow().values() {
                            if let Hook::Memory {
                                callback,
                                data,
                                write: true,
                                range,
                                ..
                            } = observer.hook
                                && includes(range, target)
                            {
                                unsafe {
                                    callback(Access::Write, target, 4, value as i64, data);
                                }
                            }
                        }
                        if self.user_stop.get() {
                            return Ok(true);
                        }
                        // Already completed here: never replay this instruction
                        // when another translated alias needs invalidation.
                        self.mark_direct_write(target, bytes.len())?;
                        self.cpu.write_memory(target, &bytes)?;
                        if self.memory.written(self.id.get(), target, 4)? {
                            self.memory.prepare_external_write()?;
                        }
                    }
                    self.set(arm::CORE[status], u32::from(!success))?;
                }
            }
        }
        self.advance(address, size, cpsr)?;
        Ok(true)
    }
}
