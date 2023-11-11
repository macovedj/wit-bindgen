use heck::{ToKebabCase, ToSnakeCase};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::str::FromStr;
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
    import_modules: Vec<(String, Vec<String>)>,
    export_modules: Vec<(String, Vec<String>)>,
    skip: HashSet<String>,
    interface_names: HashMap<InterfaceId, InterfaceName>,
    resources: HashMap<TypeId, ResourceInfo>,
    import_funcs_called: bool,
    with_name_counter: usize,
}

impl ZigWasm {
    fn new() -> ZigWasm {
        ZigWasm::default()
    }

    fn interface<'a>(
        &'a mut self,
        identifier: Identifier<'a>,
        wasm_import_module: Option<&'a str>,
        resolve: &'a Resolve,
        in_import: bool,
    ) -> InterfaceGenerator<'a> {
        InterfaceGenerator {
            src: Source::default(),
            gen: self,
            resolve,
            interface: None,
            name: identifier,
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
        todo!()
    }

    fn export_interface(
        &mut self,
        resolve: &Resolve,
        name: &WorldKey,
        iface: InterfaceId,
        files: &mut wit_bindgen_core::Files,
    ) -> anyhow::Result<()> {
        todo!()
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
        dbg!("export funcs");
        self.src.push_str(
            "const Guest = struct {
          ",
        );
        let mut gen = self.interface(Identifier::World(world), None, resolve, false);
        for (name, func) in funcs.iter() {
            gen.export(resolve, func);
        }
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
        }",
        );
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
    name: Identifier<'a>,
    public_anonymous_types: BTreeSet<TypeId>,
    in_import: bool,
    export_funcs: Vec<(String, String)>,
}

impl InterfaceGenerator<'_> {
    fn export(&mut self, resolve: &Resolve, func: &Function) {
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
        let interface_decl = &func.name;
    }
}

struct FunctionBindgen<'a, 'b> {
    interface: &'a mut InterfaceGenerator<'b>,
    _func: &'a Function,
    c_args: Vec<String>,
    args: Vec<String>,
    lower_src: Source,
    lift_src: Source,
}

impl<'a, 'b> FunctionBindgen<'a, 'b> {
    fn new(interface: &'a mut InterfaceGenerator<'b>, func: &'a Function) -> Self {
        Self {
            interface,
            _func: func,
            c_args: Vec::new(),
            args: Vec::new(),
            lower_src: Source::default(),
            lift_src: Source::default(),
        }
    }

    fn lower(&mut self, name: &str, ty: &Type, in_export: bool) {}
    fn lift(&mut self, name: &str, ty: &Type) {
        dbg!(&name);
        self.lift_value(name, ty);
        self.args.push(name.to_string());
    }

    fn lift_value(&mut self, param: &str, ty: &Type) {
        match ty {
            Type::Bool => todo!(),
            Type::U8 => {
                dbg!("ITS A u8");
            }
            Type::U16 => todo!(),
            Type::U32 => todo!(),
            Type::U64 => todo!(),
            Type::S8 => todo!(),
            Type::S16 => todo!(),
            Type::S32 => todo!(),
            Type::S64 => todo!(),
            Type::Float32 => todo!(),
            Type::Float64 => todo!(),
            Type::Char => todo!(),
            Type::String => {
                self.lift_src
                    .push_str(&format!("const {param} = {param}Ptr[0..{param}Length;"));
            }
            Type::Id(_) => todo!(),
        }
    }
}
