//! Partial evaluation of fixed-function control registers. Addresses, colors,
//! extents and vertex uniforms stay dynamic. The key contains every constant
//! substituted into the shared fragment equations.
const REGISTERS: [usize; 22] = [
    0x280, 0x283, 0x293, 0x28e, 0x296, 0x300, 0x301, 0x304, 0x305, 0x306, 0x307, 0x316, 0x317,
    0x2e0, 0x2c0, 0x2c1, 0x2c2, 0x2c4, 0x2c8, 0x2c9, 0x2ca, 0x2cc,
];

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct FragmentProgram {
    pub(super) flat_shading: bool,
    controls: Option<[u32; REGISTERS.len()]>,
}
impl FragmentProgram {
    pub(super) fn new(flat_shading: bool, parameters: &[u32]) -> Self {
        Self {
            flat_shading,
            controls: Some(REGISTERS.map(|index| parameters[index])),
        }
    }
    pub(super) fn generic(flat_shading: bool) -> Self {
        Self {
            flat_shading,
            controls: None,
        }
    }
    pub(super) fn constants(&self) -> Vec<(String, f64)> {
        let Some(controls) = self.controls else {
            return Vec::new();
        };
        std::iter::once(("fixed_controls".to_owned(), 1.0))
            .chain(
                REGISTERS
                    .iter()
                    .zip(controls)
                    .map(|(index, value)| (format!("control_{index}"), f64::from(value))),
            )
            .collect()
    }
    pub(super) fn shader(source: String) -> String {
        let mut function = String::from("override fixed_controls: bool = false;\n");
        for index in REGISTERS {
            function.push_str(&format!("override control_{index}: u32 = 0u;\n"));
        }
        function.push_str("fn reg(case_index: u32, index: u32) -> u32 {\nif !fixed_controls { return param(case_index, index); }\n");
        for index in REGISTERS {
            function.push_str(&format!(
                "if index == {index}u {{ return control_{index}; }}\n"
            ));
        }
        function.push_str("return param(case_index, index);\n}");
        source.replace(
            "fn reg(case_index: u32, index: u32) -> u32 {\n    return param(case_index, index);\n}",
            &function,
        )
    }
}
