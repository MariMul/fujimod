#![cfg(not(target_os = "emscripten"))]

use crate::board::{
    Board,
    bus::{self, Bus},
    factory::{Engine, Graph, MemoryMap, Options, Processor, Ram},
    memory::BusMapping,
};
use fujimod_emulator_cpu::{ExecutionPolicy, Model, Register, timeline::Ratio};
use fujimod_emulator_devices::{Cx, Device, checkpoint::Checkpoint};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct DeadlineSample {
    due: Option<u64>,
    samples: Vec<(u64, u32)>,
}

impl Device for DeadlineSample {
    fn name(&self) -> &'static str {
        "interactive-deadline-sample"
    }

    fn checkpoint(&self) -> Result<Checkpoint, String> {
        Checkpoint::encode(self.name(), self)
    }

    fn validate_checkpoint(&self, saved: &Checkpoint) -> Result<(), String> {
        let _: Self = saved.decode(self.name())?;
        Ok(())
    }

    fn restore_checkpoint(&mut self, saved: &Checkpoint) -> Result<(), String> {
        *self = saved.decode(self.name())?;
        Ok(())
    }

    fn fingerprint(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(self).map_err(|error| error.to_string())
    }

    fn read(&mut self, cx: &mut Cx<'_>, _: u64, _: u8) -> u64 {
        cx.unmodeled();
        0
    }

    fn write(&mut self, cx: &mut Cx<'_>, _: u64, _: u8, _: u64) {
        cx.unmodeled();
    }

    fn next_deadline(&self, _: u64) -> Option<u64> {
        self.due
    }

    fn advance(&mut self, cx: &mut Cx<'_>) {
        if self.due.is_some_and(|due| due <= cx.now()) {
            self.due = None;
            let mut bytes = [0; 4];
            if let Err(error) = cx.ram_read(0x3000, &mut bytes) {
                cx.fail(error);
                return;
            }
            self.samples.push((cx.now(), u32::from_le_bytes(bytes)));
        }
    }
}

fn board(policy: ExecutionPolicy, due: Option<u64>) -> (Board, usize) {
    let processors = (0..2)
        .map(|core| Processor {
            model: Model::CortexR5F,
            registers: vec![
                (Register::Pc, 0x1000 + core * 0x1000),
                (Register::Pstate, 0xd3),
            ],
            released: true,
            mprofile: None,
            rprofile: Some(Default::default()),
        })
        .collect();
    let graph = Graph {
        processors,
        ram: vec![Ram {
            base: 0,
            size: 0x4000,
        }],
        dma_mappings: vec![BusMapping::ram(0, 0x4000)],
        decoders: vec![],
        mappings: (0..2)
            .map(|core| MemoryMap {
                core,
                address: 0,
                physical: 0,
                size: 0x4000,
                permissions: 7,
            })
            .collect(),
        peripherals: vec![],
        instructions_per_tick: Ratio::new(1, 1).unwrap(),
    };
    let mut bus = Bus::new(bus::Mode::Strict);
    let device = bus
        .register(
            0x8000000,
            4,
            Box::new(DeadlineSample {
                due,
                samples: Vec::new(),
            }),
        )
        .unwrap();
    let mut board = graph
        .build(
            Options {
                engine: Engine::Unicorn,
                unicorn: Some(crate::board::test_support::unicorn_library_path()),
                translation_cache_bytes: 1024 * 1024,
            },
            bus,
            None,
        )
        .unwrap();
    let code = [0xe3a0005au32, 0xe3a01a03, 0xe5810000, 0xeafffffe];
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
    // Either host thread may finish first. Both complete the same store so
    // the sampled value does not depend on which worker reaches its barrier.
    board
        .write_memory(
            0,
            0x2000,
            &code
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    if policy != ExecutionPolicy::Exact {
        board.set_execution_policy(policy).unwrap();
    }
    assert!(
        board
            .cpus
            .iter()
            .all(|cpu| cpu.features().asynchronous_runs)
    );
    (board, device)
}

fn samples(board: &Board, device: usize) -> Vec<(u64, u32)> {
    board
        .inspect_device::<DeadlineSample, _>(device, |sample| Ok(sample.samples.clone()))
        .unwrap()
}

#[test]
fn deadline_samples_policy_boundary_and_checkpoint_restores_the_observed_state() {
    for (policy, expected) in [
        (ExecutionPolicy::Exact, 0),
        (ExecutionPolicy::Interactive, 0x5a),
    ] {
        let (mut board, device) = board(policy, Some(2));
        board.run_until(2, 16).unwrap();

        assert_eq!(board.state()["timeline"]["now"], 2);
        assert_eq!(samples(&board, device), [(2, expected)]);
        let before = board.fingerprint().unwrap();
        let saved = board.checkpoint(None).unwrap();

        board
            .write_physical(0x3000, &0xdead_beefu32.to_le_bytes())
            .unwrap();
        board.restore_keyframe(&saved).unwrap();
        assert_eq!(board.fingerprint().unwrap(), before);
        assert_eq!(samples(&board, device), [(2, expected)]);

        board.run_until(4, 16).unwrap();
        assert_eq!(samples(&board, device), [(2, expected)]);
    }
}

#[test]
fn interactive_instruction_limit_allows_only_bounded_block_overshoot() {
    let (mut board, _) = board(ExecutionPolicy::Interactive, None);
    board.run_for_instructions(2, 16).unwrap();

    let retired = board.instruction_counts().into_iter().sum::<u64>();
    assert!(
        retired > 2,
        "interactive execution may retire a whole block"
    );
    assert!(
        retired <= 8,
        "two one-instruction shares can each overrun by at most one four-instruction block; retired {retired}"
    );
}
