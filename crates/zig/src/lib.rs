use heck::{ToKebabCase, ToSnakeCase, ToUpperCamelCase};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::str::FromStr;
use std::{fmt, mem};
use wit_bindgen_core::abi::{self, AbiVariant, LiftLower};
use wit_bindgen_core::{wit_parser::*, Source, Types, WorldGenerator};
mod go;
mod rust;

const ZIGKEYWORDS: [&str; 0] = [];
fn avoid_keyword(s: &str) -> String {
    if ZIGKEYWORDS.contains(&s) {
        format!("{s}_")
    } else {
        s.into()
    }
}
#[derive(Default, Debug, Clone, Copy)]
pub enum Ownership {
    /// Generated types will be composed entirely of owning fields, regardless
    /// of whether they are used as parameters to imports or not.
    #[default]
    Owning,

    /// Generated types used as parameters to imports will be "deeply
    /// borrowing", i.e. contain references rather than owned values when
    /// applicable.
    Borrowing {
        /// Whether or not to generate "duplicate" type definitions for a single
        /// WIT type if necessary, for example if it's used as both an import
        /// and an export, or if it's used both as a parameter to an import and
        /// a return value from an import.
        duplicate_if_necessary: bool,
    },
}

impl FromStr for Ownership {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "owning" => Ok(Self::Owning),
            "borrowing" => Ok(Self::Borrowing {
                duplicate_if_necessary: false,
            }),
            "borrowing-duplicate-if-necessary" => Ok(Self::Borrowing {
                duplicate_if_necessary: true,
            }),
            _ => Err(format!(
                "unrecognized ownership: `{s}`; \
               expected `owning`, `borrowing`, or `borrowing-duplicate-if-necessary`"
            )),
        }
    }
}

impl fmt::Display for Ownership {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Ownership::Owning => "owning",
            Ownership::Borrowing {
                duplicate_if_necessary: false,
            } => "borrowing",
            Ownership::Borrowing {
                duplicate_if_necessary: true,
            } => "borrowing-duplicate-if-necessary",
        })
    }
}

#[cfg(feature = "clap")]
fn iterate_hashmap_string(s: &str) -> impl Iterator<Item = Result<(&str, &str), String>> {
    s.split(',').map(move |entry| {
        entry.split_once('=').ok_or_else(|| {
            format!("expected string of form `<key>=<value>[,<key>=<value>...]`; got `{s}`")
        })
    })
}

#[cfg(feature = "clap")]
fn parse_exports(s: &str) -> Result<HashMap<ExportKey, String>, String> {
    if s.is_empty() {
        Ok(HashMap::default())
    } else {
        iterate_hashmap_string(s)
            .map(|entry| {
                let (key, value) = entry?;
                Ok((
                    match key {
                        "world" => ExportKey::World,
                        _ => ExportKey::Name(key.to_owned()),
                    },
                    value.to_owned(),
                ))
            })
            .collect()
    }
}

enum Identifier<'a> {
    World(WorldId),
    Interface(InterfaceId, &'a WorldKey),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExportKey {
    World,
    Name(String),
}

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct Opts {
    /// Whether or not `rustfmt` is executed to format generated code.
    #[cfg_attr(feature = "clap", arg(long))]
    pub rustfmt: bool,

    /// If true, code generation should qualify any features that depend on
    /// `std` with `cfg(feature = "std")`.
    #[cfg_attr(feature = "clap", arg(long))]
    pub std_feature: bool,

    /// If true, code generation should pass borrowed string arguments as
    /// `&[u8]` instead of `&str`. Strings are still required to be valid
    /// UTF-8, but this avoids the need for Rust code to do its own UTF-8
    /// validation if it doesn't already have a `&str`.
    #[cfg_attr(feature = "clap", arg(long))]
    pub raw_strings: bool,

    /// Names of functions to skip generating bindings for.
    #[cfg_attr(feature = "clap", arg(long))]
    pub skip: Vec<String>,

    /// Names of the concrete types which implement the traits representing any
    /// functions, interfaces, and/or resources exported by the world.
    ///
    /// Example: `--exports world=MyWorld,ns:pkg/iface1=MyIface1,ns:pkg/iface1/resource1=MyResource1`,
    #[cfg_attr(feature = "clap", arg(long, value_parser = parse_exports, default_value = ""))]
    pub exports: HashMap<ExportKey, String>,

    /// If true, generate stub implementations for any exported functions,
    /// interfaces, and/or resources.
    #[cfg_attr(feature = "clap", arg(long))]
    pub stubs: bool,

    /// Optionally prefix any export names with the specified value.
    ///
    /// This is useful to avoid name conflicts when testing.
    #[cfg_attr(feature = "clap", arg(long))]
    pub export_prefix: Option<String>,

    /// Whether to generate owning or borrowing type definitions.
    ///
    /// Valid values include:
    ///
    /// - `owning`: Generated types will be composed entirely of owning fields,
    /// regardless of whether they are used as parameters to imports or not.
    ///
    /// - `borrowing`: Generated types used as parameters to imports will be
    /// "deeply borrowing", i.e. contain references rather than owned values
    /// when applicable.
    ///
    /// - `borrowing-duplicate-if-necessary`: As above, but generating distinct
    /// types for borrowing and owning, if necessary.
    #[cfg_attr(feature = "clap", arg(long, default_value_t = Ownership::Owning))]
    pub ownership: Ownership,

    /// The optional path to the wit-bindgen runtime module to use.
    ///
    /// This defaults to `wit_bindgen::rt`.
    #[cfg_attr(feature = "clap", arg(long))]
    pub runtime_path: Option<String>,

    /// The optional path to the bitflags crate to use.
    ///
    /// This defaults to `wit_bindgen::bitflags`.
    #[cfg_attr(feature = "clap", arg(long))]
    pub bitflags_path: Option<String>,

    /// Additional derive attributes to add to generated types. If using in a CLI, this flag can be
    /// specified multiple times to add multiple attributes.
    ///
    /// These derive attributes will be added to any generated structs or enums
    #[cfg_attr(feature = "clap", arg(long = "additional_derive_attribute", short = 'd', default_values_t = Vec::<String>::new()))]
    pub additional_derive_attributes: Vec<String>,

    /// Remapping of interface names to rust module names.
    #[cfg_attr(feature = "clap", arg(long, value_parser = parse_with, default_value = ""))]
    pub with: HashMap<String, String>,
}

#[cfg(feature = "clap")]
fn parse_with(s: &str) -> Result<HashMap<String, String>, String> {
    if s.is_empty() {
        Ok(HashMap::default())
    } else {
        iterate_hashmap_string(s)
            .map(|entry| {
                let (key, value) = entry?;
                Ok((key.to_owned(), value.to_owned()))
            })
            .collect()
    }
}

impl Opts {
    pub fn build(self) -> Box<dyn WorldGenerator> {
        let mut r = ZigWasm::new();
        r.skip = self.skip.iter().cloned().collect();
        r.opts = self;
        Box::new(r)
    }
}

#[derive(Default, Copy, Clone, PartialEq, Eq)]
enum Direction {
    #[default]
    Import,
    Export,
}

#[derive(Default)]
struct ResourceInfo {
    // Note that a resource can be both imported and exported (e.g. when
    // importing and exporting the same interface which contains one or more
    // resources).  In that case, this field will be `Import` while we're
    // importing the interface and later change to `Export` while we're
    // exporting the interface.
    direction: Direction,
    owned: bool,
}

struct InterfaceName {
    /// True when this interface name has been remapped through the use of `with` in the `bindgen!`
    /// macro invocation.
    remapped: bool,

    /// The string name for this interface.
    path: String,
}
#[derive(Default)]
struct ZigWasm {
    types: Types,
    src: Source,
    world: String,
    opts: Opts,
    needs_result_option: bool,
    interface_names: HashMap<InterfaceId, WorldKey>,
    import_modules: Vec<(String, Vec<String>)>,
    export_modules: Vec<(String, Vec<String>)>,
    skip: HashSet<String>,
    // interface_names: HashMap<InterfaceId, InterfaceName>,
    resources: HashMap<TypeId, ResourceInfo>,
    import_funcs_called: bool,
    with_name_counter: usize,
}

impl ZigWasm {
    fn new() -> ZigWasm {
        ZigWasm::default()
    }

    fn get_zig_ty(&self, ty: &Type) -> String {
        match ty {
            Type::Bool => "bool".into(),
            Type::U8 => "u8".into(),
            Type::U16 => "u16".into(),
            Type::U32 => "u32".into(),
            Type::U64 => "u64".into(),
            Type::S8 => "s8".into(),
            Type::S16 => "s16".into(),
            Type::S32 => "s32".into(),
            Type::S64 => "s64".into(),
            Type::Float32 => todo!(),
            Type::Float64 => todo!(),
            Type::Char => todo!(),
            Type::String => "[]u8".into(),
            Type::Id(_) => todo!(),
        }
    }

    fn interface<'a>(
        &'a mut self,
        // identifier: Identifier<'a>,
        // wasm_import_module: Option<&'a str>,
        resolve: &'a Resolve,
        name: &'a Option<&'a WorldKey>,
        in_import: bool,
    ) -> InterfaceGenerator<'a> {
        InterfaceGenerator {
            src: Source::default(),
            gen: self,
            resolve,
            interface: None,
            // name: identifier,
            name,
            public_anonymous_types: BTreeSet::new(),
            in_import,
            export_funcs: Vec::new(),
        }
    }
}

impl WorldGenerator for ZigWasm {
    fn preprocess(&mut self, resolve: &Resolve, world: WorldId) {
        let name = &resolve.worlds[world].name;
        self.world = name.to_string();
        // self.sizes.fill(resolve);
    }

    fn import_interface(
        &mut self,
        resolve: &Resolve,
        name: &WorldKey,
        iface: InterfaceId,
        files: &mut wit_bindgen_core::Files,
    ) {
        let gen_name = Some(name);
        let mut gen = self.interface(resolve, &gen_name, false);
        let func_prefix = gen.get_package_name();
        for (_name, func) in resolve.interfaces[iface].functions.iter() {
            gen.export(resolve, func, Some(func_prefix.clone()));
        }
        let src = mem::take(&mut gen.src);
        self.src.push_str(&src);
    }

    fn export_interface(
        &mut self,
        resolve: &Resolve,
        name: &WorldKey,
        iface: InterfaceId,
        files: &mut wit_bindgen_core::Files,
    ) -> anyhow::Result<()> {
        dbg!("EXPORTING INTERFACE");
        // let mut export_names = Vec::new();
        // let mut post_return_names = Vec::new();
        let iface = &resolve.interfaces[iface];
        // dbg!(&iface.name.as_ref().unwrap());
        self.src.push_str(&format!(
            "const {} = struct {{\n",
            iface.name.as_ref().unwrap().to_upper_camel_case(),
        ));
        for (_name, func) in iface.functions.iter() {
            self.src.push_str(&format!("fn {}(", &func.name));
            // export_names.push(&func.name);
            // if abi::guest_export_needs_post_return(resolve, func) {
            // post_return_names.push(&func.name);
            // };
            for (name, ty) in &func.params {
                self.src
                    .push_str(&format!("{name}: {}, ", self.get_zig_ty(ty)));
            }
            match func.results.len() {
                0 => {}
                1 => {
                    let res = func.results.iter_types().last().unwrap();
                    self.src
                        .push_str(&format!(") {} {{}}\n", self.get_zig_ty(res)));
                }
                _ => {}
            }
        }
        self.src.push_str("};\n\n");

        Ok(())
    }

    fn import_funcs(
        &mut self,
        resolve: &Resolve,
        world: WorldId,
        funcs: &[(&str, &Function)],
        files: &mut wit_bindgen_core::Files,
    ) {
        todo!()
    }

    fn export_funcs(
        &mut self,
        resolve: &Resolve,
        world: WorldId,
        funcs: &[(&str, &Function)],
        files: &mut wit_bindgen_core::Files,
    ) -> anyhow::Result<()> {
        let name = &resolve.worlds[world].name;
        let mut gen = self.interface(resolve, &None, false);
        // let mut gen = self.interface(Identifier::World(world), None, resolve, false);
        for (name, func) in funcs.iter() {
            gen.export(resolve, func, None);
        }
        gen.finish();
        let src = mem::take(&mut gen.src);
        self.src.push_str(&src);
        Ok(())
    }

    fn import_types(
        &mut self,
        resolve: &Resolve,
        world: WorldId,
        types: &[(&str, TypeId)],
        files: &mut wit_bindgen_core::Files,
    ) {
        todo!()
    }

    fn finish(&mut self, resolve: &Resolve, id: WorldId, files: &mut wit_bindgen_core::Files) {
        // todo!()
        // let snake = self.world.to_snake_case();
        // self.src.push_str("package ");
        // self.src.push_str(&snake);
        // self.src.push_str("\n\n");
        let src = mem::take(&mut self.src);
        let world = &resolve.worlds[id];
        self.src.push_str(
            "const std = @import(\"std\");
        const mem = std.mem;
        var gpa = std.heap.GeneralPurposeAllocator(.{}){};
        const allocator = gpa.allocator();
        
        fn alloc(len: usize) [*]u8 {
            const buf = allocator.alloc(u8, len) catch |e| {
                std.debug.panic(\"FAILED TO ALLOC MEM {}\", .{e});
            };
            return buf.ptr;
        }
        
        export fn cabi_realloc(origPtr: *[]u8, origSize: u8, alignment: u8, newSize: u8) ?[*]u8 {
            _ = origSize;
            _ = alignment;
            const buf = allocator.realloc(origPtr.*, newSize) catch {
                return null;
            };
            return buf.ptr;
        }

        ",
        );
        self.src.push_str(&src);
        let mut export_names = Vec::new();
        let mut post_return_names = Vec::new();
        self.src.push_str("const Guest = struct {\n");
        for (_, world_item) in &world.exports {
            match world_item {
                WorldItem::Interface(iface) => {}
                WorldItem::Function(func) => {
                    self.src.push_str(&format!("fn {}(", &func.name));
                    export_names.push(&func.name);
                    if abi::guest_export_needs_post_return(resolve, func) {
                        post_return_names.push(&func.name);
                    };
                    for (name, ty) in &func.params {
                        self.src
                            .push_str(&format!("{name}: {}, ", self.get_zig_ty(ty)));
                    }
                    match func.results.len() {
                        0 => {}
                        1 => {
                            let res = func.results.iter_types().last().unwrap();
                            self.src
                                .push_str(&format!(") {} {{}}\n", self.get_zig_ty(res)));
                        }
                        _ => {}
                    }
                }
                WorldItem::Type(_) => todo!(),
            }
        }
        self.src.push_str(
            "};

            comptime {
        ",
        );
        for name in export_names {
            self.src.push_str(&format!(
                "@export(__export_{name}, .{{ .name = \"{name}\" }});\n"
            ));
        }
        for name in post_return_names {
            self.src.push_str(&format!(
                "@export(__post_return_{name}, . {{ .name = \"cabi_post_{name}\" }});\n"
            ));
        }
        self.src.push_str(
            "}
        
        pub fn main() void {}",
        );
        // for exp in self.export_funcs(resolve, world, funcs, files)
        files.push(
            &format!("{}.zig", world.name.to_kebab_case()),
            self.src.as_bytes(),
        );
    }
}

struct InterfaceGenerator<'a> {
    src: Source,
    gen: &'a mut ZigWasm,
    resolve: &'a Resolve,
    interface: Option<InterfaceId>,
    name: &'a Option<&'a WorldKey>,
    // name: Identifier<'a>,
    public_anonymous_types: BTreeSet<TypeId>,
    in_import: bool,
    export_funcs: Vec<(String, String)>,
}

impl InterfaceGenerator<'_> {
    fn get_ty_name_with(&self, key: &WorldKey) -> String {
        let mut name = String::new();
        match key {
            WorldKey::Name(k) => name.push_str(&k.to_upper_camel_case()),
            WorldKey::Interface(id) => {
                let iface = &self.resolve.interfaces[*id];
                let pkg = &self.resolve.packages[iface.package.unwrap()];
                name.push_str(&pkg.name.namespace.to_upper_camel_case());
                name.push_str(&pkg.name.name.to_upper_camel_case());
                name.push_str(&iface.name.as_ref().unwrap().to_upper_camel_case());
            }
        }
        name
    }
    fn get_optional_ty(&mut self, ty: Option<&Type>) -> String {
        match ty {
            Some(ty) => self.get_ty(ty),
            None => "struct{}".into(),
        }
    }
    fn resolve(&self) -> &'_ Resolve {
        self.resolve
    }

    fn get_ty(&mut self, ty: &Type) -> String {
        match ty {
            Type::Bool => "bool".into(),
            Type::U8 => "uint8".into(),
            Type::U16 => "uint16".into(),
            Type::U32 => "uint32".into(),
            Type::U64 => "uint64".into(),
            Type::S8 => "int8".into(),
            Type::S16 => "int16".into(),
            Type::S32 => "int32".into(),
            Type::S64 => "int64".into(),
            Type::Float32 => "float32".into(),
            Type::Float64 => "float64".into(),
            Type::Char => "rune".into(),
            Type::String => "string".into(),
            Type::Id(id) => {
                let ty = &self.resolve().types[*id];
                match &ty.kind {
                    wit_bindgen_core::wit_parser::TypeDefKind::Type(ty) => format!("type unimpl"),
                    // self.get_ty(ty),
                    wit_bindgen_core::wit_parser::TypeDefKind::List(ty) => {
                        // format!("[]{}", self.get_ty(ty))
                        format!("list unimpl")
                    }
                    wit_bindgen_core::wit_parser::TypeDefKind::Option(o) => {
                        // self.gen.needs_result_option = true;
                        // format!("Option[{}]", self.get_ty(o))
                        format!("option unimpl")
                    }
                    wit_bindgen_core::wit_parser::TypeDefKind::Result(r) => {
                        // self.gen.needs_result_option = true;
                        // format!(
                        //     "Result[{}, {}]",
                        //     self.get_optional_ty(r.ok.as_ref()),
                        //     self.get_optional_ty(r.err.as_ref())
                        // )
                        format!("result unimpl")
                    }
                    _ => {
                        if let Some(name) = &ty.name {
                            if let TypeOwner::Interface(owner) = ty.owner {
                                let key = &self.gen.interface_names[&owner];
                                let iface = self.get_ty_name_with(key);
                                format!("{iface}{name}", name = name.to_upper_camel_case())
                            } else {
                                self.get_type_name(name, true)
                            }
                        } else {
                            self.public_anonymous_types.insert(*id);
                            self.get_type_name(&self.get_ty_name(&Type::Id(*id)), false)
                        }
                    }
                }
            }
        }
    }

    fn get_ty_name(&self, ty: &Type) -> String {
        match ty {
            Type::Bool => "Bool".into(),
            Type::U8 => "U8".into(),
            Type::U16 => "U16".into(),
            Type::U32 => "U32".into(),
            Type::U64 => "U64".into(),
            Type::S8 => "S8".into(),
            Type::S16 => "S16".into(),
            Type::S32 => "S32".into(),
            Type::S64 => "S64".into(),
            Type::Float32 => "F32".into(),
            Type::Float64 => "F64".into(),
            Type::Char => "Byte".into(),
            Type::String => "String".into(),
            Type::Id(id) => {
                let ty = &self.resolve.types[*id];
                if let Some(name) = &ty.name {
                    let prefix = match ty.owner {
                        TypeOwner::World(owner) => {
                            self.resolve.worlds[owner].name.to_upper_camel_case()
                        }
                        TypeOwner::Interface(owner) => {
                            let key = &self.gen.interface_names[&owner];
                            self.get_ty_name_with(key)
                        }
                        TypeOwner::None => "".into(),
                    };
                    return format!(
                        "{prefix}{name}",
                        prefix = prefix,
                        name = name.to_upper_camel_case()
                    );
                }
                match &ty.kind {
                    TypeDefKind::Type(t) => self.get_ty_name(t),
                    TypeDefKind::Record(_)
                    | TypeDefKind::Resource
                    | TypeDefKind::Flags(_)
                    | TypeDefKind::Enum(_)
                    | TypeDefKind::Variant(_) => {
                        unimplemented!()
                    }
                    TypeDefKind::Tuple(t) => {
                        let mut src = String::new();
                        src.push_str("Tuple");
                        src.push_str(&t.types.len().to_string());
                        for ty in t.types.iter() {
                            src.push_str(&self.get_ty_name(ty));
                        }
                        src.push('T');
                        src
                    }
                    TypeDefKind::Option(t) => {
                        let mut src = String::new();
                        src.push_str("Option");
                        src.push_str(&self.get_ty_name(t));
                        src.push('T');
                        src
                    }
                    TypeDefKind::Result(r) => {
                        let mut src = String::new();
                        src.push_str("Result");
                        src.push_str(&self.get_optional_ty_name(r.ok.as_ref()));
                        src.push_str(&self.get_optional_ty_name(r.ok.as_ref()));
                        src.push('T');
                        src
                    }
                    TypeDefKind::List(t) => {
                        let mut src = String::new();
                        src.push_str("List");
                        src.push_str(&self.get_ty_name(t));
                        src.push('T');
                        src
                    }
                    TypeDefKind::Future(t) => {
                        let mut src = String::new();
                        src.push_str("Future");
                        src.push_str(&self.get_optional_ty_name(t.as_ref()));
                        src.push('T');
                        src
                    }
                    TypeDefKind::Stream(t) => {
                        let mut src = String::new();
                        src.push_str("Stream");
                        src.push_str(&self.get_optional_ty_name(t.element.as_ref()));
                        src.push_str(&self.get_optional_ty_name(t.end.as_ref()));
                        src.push('T');
                        src
                    }
                    TypeDefKind::Handle(Handle::Own(ty)) => {
                        let mut src = String::new();
                        src.push_str("Own");
                        src.push_str(&self.get_ty_name(&Type::Id(*ty)));
                        src.push('T');
                        src
                    }
                    TypeDefKind::Handle(Handle::Borrow(ty)) => {
                        let mut src = String::new();
                        src.push_str("Borrow");
                        src.push_str(&self.get_ty_name(&Type::Id(*ty)));
                        src.push('T');
                        src
                    }
                    TypeDefKind::Unknown => unreachable!(),
                }
            }
        }
    }

    fn get_optional_ty_name(&self, ty: Option<&Type>) -> String {
        match ty {
            Some(ty) => self.get_ty_name(ty),
            None => "Empty".into(),
        }
    }

    fn get_type_name(&self, ty_name: &str, convert: bool) -> String {
        let mut name = String::new();
        let package_name = match self.name {
            Some(key) => self.get_ty_name_with(key),
            None => self.gen.world.to_upper_camel_case(),
        };
        let ty_name = if convert {
            ty_name.to_upper_camel_case()
        } else {
            ty_name.into()
        };
        name.push_str(&package_name);
        name.push_str(&ty_name);
        name
    }

    fn get_func_params(&mut self, _resolve: &Resolve, func: &Function) -> String {
        let mut params = String::new();
        for (i, (name, param)) in func.params.iter().enumerate() {
            if i > 0 {
                params.push_str(", ");
            }

            params.push_str(&avoid_keyword(&name.to_snake_case()));

            params.push(' ');
            params.push_str(&self.get_ty(param));
        }
        params
    }

    fn get_func_signature_no_interface(&mut self, resolve: &Resolve, func: &Function) -> String {
        format!(
            "{}({}){}",
            func.name.to_upper_camel_case(),
            self.get_func_params(resolve, func),
            self.get_func_results(resolve, func)
        )
    }
    fn get_func_results(&mut self, _resolve: &Resolve, func: &Function) -> String {
        let mut results = String::new();
        results.push(' ');
        match func.results.len() {
            0 => {}
            1 => {
                results.push_str(&self.get_ty(func.results.iter_types().next().unwrap()));
                results.push(' ');
            }
            _ => {
                results.push('(');
                for (i, ty) in func.results.iter_types().enumerate() {
                    if i > 0 {
                        results.push_str(", ");
                    }
                    results.push_str(&self.get_ty(ty));
                }
                results.push_str(") ");
            }
        }
        results
    }
    fn get_package_name_with(&self, key: &WorldKey) -> String {
        let mut name = String::new();
        match key {
            WorldKey::Name(k) => name.push_str(&k.to_upper_camel_case()),
            WorldKey::Interface(id) => {
                if !self.in_import {
                    // name.push_str("Exports");
                }
                let iface = &self.resolve.interfaces[*id];
                let pkg = &self.resolve.packages[iface.package.unwrap()];
                name.push_str(&format!(
                    "{}:{}/{}#",
                    &pkg.name.namespace,
                    &pkg.name.name,
                    &iface.name.as_ref().unwrap()
                ));
            }
        }
        name
    }
    fn get_package_name(&self) -> String {
        // dbg!()
        match self.name {
            Some(key) => self.get_package_name_with(key),
            None => self.gen.world.to_upper_camel_case(),
        }
    }
    fn print_func_signature(&mut self, resolve: &Resolve, func: &Function) {
        self.src.push_str("export fn ");
        let func_prefix = self.get_package_name();
        let params = self.get_func_params(resolve, func);
        let results = self.get_func_results(resolve, func);
        self.src
            .push_str(&format!("{func_prefix}{}({params}){results}", func.name));

        // let func_sig = self.get_func_signature_no_interface(resolve, func);
        // dbg!(&func_sig);
        // self.src.push_str(&func_sig);
        self.src.push_str("{\n");
    }

    // fn import(&mut self, resolve: &Resolve, func: &Function) {
    //     let mut func_bindgen = FunctionBindgen::new(self, func);
    //     // lower params to c
    //     func.params.iter().for_each(|(name, ty)| {
    //         // dbg!
    //         func_bindgen.lift(&avoid_keyword(&name.to_snake_case()), ty);
    //     });
    //     // lift results from c
    //     match func.results.len() {
    //         0 => {}
    //         1 => {
    //             // let ty = func.results.iter_types().next().unwrap();
    //             // func_bindgen.lift("ret", ty);
    //         }
    //         _ => {
    //             for (i, ty) in func.results.iter_types().enumerate() {
    //                 func_bindgen.lift(&format!("ret{i}"), ty);
    //             }
    //         }
    //     };
    //     // let args = func_bindgen.args;
    //     let ret = func_bindgen.args;
    //     let lower_src = func_bindgen.lower_src.to_string();
    //     let lift_src = func_bindgen.lift_src.to_string();

    //     // // print function signature
    //     self.print_func_signature(resolve, func);

    //     // body
    //     // prepare args
    //     self.src.push_str(lift_src.as_str());
    //     // self.src.push_str(lower_src.as_str());

    //     // self.import_invoke(resolve, func, c_args, &lift_src, ret);

    //     // return

    //     self.src.push_str("}\n\n");
    // }

    fn export(&mut self, resolve: &Resolve, func: &Function, func_prefix: Option<String>) {
        let mut func_bindgen = FunctionBindgen::new(self, func);
        match func.results.len() {
            0 => {}
            1 => {
                func.params.iter().for_each(|(name, ty)| {
                    func_bindgen.lift(&avoid_keyword(&name.to_snake_case()), ty);
                });
                let ty = func.results.iter_types().next().unwrap();
                func_bindgen.lower("result", ty, true);
            }
            _ => {}
        }
        let args = func_bindgen.args;
        let lift_src = func_bindgen.lift_src.to_string();
        let lower_src = func_bindgen.lower_src.to_string();
        let mut interface_decl = if let Some(pre) = func_prefix.clone() {
            format!("export fn @\"{pre}{}\"(", func.name)
        } else {
            format!("export fn __export_{}(", func.name)
        };
        for arg in args.clone() {
            interface_decl.push_str(&format!("{}: {}, ", arg.0, arg.1));
        }
        interface_decl.push_str(") ");
        let mut src = String::new();
        let result = func.results.iter_types().last().unwrap();
        src.push_str(&self.get_zig_binding_ty(result));
        src.push_str("{\n");
        src.push_str(&lift_src);
        // invoke
        let invoke = format!(
            "const result = {}.{}({})",
            &self.get_interface_var_name(),
            &func.name,
            func.params
                .iter()
                .enumerate()
                .map(|(i, name)| format!(
                    "{}{}",
                    name.0,
                    if i < func.params.len() - 1 { ", " } else { "" }
                ))
                .collect::<String>()
        );
        src.push_str(&invoke);
        src.push_str(";\n");
        // prepare ret
        match func.results.len() {
            0 => {}
            1 => {
                src.push_str(&lower_src);
                // src.push_str(
                //     "const ret = alloc(8);
                // std.mem.writeIntLittle(u32, ret[0..4], @intCast(@intFromPtr(result.ptr)));
                // std.mem.writeIntLittle(u32, ret[4..8], @intCast(result.len));
                // return ret;
                // ",
                // );
            }
            _ => {}
        }
        src.push_str("\n");
        self.src.push_str(&interface_decl);
        self.src.push_str(&src);
        if abi::guest_export_needs_post_return(resolve, func) {
            if let Some(pre) = func_prefix {
                self.src.push_str(&format!(
                    "export fn @\"__post_return_{pre}{}\"(arg: u32) void {{
                  var buffer: [8]u8 = .{{0}} ** 8;
                  std.mem.writeIntNative(u32, buffer[0..][0..@sizeOf(u32)], arg);
                  const stringPtr = buffer[0..4];
                  const stringSize = buffer[4..8];
                  const bytesPtr = std.mem.readIntLittle(u32, @ptrCast(stringPtr));
                  const ptr_size = std.mem.readIntLittle(u32, @ptrCast(stringSize));
                  const casted: [*]u8 = @ptrFromInt(bytesPtr);
                  allocator.free(casted[0..ptr_size]);
                }}
                
                ",
                    func.name
                ));
            } else {
                self.src.push_str(&format!(
                    "export fn __post_return_{}(arg: u32) void {{
              var buffer: [8]u8 = .{{0}} ** 8;
              std.mem.writeIntNative(u32, buffer[0..][0..@sizeOf(u32)], arg);
              const stringPtr = buffer[0..4];
              const stringSize = buffer[4..8];
              const bytesPtr = std.mem.readIntLittle(u32, @ptrCast(stringPtr));
              const ptr_size = std.mem.readIntLittle(u32, @ptrCast(stringSize));
              const casted: [*]u8 = @ptrFromInt(bytesPtr);
              allocator.free(casted[0..ptr_size]);
            }}
            
            ",
                    func.name
                ));
            }
        }
        // self.export_funcs.push(self.src);
    }

    fn get_interface_var_name(&self) -> String {
        let mut name = String::new();
        match self.name {
            Some(WorldKey::Name(k)) => name.push_str(&k.to_snake_case()),
            Some(WorldKey::Interface(id)) => {
                let iface = &self.resolve.interfaces[*id];
                let pkg = &self.resolve.packages[iface.package.unwrap()];
                // name.push_str(&pkg.name.namespace.to_snake_case());
                // name.push('_');
                // name.push_str(&pkg.name.name.to_upper_camel_case());
                // name.push('_');
                name.push_str(&iface.name.as_ref().unwrap().to_upper_camel_case());
            }
            None => {
                name.push_str("Guest");
                // name.push_str(&self.gen.world.to_snake_case())
            }
        }
        name
    }

    fn get_zig_binding_ty(&self, ty: &Type) -> String {
        match ty {
            Type::Bool => "bool".into(),
            Type::U8 => "u8".into(),
            Type::U16 => "u16".into(),
            Type::U32 => "u32".into(),
            Type::U64 => "u64".into(),
            Type::S8 => "s8".into(),
            Type::S16 => "s16".into(),
            Type::S32 => "s32".into(),
            Type::S64 => "s64".into(),
            Type::Float32 => todo!(),
            Type::Float64 => todo!(),
            Type::Char => todo!(),
            Type::String => "[*]u8".into(),
            Type::Id(_) => todo!(),
        }
    }

    fn finish(&mut self) {
        for (name, export_func) in &self.export_funcs {
            self.src.push_str(export_func);
        }
    }
}

struct FunctionBindgen<'a, 'b> {
    interface: &'a mut InterfaceGenerator<'b>,
    _func: &'a Function,
    args: Vec<(String, String)>,
    lower_src: Source,
    lift_src: Source,
}

impl<'a, 'b> FunctionBindgen<'a, 'b> {
    fn new(interface: &'a mut InterfaceGenerator<'b>, func: &'a Function) -> Self {
        Self {
            interface,
            _func: func,
            args: Vec::new(),
            lower_src: Source::default(),
            lift_src: Source::default(),
        }
    }

    fn lower(&mut self, name: &str, ty: &Type, in_export: bool) {
        let lower_name = format!("lower_{name}");
        self.lower_value(name, ty, lower_name.as_ref());
    }

    fn lower_value(&mut self, param: &str, ty: &Type, lower_name: &str) {
        match ty {
            Type::Bool
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::S8
            | Type::S16
            | Type::S32
            | Type::S64
            | Type::Float32
            | Type::Float64
            | Type::Char => self.lower_src.push_str("return result;\n}\n"),
            Type::String => self.lower_src.push_str(
                "const ret = alloc(8);
              std.mem.writeIntLittle(u32, ret[0..4], @intCast(@intFromPtr(result.ptr)));
              std.mem.writeIntLittle(u32, ret[4..8], @intCast(result.len));
              return ret;
            }
              ",
            ),
            Type::Id(_) => todo!(),
        }
    }
    fn lift(&mut self, name: &str, ty: &Type) {
        self.lift_value(name, ty);
    }

    fn lift_value(&mut self, param: &str, ty: &Type) {
        match ty {
            Type::Bool => {
                self.args.push((param.to_string(), "bool".to_string()));
            }
            Type::U8 => {
                self.args.push((param.to_string(), "u8".to_string()));
            }
            Type::U16 => {
                self.args.push((param.to_string(), "u16".to_string()));
            }
            Type::U32 => {
                self.args.push((param.to_string(), "u32".to_string()));
            }
            Type::U64 => {
                self.args.push((param.to_string(), "u64".to_string()));
            }
            Type::S8 => {
                self.args.push((param.to_string(), "s8".to_string()));
            }
            Type::S16 => {
                self.args.push((param.to_string(), "s16".to_string()));
            }
            Type::S32 => {
                self.args.push((param.to_string(), "s32".to_string()));
            }
            Type::S64 => {
                self.args.push((param.to_string(), "s64".to_string()));
            }
            Type::Float32 => todo!(),
            Type::Float64 => todo!(),
            Type::Char => todo!(),
            Type::String => {
                self.lift_src
                    .push_str(&format!("const {param} = {param}Ptr[0..{param}Length];\n"));
                self.args.push((format!("{param}Ptr"), "[*]u8".to_string()));
                self.args
                    .push((format!("{param}Length"), "u32".to_string()));
            }
            Type::Id(_) => todo!(),
        }
    }
}
