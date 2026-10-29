//! Explicit XP4 processor, wiring and external input selection.
use super::*;
use serde::Deserialize;

fn instruction_rate() -> (u64, u64) {
    (1, 1)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub ram_size: usize,
    pub cores: usize,
    pub discovery: bool,
    #[serde(default)]
    pub memory_regions: Vec<super::memory::Region>,
    #[serde(default)]
    pub initial_registers: Vec<Vec<(Register, u64)>>,
    #[serde(default)]
    pub labels: Vec<u32>,
    #[serde(default)]
    pub cbar: Option<u32>,
    #[serde(default)]
    pub timer_base: Option<u32>,
    #[serde(default)]
    pub timer_layout: Option<TimerLayout>,
    #[serde(default)]
    pub timer_routes: Vec<(usize, usize)>,
    #[serde(default)]
    pub storage_jedec: Option<u32>,
    #[serde(default)]
    pub storage_dma_routes: Vec<u32>,
    #[serde(default)]
    pub spi_inputs: Vec<(u64, bool)>,
    #[serde(default)]
    pub control_requests: Vec<(u64, u64)>,
    #[serde(default)]
    pub storage_geometry: Option<(usize, usize, u64, u64)>,
    #[serde(default)]
    pub storage_irq: Option<u32>,
    #[serde(default)]
    pub xpp_storage: Option<XppStorage>,
    #[serde(default)]
    pub xpp_serial: Vec<u64>,
    #[serde(default)]
    pub cache_sizes: Option<(u32, u32, u32)>,
    #[serde(default)]
    pub io_group_base: Option<u64>,
    #[serde(default)]
    pub decompressor: Option<(u64, u32)>,
    #[serde(default)]
    pub lcdc: Vec<(u64, u64, u32, u32, Option<String>, Option<String>)>,
    #[serde(default)]
    pub dmp: Option<(u64, u32)>,
    #[serde(default)]
    pub adc: Option<([u16; 12], u64)>,
    #[serde(default)]
    pub gpio: Option<[u16; 16]>,
    #[serde(default)]
    pub temperature: Option<(i32, u64)>,
    #[serde(default)]
    pub power_spi: Option<(u64, bool, usize, usize, bool, BTreeMap<u8, u16>, u64)>,
    #[serde(default)]
    pub checksum_power_spi: Option<(
        u64,
        bool,
        usize,
        usize,
        bool,
        BTreeMap<u8, u16>,
        Vec<(u8, u16)>,
        Option<fujimod_emulator_xp4_io::rtc::StagedConfig>,
        Vec<u8>,
    )>,
    #[serde(default)]
    pub imaging: Option<(
        (u64, u64, Vec<u32>, Vec<u32>, Vec<u32>),
        (u64, u64, u64, u64),
        (u16, u8),
        Option<(u64, u32, u64, u64, u64, String)>,
    )>,
    #[serde(default)]
    pub movdc: Vec<(u64, u32, u64, u64, u8)>,
    #[serde(default)]
    pub sd_clocks: Vec<(u64, u64)>,
    #[serde(default = "instruction_rate")]
    pub instructions_per_tick: (u64, u64),
    #[serde(default)]
    pub usb: Option<(u64, u32, u64, u64, u8)>,
    #[serde(default)]
    pub usb_phy: Option<(u64, [u8; 32], u32)>,
    #[serde(default)]
    pub xpp_boot: Option<XppBoot>,
    #[serde(default)]
    pub xpp_sd: Option<XppSd>,
    #[serde(default)]
    pub dram_alias: Option<u64>,
    #[serde(default)]
    pub pmu_cycles_per_tick: Option<u64>,
    /// GPIO bank base; XP4 places it at 0xfff41000.
    #[serde(default)]
    pub gpio_base: Option<u64>,
    /// Declared (port base, send word, receive word) replies.
    #[serde(default)]
    pub xpp_serial_replies: Vec<(u64, u32, u32)>,
    /// ADC register base; XP4 places it at 0xfff40000. A declared base
    /// selects the X-Processor Pro law, where +8 is a power-down bit.
    #[serde(default)]
    pub adc_base: Option<u64>,
}

/// Register layout of the interval timer at `timer_base`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerLayout {
    /// Eight channels; select bits set in the control word.
    Xp4,
    /// Sixteen channels; select bits clear in the control word.
    Xpp,
}

/// X-Processor Pro flash controller, read aperture and PL330 wiring.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XppStorage {
    pub flash_controller: u64,
    pub aperture: u64,
    pub aperture_size: u64,
    pub dma: u64,
    pub dma_irq_routes: Vec<u32>,
    pub dma_setup_ticks: u64,
    pub dma_bytes_per_tick: u64,
    #[serde(default)]
    pub nor: Vec<fujimod_emulator_xpp::board::Nor>,
    /// IRQ for completed DMA programming; without it, aperture stores refuse.
    #[serde(default)]
    pub program_irq: Option<u32>,
}

/// X-Processor Pro cold-reset controller and its boot-selection strap level.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XppBoot {
    pub options: fujimod_emulator_xpp::boot::Options,
    pub card_strap: bool,
    /// Memory window shown at physical 0 until the guest selects the remap.
    #[serde(default)]
    pub low_alias: Option<u64>,
}

/// X-Processor Pro SD0 wrapper and the FAT geometry of its inserted card.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XppSd {
    pub options: fujimod_emulator_xpp::sd::Options,
    pub format: fujimod_emulator_storage::fat::Options,
}
