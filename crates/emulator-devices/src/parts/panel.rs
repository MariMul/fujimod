//! Attached rear LCD serial sink, separate from the SPI SoC controller.
//! Endpoint1 words use bit15 for command/data and bits14..7 for the byte,
//! then close without reading RX. Register effects and receive data are not
//! modeled. RAW16 framing retains whole words; panel pixels/effects are not inferred.
use crate::parts::serial::{Endpoint, Frame};
use crate::record::{Result, Unsupported};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Serial word layouts supported by attached panel sinks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Framing {
    Raw16,
    CommandByte { command_shift: u8, byte_shift: u8 },
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    #[serde(default)]
    pub framing: Framing,
}

impl Default for Framing {
    fn default() -> Self {
        Self::CommandByte {
            command_shift: 15,
            byte_shift: 7,
        }
    }
}

impl Framing {
    fn validate(self) -> Result<()> {
        if let Self::CommandByte {
            command_shift,
            byte_shift,
        } = self
        {
            if byte_shift > 7 || command_shift != byte_shift + 8 {
                return Err(Unsupported::from(
                    "Rear-panel framing requires adjacent command/data and byte fields",
                ));
            }
        }
        Ok(())
    }

    fn decode(self, frame: Frame) -> Result<Vec<(&'static str, u32)>> {
        self.validate()?;
        if frame.bits != 16 {
            return Err(Unsupported::from(
                "Rear-panel serial sink requires a sixteen-bit word",
            ));
        }
        match self {
            Self::Raw16 => Ok(vec![("word16", u32::from(frame.word))]),
            Self::CommandByte {
                command_shift,
                byte_shift,
            } => {
                let used_mask = 0x01ffu16 << byte_shift;
                if frame.word & !used_mask != 0 {
                    return Err(Unsupported::from(
                        "Rear-panel serial sink requires one left-justified 9-bit command/data word",
                    ));
                }
                Ok(vec![
                    ("command_data", u32::from((frame.word >> command_shift) & 1)),
                    ("byte", u32::from((frame.word >> byte_shift) & 0xff)),
                ])
            }
        }
    }
}

/// Write-only serial sink. Received words have no modeled panel effect.
pub struct PanelSink {
    framing: Framing,
}

impl PanelSink {
    pub fn new(options: Options) -> Result<Self> {
        let framing = options.framing;
        framing.validate()?;
        Ok(Self { framing })
    }
}

impl Endpoint for PanelSink {
    fn name(&self) -> &'static str {
        "rear-panel-serial-sink"
    }

    fn validate(&self, frame: Frame) -> std::result::Result<(), String> {
        self.framing
            .decode(frame)
            .map(|_| ())
            .map_err(|error| error.0)
    }

    fn reset_transaction(&mut self) {}

    fn transfer(&mut self, frame: Frame, _now_ns: u64) -> u16 {
        // This write-only endpoint does not read RX; zero represents an
        // unused controller input, not a measured panel response.
        let _ = self
            .framing
            .decode(frame)
            .expect("serial controller validates panel frames before transfer");
        0
    }

    fn save(&self) -> Value {
        json!({"framing": self.framing})
    }

    fn restore(&mut self, state: &Value) -> std::result::Result<(), String> {
        let framing: Framing = serde_json::from_value(state["framing"].clone())
            .map_err(|error| format!("invalid panel serial checkpoint: {error}"))?;
        framing.validate().map_err(|error| error.0)?;
        if framing != self.framing {
            return Err("panel serial framing differs from checkpoint".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct PanelSerial {
    pub words_received: u64,
}
impl PanelSerial {
    pub fn receive(
        &mut self,
        word: Option<i128>,
        transport_matches: bool,
        selection_matches: bool,
        command_shift: Option<u8>,
        byte_shift: Option<u8>,
    ) -> Result<Vec<(&'static str, u32)>> {
        let decoded = validate(
            word,
            transport_matches,
            selection_matches,
            command_shift,
            byte_shift,
        )?;
        self.words_received += 1;
        Ok(decoded)
    }
}
pub fn validate(
    word: Option<i128>,
    transport_matches: bool,
    selection_matches: bool,
    command_shift: Option<u8>,
    byte_shift: Option<u8>,
) -> Result<Vec<(&'static str, u32)>> {
    if !transport_matches {
        return Err(Unsupported::from(
            "Unsupported rear-panel SPI1 transport configuration",
        ));
    }
    if !selection_matches {
        return Err(Unsupported::from(
            "Rear-panel SPI1 requires its declared GPIO chip-select levels",
        ));
    }
    if byte_shift.is_none() {
        let Some(word) = word.filter(|w| (0..1 << 16).contains(w)) else {
            return Err(Unsupported::from(
                "Rear-panel SPI1 requires one sixteen-bit word",
            ));
        };
        return Ok(vec![("word16", word as u32)]);
    }
    let shift = byte_shift.unwrap();
    let Some(word) = word.filter(|w| (0..1 << 16).contains(w) && w & !(0x1ff << shift) == 0) else {
        return Err(Unsupported::from(
            "Rear-panel SPI1 requires one left-justified 9-bit command/data word",
        ));
    };
    Ok(vec![
        (
            "command_data",
            ((word >> command_shift.unwrap()) & 1) as u32,
        ),
        ("byte", ((word >> shift) & 0xff) as u32),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_sink_decodes_stock_word_and_returns_unused_idle_rx() {
        let mut panel = PanelSink::new(Options {
            framing: Framing::default(),
        })
        .unwrap();
        let frame = Frame {
            bits: 16,
            mode: 0,
            word: 0xad00,
        };
        assert_eq!(panel.validate(frame), Ok(()));
        assert_eq!(panel.transfer(frame, 100), 0);
        assert_eq!(
            panel.save(),
            json!({"framing": {
                "CommandByte": {"command_shift": 15, "byte_shift": 7}
            }})
        );
    }

    #[test]
    fn raw_sibling_word_and_unsupported_frames_are_explicit() {
        let mut panel = PanelSink::new(Options {
            framing: Framing::Raw16,
        })
        .unwrap();
        let raw = Frame {
            bits: 16,
            mode: 3,
            word: 0x401,
        };
        assert_eq!(panel.validate(raw), Ok(()));
        assert_eq!(panel.transfer(raw, 0), 0);
        assert!(
            panel
                .validate(Frame { bits: 8, ..raw })
                .unwrap_err()
                .contains("sixteen-bit")
        );

        let framed = PanelSink::new(Options {
            framing: Framing::default(),
        })
        .unwrap();
        assert!(
            framed
                .validate(Frame {
                    word: 0xad01,
                    ..raw
                })
                .unwrap_err()
                .contains("left-justified")
        );
        assert!(
            PanelSink::new(Options {
                framing: Framing::CommandByte {
                    command_shift: 14,
                    byte_shift: 7,
                },
            })
            .is_err()
        );
    }

    #[test]
    fn restore_preserves_the_selected_framing_contract() {
        let mut panel = PanelSink::new(Options {
            framing: Framing::Raw16,
        })
        .unwrap();
        assert!(
            panel
                .restore(&json!({"framing": {
                    "CommandByte": {"command_shift": 15, "byte_shift": 7}
                }}))
                .is_err()
        );
        panel.restore(&json!({"framing":"Raw16"})).unwrap();
    }
}
