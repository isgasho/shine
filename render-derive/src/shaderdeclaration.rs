use syn;
use quote;
use std::env;
use std::path::Path;
use std::fs::File;
use std::io::Read;

//use utils::*;

#[derive(Debug)]
enum SourceKind {
    Src(String),
    Path(String),
}

pub fn impl_shader_declaration(ast: &syn::DeriveInput) -> quote::Tokens {
    let name = &ast.ident;

    let sources = ast.attrs.iter()
        .filter_map(|attr| {
            match attr.value {
                syn::MetaItem::NameValue(ref i, syn::Lit::Str(ref val, _)) if i == "vert_src" => {
                    Some(("VertexShader".to_string(), SourceKind::Src(val.clone())))
                }

                syn::MetaItem::NameValue(ref i, syn::Lit::Str(ref val, _)) if i == "vert_path" => {
                    Some(("VertexShader".to_string(), SourceKind::Path(val.clone())))
                }

                syn::MetaItem::NameValue(ref i, syn::Lit::Str(ref val, _)) if i == "frag_src" => {
                    Some(("FragmentShader".to_string(), SourceKind::Src(val.clone())))
                }

                syn::MetaItem::NameValue(ref i, syn::Lit::Str(ref val, _)) if i == "frag_path" => {
                    Some(("FragmentShader".to_string(), SourceKind::Path(val.clone())))
                }

                _ => None
            }
        })
        .filter_map(|(sh_type, source)| {
            let sh_type = syn::Ident::new(sh_type);
            match source {
                SourceKind::Src(source) => Some(quote! { (::dragorust_engine::render::ShaderType::#sh_type, #source) }),

                SourceKind::Path(path) => {
                    let root = env::var("CARGO_MANIFEST_DIR").unwrap_or(".".into());
                    let full_path = Path::new(&root).join(&path);

                    if full_path.is_file() {
                        let mut buf = String::new();
                        File::open(full_path)
                            .and_then(|mut file| file.read_to_string(&mut buf))
                            .expect(&format!("Error reading source from {:?}", path));
                        Some(quote! { (::dragorust_engine::render::ShaderType::#sh_type, #buf) })
                    } else {
                        panic!("File {:?} was not found. Path must be relative to your Cargo.toml", path);
                    }
                }
            }
        });

    let sources = sources.collect::<Vec<_>>();
    let gen = quote! {
        impl ShaderDeclaration for #name {
            type Attribute = ShSimpleAttribute;
            type Uniform = ShSimpleUniform;

            fn map_sources<F: FnMut((ShaderType, &str)) -> bool>(mut f: F) -> bool {
                let sh_source = #sources;

                for src in sh_source.iter() {
                    if !f(*src) {
                        return false
                    }
                }
                true
            }
        }
    };

    println!("!!!!: {:?}", gen.to_string());

    gen
}