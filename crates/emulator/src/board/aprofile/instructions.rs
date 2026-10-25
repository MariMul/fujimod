//! Architectural boundaries for instruction-observed AArch64 backends.
use crate::board::{State, scalar, set};
use fujimod_emulator_cpu::{Access, Cpu, Hint, Register, arm};

fn system(encoding: u16) -> Register {
    Register::Arm64System(arm::SystemRegister::from_encoding(encoding))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{
        Board, InterruptMap, InterruptRegisters, aprofile,
        bus::{Bus, Mode},
        factory,
    };
    use fujimod_emulator_cpu::Model;
    use fujimod_emulator_ip_arm::gicv3::Gic;

    fn board() -> Board {
        let graph = factory::Graph {
            processors: vec![factory::Processor {
                model: Model::ArmV8A,
                registers: vec![(Register::Pc, 0x1000), (Register::Pstate, 0x3cd)],
                released: true,
                mprofile: None,
                rprofile: None,
            }],
            ram: vec![factory::Ram {
                base: 0,
                size: 0x4000,
            }],
            dma_mappings: vec![crate::board::memory::BusMapping::ram(0, 0x4000)],
            decoders: vec![],
            mappings: vec![factory::MemoryMap {
                core: 0,
                address: 0,
                physical: 0,
                size: 0x4000,
                permissions: 7,
            }],
            peripherals: vec![],
            instructions_per_tick: fujimod_emulator_cpu::timeline::Ratio::new(1, 1).unwrap(),
        };
        let interrupts = InterruptMap {
            gic: Box::new(Gic::new(vec![0], 128).unwrap()),
            registers: InterruptRegisters::GicV3 {
                distributor: 0xfa000000,
                redistributor: 0xfa080000,
                stride: 0x20000,
            },
            cores: 1,
        };
        let mut board = graph
            .build(
                factory::Options::new(factory::Engine::Unicorn),
                Bus::new(Mode::Strict),
                Some(interrupts),
            )
            .unwrap();
        board
            .configure_aprofile(
                0,
                aprofile::Config {
                    clock: aprofile::generic_timer::Clock {
                        counter_hz: 100,
                        ticks_per_second: 100,
                    },
                    physical_irq: 30,
                    virtual_irq: 27,
                    pmu_hz: 100,
                    implementation: None,
                },
            )
            .unwrap();
        board
    }

    #[test]
    fn unicorn_counters_consume_guest_ticks_and_wfi_advances_pc_once() {
        let mut board = board();
        let msr = |r: u32, rt: u32| 0xd5000000 | r << 5 | rt;
        let mrs = |r: u32, rt: u32| 0xd5200000 | r << 5 | rt;
        let code = [
            msr(0xdce0, 0),
            msr(0xdce1, 1),
            mrs(0xdce8, 2),
            0xd503201f,
            0xd503201f,
            0xd503201f,
            0xd503201f,
            mrs(0xdce8, 3),
            mrs(0xdf01, 4),
            0xd503207f,
        ];
        board
            .write_memory(
                0,
                0x1000,
                &code
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        board.write_register(0, Register::X(0), 1).unwrap();
        board.write_register(0, Register::X(1), 1 << 31).unwrap();
        board.run_for_instructions(10, 1000).unwrap();
        assert_eq!(
            board.read_register(0, Register::X(3)).unwrap()
                - board.read_register(0, Register::X(2)).unwrap(),
            5
        );
        assert_eq!(board.read_register(0, Register::X(4)).unwrap(), 8);
        assert_eq!(board.read_register(0, Register::Pc).unwrap(), 0x1028);
        assert_eq!(board.instruction_counts(), [10]);
    }

    #[test]
    fn el1_irq_saves_interrupted_state_without_counting_the_interrupted_instruction() {
        let board = board();
        let cpu = &*board.cpus[0];
        set(cpu, Register::Pstate, 5).unwrap();
        set(cpu, system(0xf088), 0x404).unwrap();
        set(cpu, system(0xe088), 0).unwrap();
        set(cpu, system(0xc600), 0x2000).unwrap();
        let mut state = board.state.borrow_mut();
        state.pins[0] = (true, false);
        assert!(state.aarch64_before(cpu, 0, 0x1000).unwrap());
        assert_eq!(scalar(cpu, Register::Pc).unwrap(), 0x2280);
        assert_eq!(scalar(cpu, system(0xc201)).unwrap(), 0x1000);
        assert_eq!(scalar(cpu, system(0xc200)).unwrap(), 5);
        assert_eq!(scalar(cpu, Register::Pstate).unwrap(), 0x3c5);
        assert_eq!(state.instructions, [0]);
    }
}

impl State {
    pub(in crate::board) fn aarch64_before(
        &mut self,
        cpu: &dyn Cpu,
        core: usize,
        pc: u64,
    ) -> Result<bool, String> {
        if self.aprofile[core].is_none() || cpu.features().architectural_exits {
            return Ok(false);
        }
        let pstate = scalar(cpu, Register::Pstate)?;
        let (irq, fiq) = self.pins[core];
        if (irq && pstate & 0x80 == 0) || (fiq && pstate & 0x40 == 0) {
            let fiq = fiq && pstate & 0x40 == 0;
            let el = (pstate >> 2) & 3;
            if el > 1
                || scalar(cpu, system(0xf088))? & if fiq { 4 } else { 2 } != 0
                || scalar(cpu, system(0xe088))? & if fiq { 8 } else { 0x10 } != 0
            {
                return Err("AArch64 interrupt routing above EL1 is unsupported".into());
            }
            let offset = match pstate & 15 {
                0 => 0x400,
                4 => 0,
                5 => 0x200,
                _ => return Err("AArch64 interrupt entry mode is unsupported".into()),
            } + if fiq { 0x100 } else { 0x80 };
            let vector = scalar(cpu, system(0xc600))? + offset;
            set(cpu, system(0xc201), pc)?;
            set(cpu, system(0xc200), pstate)?;
            set(cpu, Register::Pstate, (pstate & 0xf0000000) | 0x3c5)?;
            set(cpu, Register::Pc, vector)?;
            cpu.stop()?;
            return Ok(true);
        }
        let mut bytes = [0; 4];
        cpu.read_virtual(pc, Access::Fetch, &mut bytes)?;
        let word = u32::from_le_bytes(bytes);
        if word & 0xffd00000 == 0xd5100000 {
            let encoding = ((word >> 5) & 0xffff) as u16;
            let r = arm::SystemRegister::from_encoding(encoding);
            let framework = r.crn == 9
                || r.crn == 14
                || r.crn == 15
                || encoding == 0xc230
                || encoding == 0xf665
                || (r.op0 == 3 && r.op1 == 0 && r.crn == 12 && (8..=15).contains(&r.crm));
            if framework {
                if encoding == 0xf665 && pstate & 12 != 12 {
                    return Err("ICC_SRE_EL3 requires EL3".into());
                }
                if pstate & 12 == 0 {
                    return Err("AArch64 EL0 framework register access is unsupported".into());
                }
                let read = word & (1 << 21) != 0;
                let rt = (word & 31) as u64;
                let syndrome = (0x18 << 26)
                    | (u64::from(r.op0) << 20)
                    | (u64::from(r.op1) << 14)
                    | (u64::from(r.crn) << 10)
                    | (u64::from(r.crm) << 1)
                    | (u64::from(r.op2) << 17)
                    | (rt << 5)
                    | u64::from(read);
                self.system_register(cpu, core, syndrome)?;
                cpu.stop()?;
            }
            return Ok(false);
        }
        let hint = match word {
            0xd503205f => Hint::WaitForEvent,
            0xd503207f => Hint::WaitForInterrupt,
            0xd503209f => Hint::SendEvent,
            0xd50320bf => Hint::SendEventLocal,
            _ => return Ok(false),
        };
        set(cpu, Register::Pc, pc.wrapping_add(4))?;
        self.hint(core, hint);
        cpu.stop()?;
        Ok(false)
    }
}
