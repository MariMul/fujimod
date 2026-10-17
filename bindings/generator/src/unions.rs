//! Payload enums: Rust field names define factories and discriminated replies.
use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValueType {
    Integer {
        bits: u8,
        signed: bool,
    },
    Named {
        name: String,
    },
    Bytes,
    Array {
        bits: u8,
        signed: bool,
        length: usize,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct Field {
    pub name: String,
    pub ty: ValueType,
}
#[derive(Clone, Debug, Serialize)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Field>,
    pub tuple: bool,
    pub unit: bool,
    pub factory: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Union {
    pub name: String,
    pub variants: Vec<Variant>,
    pub input: bool,
    pub kind_name: String,
    pub kind_export: String,
}
fn value_type(ty: &Type) -> Result<ValueType, String> {
    if let Some(name) = named(ty) {
        return Ok(match name.as_str() {
            "u8" => ValueType::Integer {
                bits: 8,
                signed: false,
            },
            "i8" => ValueType::Integer {
                bits: 8,
                signed: true,
            },
            "u16" => ValueType::Integer {
                bits: 16,
                signed: false,
            },
            "i16" => ValueType::Integer {
                bits: 16,
                signed: true,
            },
            "u32" => ValueType::Integer {
                bits: 32,
                signed: false,
            },
            "i32" => ValueType::Integer {
                bits: 32,
                signed: true,
            },
            _ => ValueType::Named { name },
        });
    }
    if let Type::Array(array) = ty
        && let ValueType::Integer { bits, signed } = value_type(&array.elem)?
        && let syn::Expr::Lit(lit) = &array.len
        && let syn::Lit::Int(length) = &lit.lit
    {
        return Ok(ValueType::Array {
            bits,
            signed,
            length: length.base10_parse().map_err(|e| e.to_string())?,
        });
    }
    if let Type::Path(path) = ty
        && let Some(last) = path.path.segments.last()
        && last.ident == "Vec"
        && let syn::PathArguments::AngleBracketed(args) = &last.arguments
        && args.args.len() == 1
        && let Some(syn::GenericArgument::Type(ty)) = args.args.first()
        && named(ty).as_deref() == Some("u8")
    {
        return Ok(ValueType::Bytes);
    }
    Err(
        "unsupported payload field type; use a named value, integer, bytes or fixed integer array"
            .into(),
    )
}
impl Union {
    pub fn parse(item: &syn::ItemEnum) -> Result<Self, String> {
        let name = item.ident.to_string();
        let mut variants = Vec::new();
        for variant in &item.variants {
            let variant_name = variant.ident.to_string();
            let fields = match &variant.fields {
                Fields::Unit => Vec::new(),
                Fields::Named(fields) => fields
                    .named
                    .iter()
                    .map(|field| {
                        Ok(Field {
                            name: field.ident.as_ref().unwrap().to_string(),
                            ty: value_type(&field.ty)?,
                        })
                    })
                    .collect::<Result<_, String>>()?,
                // A common opaque result byte has one consistent cross-language name.
                Fields::Unnamed(fields)
                    if variant_name == "Unknown"
                        && fields.unnamed.len() == 1
                        && named(&fields.unnamed[0].ty).as_deref() == Some("u8") =>
                {
                    vec![Field {
                        name: "raw_status".into(),
                        ty: ValueType::Integer {
                            bits: 8,
                            signed: false,
                        },
                    }]
                }
                _ => {
                    return Err(format!(
                        "{name}::{variant_name}: payloads need named Rust fields"
                    ));
                }
            };
            variants.push(Variant {
                factory: camel(&variant_name),
                name: variant_name,
                fields,
                tuple: matches!(variant.fields, Fields::Unnamed(_)),
                unit: matches!(variant.fields, Fields::Unit),
            });
        }
        Ok(Self {
            kind_export: if name == "ReplyValue" {
                "ReplyKind".into()
            } else if name == "WordReadResult" {
                "WordReadKind".into()
            } else {
                format!("{name}Kind")
            },
            kind_name: match name.as_str() {
                "ReplyValue" => "App80ReplyKind".into(),
                "WordReadResult" => "WordReadKind".into(),
                _ => format!("{name}Kind"),
            },
            name,
            variants,
            input: false,
        })
    }
    pub fn wasm_name(&self) -> &str {
        match self.name.as_str() {
            "Command" => "App80Command",
            "ReplyValue" => "App80ReplyValue",
            _ => &self.name,
        }
    }
    fn pattern(&self, variant: &Variant) -> String {
        let names = variant
            .fields
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let suffix = if variant.tuple {
            format!("({names})")
        } else if variant.unit {
            String::new()
        } else {
            format!("{{{names}}}")
        };
        format!("core::{}::{}{suffix}", self.name, variant.name)
    }
    pub fn python(&self, values: &Values) -> String {
        let n = &self.name;
        let mut out = format!(
            "#[pyclass(frozen,from_py_object,module=\"fujimod.usb.app80\")] #[derive(Clone)] pub(super) enum {n} {{\n"
        );
        for v in &self.variants {
            out += &format!(
                "{}{{{}}},\n",
                v.name,
                v.fields
                    .iter()
                    .map(|f| format!("{}:{}", f.name, f.ty.python_type()))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        out += "}\n";
        if self.input {
            out += &format!(
                "impl From<{n}> for core::{n} {{fn from(value:{n})->Self{{match value{{\n"
            );
            for v in &self.variants {
                let names = v
                    .fields
                    .iter()
                    .map(|f| f.name.as_str())
                    .collect::<Vec<_>>()
                    .join(",");
                let fields = v
                    .fields
                    .iter()
                    .map(|f| named_field(&f.name, &f.ty.python_input(&f.name, values)))
                    .collect::<Vec<_>>()
                    .join(",");
                out += &format!(
                    "{n}::{}{{{names}}}=>core::{n}::{}{},\n",
                    v.name,
                    v.name,
                    if v.unit {
                        String::new()
                    } else {
                        format!("{{{fields}}}")
                    }
                );
            }
        } else {
            out += &format!(
                "impl From<core::{n}> for {n} {{fn from(value:core::{n})->Self{{match value{{\n"
            );
            for v in &self.variants {
                let fields = v
                    .fields
                    .iter()
                    .map(|f| named_field(&f.name, &f.ty.python_output(&f.name)))
                    .collect::<Vec<_>>()
                    .join(",");
                out += &format!("{}=>Self::{}{{{fields}}},\n", self.pattern(v), v.name);
            }
        }
        out += "}}}\n";
        out
    }
    pub fn javascript(&self, values: &Values) -> String {
        if !self.input {
            return self.javascript_reply(values);
        }
        let n = self.wasm_name();
        let mut out = format!(
            "#[wasm_bindgen] pub struct {n}{{pub(super) inner:core::{}}}\n#[wasm_bindgen] impl {n}{{\n",
            self.name
        );
        for v in &self.variants {
            let checked = v.fields.iter().any(|f| f.ty.checked(values));
            let args = v
                .fields
                .iter()
                .map(|f| f.ty.javascript_arg(&f.name, values))
                .collect::<Vec<_>>()
                .join(",");
            let fields = v
                .fields
                .iter()
                .map(|f| named_field(&f.name, &f.ty.javascript_input(&f.name, values)))
                .collect::<Vec<_>>()
                .join(",");
            let inner = format!(
                "Self{{inner:core::{}::{}{}}}",
                self.name,
                v.name,
                if v.unit {
                    String::new()
                } else {
                    format!("{{{fields}}}")
                }
            );
            out += &format!(
                "#[wasm_bindgen(js_name={})] pub fn {}({args})->{}{{{}}}\n",
                v.factory,
                snake(&v.name),
                if checked {
                    "Result<Self,JsValue>"
                } else {
                    "Self"
                },
                if checked {
                    format!("Ok({inner})")
                } else {
                    inner
                }
            );
        }
        out += "}\n";
        out
    }
    fn javascript_reply(&self, values: &Values) -> String {
        let k = &self.kind_name;
        let mut out = format!(
            "#[wasm_bindgen] #[derive(Clone,Copy)] pub enum {k}{{{}}}\nimpl {k}{{pub(super) fn value(value:&core::{})->JsValue{{match value{{\n",
            self.variants
                .iter()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>()
                .join(","),
            self.name
        );
        for v in &self.variants {
            let fields = v
                .fields
                .iter()
                .map(|f| {
                    format!(
                        "(\"{}\",{})",
                        camel(&f.name),
                        f.ty.javascript_output(&f.name, values)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            out += &format!(
                "{}=>record(Self::{} as u32,&[{fields}]),\n",
                self.pattern(v),
                v.name
            );
        }
        out += "}}}\n";
        let ts = self
            .variants
            .iter()
            .map(|v| {
                let fields = v
                    .fields
                    .iter()
                    .map(|f| format!("{}:{}", camel(&f.name), f.ty.typescript()))
                    .collect::<Vec<_>>()
                    .join(";");
                format!("Readonly<{{kind:{k}.{};{fields}}}>", v.name)
            })
            .collect::<Vec<_>>()
            .join(" | ");
        out += &format!(
            "#[wasm_bindgen(typescript_custom_section)] const GENERATED_{}:&str=r#\"export type {}={ts};\"#;\n",
            snake(&self.name).to_uppercase(),
            self.wasm_name()
        );
        out
    }
}
impl ValueType {
    fn python_type(&self) -> String {
        match self {
            Self::Integer { bits, signed } => format!("{}{bits}", if *signed { "i" } else { "u" }),
            Self::Named { name } => name.clone(),
            Self::Bytes
            | Self::Array {
                bits: 8,
                signed: false,
                ..
            } => "Vec<u8>".into(),
            Self::Array {
                bits,
                signed,
                length,
            } => format!(
                "({})",
                std::iter::repeat_n(
                    format!("{}{bits},", if *signed { "i" } else { "u" }),
                    *length
                )
                .collect::<String>()
            ),
        }
    }
    fn python_input(&self, name: &str, values: &Values) -> String {
        match self {
            Self::Integer { .. } => name.into(),
            Self::Named { name: ty } if values.quantities.iter().any(|q| q.name == *ty) => {
                format!("{name}.inner")
            }
            Self::Named { .. } => format!("{name}.into()"),
            _ => unreachable!("input buffers need checked Rust value wrappers"),
        }
    }
    fn python_output(&self, name: &str) -> String {
        match self {
            Self::Named { .. } => format!("{name}.into()"),
            Self::Array {
                bits: 8,
                signed: false,
                ..
            } => format!("{name}.to_vec()"),
            Self::Array { length, .. } => format!(
                "({})",
                (0..*length)
                    .map(|i| format!("{name}[{i}],"))
                    .collect::<String>()
            ),
            _ => name.into(),
        }
    }
    fn is_choice(&self, values: &Values) -> bool {
        matches!(self,Self::Named{name} if values.choices.iter().any(|c|c.name==*name && !c.result))
    }
    fn checked(&self, values: &Values) -> bool {
        matches!(self, Self::Integer { .. }) || self.is_choice(values)
    }
    fn javascript_arg(&self, name: &str, values: &Values) -> String {
        match self {
            Self::Integer { .. } => {
                format!("#[wasm_bindgen(unchecked_param_type=\"number\")] {name}:JsValue")
            }
            Self::Named { name: ty } if self.is_choice(values) => {
                format!("#[wasm_bindgen(unchecked_param_type=\"{ty}\")] {name}:JsValue")
            }
            Self::Named { name: ty } => format!("{name}:&{ty}"),
            _ => unreachable!("input buffers need checked Rust value wrappers"),
        }
    }
    fn javascript_input(&self, name: &str, values: &Values) -> String {
        match self {
            Self::Integer { bits, signed: true } => format!(
                "error::signed_integer({name},i{bits}::MIN.into(),i{bits}::MAX.into())? as i{bits}"
            ),
            Self::Integer {
                bits: 32,
                signed: false,
            } => format!("integer({name},u32::MAX)?"),
            Self::Integer {
                bits,
                signed: false,
            } => format!("integer({name},u{bits}::MAX.into())? as u{bits}"),
            Self::Named { name: ty } if self.is_choice(values) => format!("{ty}::parse({name})?"),
            Self::Named { .. } => format!("{name}.inner"),
            _ => unreachable!(),
        }
    }
    fn javascript_output(&self, name: &str, values: &Values) -> String {
        match self {
            Self::Integer { .. } => format!("(*{name}).into()"),
            Self::Named { name: ty } => {
                if let Some(c) = values.choices.iter().find(|c| c.name == *ty) {
                    if c.result {
                        format!("{}::value(*{name})", kind_name(ty))
                    } else {
                        format!("({ty}::from(*{name}) as u32).into()")
                    }
                } else if values.quantities.iter().any(|q| q.name == *ty) {
                    format!("{ty}{{inner:*{name}}}.into()")
                } else {
                    let u = values.unions.iter().find(|u| u.name == *ty).unwrap();
                    format!("{}::value({name})", u.kind_name)
                }
            }
            Self::Bytes
            | Self::Array {
                bits: 8,
                signed: false,
                ..
            } => format!("error::bytes({name})"),
            Self::Array { .. } => format!(
                "{{let array:js_sys::Array={name}.iter().map(|&n|JsValue::from(n)).collect();js_sys::Object::freeze(&array).into()}}"
            ),
        }
    }
    fn typescript(&self) -> String {
        match self {
            Self::Integer { .. } => "number".into(),
            Self::Named { name } => name.clone(),
            Self::Bytes
            | Self::Array {
                bits: 8,
                signed: false,
                ..
            } => "Uint8Array".into(),
            Self::Array { length, .. } => format!(
                "readonly [{}]",
                std::iter::repeat_n("number", *length)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}
pub fn classify(values: &mut Values) -> Result<(), String> {
    fn reach(name: &str, unions: &[Union], set: &mut BTreeSet<String>) {
        if !set.insert(name.into()) {
            return;
        }
        if let Some(union) = unions.iter().find(|u| u.name == name) {
            for field in union.variants.iter().flat_map(|v| &v.fields) {
                if let ValueType::Named { name } = &field.ty {
                    reach(name, unions, set);
                }
            }
        }
    }
    let mut inputs = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    reach("Command", &values.unions, &mut inputs);
    reach("ReplyValue", &values.unions, &mut outputs);
    let known: BTreeSet<_> = values
        .choices
        .iter()
        .map(|c| c.name.clone())
        .chain(values.quantities.iter().map(|q| q.name.clone()))
        .chain(values.unions.iter().map(|u| u.name.clone()))
        .collect();
    for union in &mut values.unions {
        if inputs.contains(&union.name) == outputs.contains(&union.name) {
            return Err(format!(
                "{} must be reachable from exactly one of Command or ReplyValue",
                union.name
            ));
        }
        union.input = inputs.contains(&union.name);
        for field in union.variants.iter().flat_map(|v| &v.fields) {
            if let ValueType::Named { name } = &field.ty {
                if union.input && values.choices.iter().any(|c| c.name == *name && c.result) {
                    return Err(format!(
                        "{}: result enums cannot be command arguments",
                        union.name
                    ));
                }
                if !known.contains(name) {
                    return Err(format!("{}: unknown field type {name}", union.name));
                }
            } else if union.input && !matches!(field.ty, ValueType::Integer { .. }) {
                return Err(format!(
                    "{}: input buffers need checked Rust value wrappers",
                    union.name
                ));
            }
        }
    }
    Ok(())
}
pub fn snake(name: &str) -> String {
    let chars: Vec<_> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase()
            && i > 0
            && (chars[i - 1].is_lowercase()
                || chars[i - 1].is_ascii_digit()
                || chars.get(i + 1).is_some_and(|c| c.is_lowercase())
                    && chars[i - 1].is_uppercase())
        {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}
pub fn camel(name: &str) -> String {
    let snake = snake(name);
    let mut upper = false;
    let mut out = String::new();
    for c in snake.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn named_field(name: &str, value: &str) -> String {
    if name == value {
        name.into()
    } else {
        format!("{name}:{value}")
    }
}
