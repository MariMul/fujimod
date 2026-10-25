use crate::{
    image::{self, File, Packaging, Plan, Result, require},
    xp4,
};
use serde::{Deserialize, Serialize};
mod template {
    include!("lupi.generated.rs");
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lupi {
    pub open: u32,
    pub read: u32,
    pub filesystem: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware_transport: Option<crate::lupi_firmware::Transport>,
}
pub fn stage(api: &Lupi, ram: &[u8]) -> Result<Vec<u8>> {
    require(
        !ram.is_empty() && ram.len() < 0x40000000,
        "Invalid Lupi RAM extent",
    )?;
    require(
        api.open & 3 == 0
            && api.read & 3 == 0
            && (0x80000000..0x80002000).contains(&api.open)
            && (0x80000000..0x80002000).contains(&api.read)
            && api.filesystem == 0xfff00000,
        "Lupi filesystem ABI differs",
    )?;
    let mut bytes = template::BYTES.to_vec();
    for (i, value) in [
        api.open,
        api.read,
        api.filesystem,
        ram.len() as u32,
        xp4::crc32(ram),
    ]
    .into_iter()
    .enumerate()
    {
        image::put(&mut bytes, template::CONFIG + i * 4, value)?;
    }
    require(
        bytes.len() < 0x10000,
        "Lupi first read truncates sizes above 65535",
    )?;
    Ok(bytes)
}
pub fn package(
    api: &Lupi,
    header: &Packaging,
    inverted: &[u8],
    plan: &Plan,
    mut ram: Vec<u8>,
) -> Result<Vec<File>> {
    require(
        plan.entry == 0 && header.entry == 0 && header.load_address == 0,
        "Lupi handoff differs",
    )?;
    let system = header.header(inverted)?;
    require(system.len() == 512, "Lupi requires a 512-byte FFSYS header")?;
    require(
        header.header_filename == "ffsys.bin" && header.image_filename == "ffboot.bin",
        "Lupi ROM filenames differ",
    )?;
    let storage = api
        .firmware_transport
        .as_ref()
        .map(|transport| transport.install(api, inverted, plan, &mut ram))
        .transpose()?;
    let mut files = vec![
        File::new("ffsys.bin", system)?,
        File::new("ffboot.bin", stage(api, &ram)?)?,
        File::new("ffimage.bin", ram)?,
    ];
    if let Some(storage) = storage {
        files.push(File::new("ffstore.bin", storage)?);
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stage_configuration_uses_full_payload_size_and_crc_and_u16_boot_extent() {
        let api = Lupi {
            open: 0x80001a74,
            read: 0x80001b38,
            filesystem: 0xfff00000,
            firmware_transport: None,
        };
        let payload = vec![0x35; 70001];
        let bytes = stage(&api, &payload).unwrap();
        assert!(bytes.len() < 0x10000);
        assert_eq!(image::word(&bytes, template::CONFIG + 12).unwrap(), 70001);
        assert_eq!(
            image::word(&bytes, template::CONFIG + 16).unwrap(),
            xp4::crc32(&payload)
        );
        assert!(stage(&api, &[]).is_err());
        assert!(stage(&Lupi { read: 0, ..api }, &payload).is_err());
    }
}
