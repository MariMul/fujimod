//! Direct, reverse-engineered firmware commands; no chunking or hidden polling.
pub mod capabilities;
pub mod configuration;
pub mod debug;
pub mod diagnostic;
pub mod info;
pub mod memory;

use crate::{Error, Result, error::ProtocolError};

pub(crate) const HEADER: usize = 20;
pub(crate) const PARAMETERS: usize = 16;

pub(crate) fn memory_request(
    command: u32,
    function: u32,
    parameters: &[u8; 16],
    data: &[u8],
) -> Vec<u8> {
    let mut request = vec![0; HEADER + PARAMETERS];
    request[..4].copy_from_slice(&command.to_le_bytes());
    request[16..20].copy_from_slice(&function.to_le_bytes());
    request[20..].copy_from_slice(parameters);
    request.extend_from_slice(data);
    request
}

pub(crate) fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("checked response extent"),
    )
}

pub(crate) fn status(response: &[u8], command: u32, header: usize) -> Result<()> {
    if response.len() < header {
        return Err(ProtocolError::ReplyLength {
            expected: header,
            actual: response.len(),
        }
        .into());
    }
    let actual = word(response, 0);
    if actual != command {
        return Err(ProtocolError::CommandEcho {
            expected: command,
            actual,
        }
        .into());
    }
    if word(response, 12) != 0 {
        return Err(ProtocolError::ReservedField(word(response, 12)).into());
    }
    let (primary, secondary) = (word(response, 4), word(response, 8));
    if primary != 0 || secondary != 0 {
        return Err(Error::DeviceStatus {
            command,
            primary,
            secondary,
        });
    }
    Ok(())
}

pub(crate) fn parameter_reply(
    response: Vec<u8>,
    command: u32,
    function: u32,
    parameters: &[u8; 16],
    length: usize,
) -> Result<Vec<u8>> {
    // Validate ownership even for an error reply before allowing session reuse.
    if response.len() >= HEADER && word(&response, 16) != function {
        let actual = word(&response, 16);
        return Err(ProtocolError::FunctionEcho {
            expected: function,
            actual,
        }
        .into());
    }
    status(&response, command, HEADER)?;
    let expected = HEADER + PARAMETERS + length;
    if response.len() != expected {
        return Err(ProtocolError::ReplyLength {
            expected,
            actual: response.len(),
        }
        .into());
    }
    if response[HEADER..HEADER + PARAMETERS] != *parameters {
        return Err(ProtocolError::ParameterEcho.into());
    }
    Ok(response[HEADER + PARAMETERS..].to_vec())
}
