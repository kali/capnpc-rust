// Copyright (c) 2013-2015 Sandstorm Development Group, Inc. and contributors
// Licensed under the MIT License:
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

use capnp;
use std::collections;
use schema_capnp;
use codegen_types::{ Module, RustTypeInfo, RustNodeInfo };
use self::FormattedText::{Indent, Line, Branch, BlankLine};

pub struct GeneratorContext<'a> {
    pub request : schema_capnp::code_generator_request::Reader<'a>,
    pub node_map : collections::hash_map::HashMap<u64, schema_capnp::node::Reader<'a>>,
    pub scope_map : collections::hash_map::HashMap<u64, Vec<String>>,
}

impl <'a> GeneratorContext<'a> {

    pub fn new(message:&'a capnp::message::Reader<capnp::serialize::OwnedSegments> )
            -> ::capnp::Result<GeneratorContext<'a>> {

        let mut gen = GeneratorContext {
            request : try!(message.get_root()),
            node_map: collections::hash_map::HashMap::<u64, schema_capnp::node::Reader<'a>>::new(),
            scope_map: collections::hash_map::HashMap::<u64, Vec<String>>::new(),
        };

        for node in try!(gen.request.get_nodes()).iter() {
            gen.node_map.insert(node.get_id(), node);
        }

        for requested_file in try!(gen.request.get_requested_files()).iter() {
             let id = requested_file.get_id();

            let imports = try!(requested_file.get_imports());
            for import in imports.iter() {
                let importpath = ::std::path::Path::new(try!(import.get_name()));
                let root_name : String = format!("::{}_capnp",
                                                 importpath.file_stem().unwrap().to_owned()
                                                .into_string().unwrap().replace("-", "_"));
                populate_scope_map(&gen.node_map, &mut gen.scope_map, vec!(root_name), import.get_id());
            }

            let root_name = ::std::path::PathBuf::from(try!(requested_file.get_filename()))
                .file_stem().unwrap().to_owned().into_string().unwrap();
            let root_mod = format!("::{}_capnp", root_name.to_owned().replace("-", "_"));
            populate_scope_map(&gen.node_map, &mut gen.scope_map, vec!(root_mod), id);
        }
        Ok(gen)
    }

}

fn tuple_result<T,U,V>(t : Result<T, V>, u : Result<U, V>) -> Result<(T,U), V> {
    match (t, u) {
        (Ok(t1), Ok(u1)) => Ok((t1, u1)),
        (Err(e), _) => Err(e),
        (_, Err(e)) => Err(e),
    }
}

pub fn camel_to_upper_case(s : &str) -> String {
    use std::ascii::*;
    let mut result_chars : Vec<char> = Vec::new();
    for c in s.chars() {
        assert!(c.is_alphanumeric(), format!("not alphanumeric '{}'", c));
        if c.is_uppercase() {
            result_chars.push('_');
        }
        result_chars.push((c as u8).to_ascii_uppercase() as char);
    }
    return result_chars.into_iter().collect();
}

fn snake_to_upper_case(s : &str) -> String {
    use std::ascii::*;
    let mut result_chars : Vec<char> = Vec::new();
    for c in s.chars() {
        if c == '_' {
            result_chars.push('_');
        } else {
            assert!(c.is_alphanumeric(), format!("not alphanumeric '{}'", c));
            result_chars.push((c as u8).to_ascii_uppercase() as char);
        }
    }
    return result_chars.into_iter().collect();
}

fn camel_to_snake_case(s : &str) -> String {
    use std::ascii::*;
    let mut result_chars : Vec<char> = Vec::new();
    let mut first_char = true;
    for c in s.chars() {
        assert!(c.is_alphanumeric(),
                format!("not alphanumeric '{}', i.e. {}", c, c as usize));
        if c.is_uppercase() && !first_char {
            result_chars.push('_');
        }
        result_chars.push((c as u8).to_ascii_lowercase() as char);
        first_char = false;
    }
    return result_chars.into_iter().collect();
}

fn capitalize_first_letter(s : &str) -> String {
    use std::ascii::*;
    let mut result_chars : Vec<char> = Vec::new();
    for c in s.chars() { result_chars.push(c) }
    result_chars[0] = (result_chars[0] as u8).to_ascii_uppercase() as char;
    return result_chars.into_iter().collect();
}

#[test]
fn test_camel_to_upper_case() {
    assert_eq!(camel_to_upper_case("fooBar"), "FOO_BAR".to_string());
    assert_eq!(camel_to_upper_case("fooBarBaz"), "FOO_BAR_BAZ".to_string());
    assert_eq!(camel_to_upper_case("helloWorld"), "HELLO_WORLD".to_string());
}

#[test]
fn test_camel_to_snake_case() {
    assert_eq!(camel_to_snake_case("fooBar"), "foo_bar".to_string());
    assert_eq!(camel_to_snake_case("FooBar"), "foo_bar".to_string());
    assert_eq!(camel_to_snake_case("fooBarBaz"), "foo_bar_baz".to_string());
    assert_eq!(camel_to_snake_case("FooBarBaz"), "foo_bar_baz".to_string());
    assert_eq!(camel_to_snake_case("helloWorld"), "hello_world".to_string());
    assert_eq!(camel_to_snake_case("HelloWorld"), "hello_world".to_string());
    assert_eq!(camel_to_snake_case("uint32Id"), "uint32_id".to_string());
}

#[derive(PartialEq)]
pub enum FormattedText {
    Indent(Box<FormattedText>),
    Branch(Vec<FormattedText>),
    Line(String),
    BlankLine
}

fn to_lines(ft : &FormattedText, indent : usize) -> Vec<String> {
    match *ft {
        Indent (ref ft) => {
            return to_lines(&**ft, indent + 1);
        }
        Branch (ref fts) => {
            let mut result = Vec::new();
            for ft in fts.iter() {
                for line in to_lines(ft, indent).iter() {
                    result.push(line.clone());  // TODO there's probably a better way to do this.
                }
            }
            return result;
        }
        Line(ref s) => {
            let mut s1 : String = ::std::iter::repeat(' ').take(indent * 2).collect();
            s1.push_str(&s);
            return vec!(s1.to_string());
        }
        BlankLine => return vec!("".to_string())
    }
}

fn stringify(ft : & FormattedText) -> String {
    let mut result = to_lines(ft, 0).connect("\n");
    result.push_str("\n");
    return result.to_string();
}

const RUST_KEYWORDS : [&'static str; 51] =
    ["abstract", "alignof", "as", "be", "box",
     "break", "const", "continue", "crate", "do",
     "else", "enum", "extern", "false", "final",
     "fn", "for", "if", "impl", "in",
     "let", "loop", "match", "mod", "move",
     "mut", "offsetof", "once", "override", "priv",
     "proc", "pub", "pure", "ref", "return",
     "sizeof", "static", "self", "struct", "super",
     "true", "trait", "type", "typeof", "unsafe",
     "unsized", "use", "virtual", "where", "while",
     "yield"];

fn module_name(camel_case : &str) -> String {
    let mut name = camel_to_snake_case(camel_case);
    if RUST_KEYWORDS.contains(&&*name) {
        name.push('_');
    }
    return name;
}

fn populate_scope_map(node_map : &collections::hash_map::HashMap<u64, schema_capnp::node::Reader>,
                      scope_map : &mut collections::hash_map::HashMap<u64, Vec<String>>,
                      scope_names : Vec<String>,
                      node_id : u64) {

    scope_map.insert(node_id, scope_names.clone());

    // unused nodes in imported files might be omitted from the node map
    let node_reader = match node_map.get(&node_id) { Some(node) => node, None => return (), };

    let nested_nodes = node_reader.get_nested_nodes().unwrap();
    for nested_node in nested_nodes.iter(){
        let mut scope_names = scope_names.clone();
        let nested_node_id = nested_node.get_id();
        match node_map.get(&nested_node_id) {
            None => {}
            Some(node_reader) => {
                match node_reader.which() {
                    Ok(schema_capnp::node::Enum(_enum_reader)) => {
                        scope_names.push(nested_node.get_name().unwrap().to_string());
                        populate_scope_map(node_map, scope_map, scope_names, nested_node_id);
                    }
                    _ => {
                        scope_names.push(module_name(nested_node.get_name().unwrap()));
                        populate_scope_map(node_map, scope_map, scope_names, nested_node_id);

                    }
                }
            }
        }
    }

    match node_reader.which() {
        Ok(schema_capnp::node::Struct(struct_reader)) => {
            let fields = struct_reader.get_fields().unwrap();
            for field in fields.iter() {
                match field.which() {
                    Ok(schema_capnp::field::Group(group)) => {
                        let name = module_name(field.get_name().unwrap());
                        let mut scope_names = scope_names.clone();
                        scope_names.push(name);
                        populate_scope_map(node_map, scope_map, scope_names, group.get_type_id());
                    }
                    _ => {}
                }
            }
        }
        _ => {  }
    }
}

fn generate_import_statements() -> FormattedText {
    Branch(vec!(
        Line("#![allow(unused_imports)]".to_string()),
        Line("use capnp::capability::{FromClientHook, FromTypelessPipeline};".to_string()),
        Line("use capnp::{text, data, Result};".to_string()),
        Line("use capnp::private::layout;".to_string()),
        Line("use capnp::traits::{FromStructBuilder, FromStructReader};".to_string()),
        Line("use capnp::{primitive_list, enum_list, struct_list, text_list, data_list, list_list};".to_string()),
    ))
}

fn generate_import_statements_for_generics() -> FormattedText {
    Branch(vec!(
        Line("use capnp::traits::{ FromPointerReader, FromPointerBuilder, SetPointerBuilder };".to_string()),
        Line("use std::marker::PhantomData;".to_string()),
    ))
}

fn prim_default (value : &schema_capnp::value::Reader) -> Option<String> {
    use schema_capnp::value;
    match value.which().unwrap() {
        value::Bool(false) |
        value::Int8(0) | value::Int16(0) | value::Int32(0) |
        value::Int64(0) | value::Uint8(0) | value::Uint16(0) |
        value::Uint32(0) | value::Uint64(0) | value::Float32(0.0) |
        value::Float64(0.0) => None,

        value::Bool(true) => Some(format!("true")),
        value::Int8(i) => Some(i.to_string()),
        value::Int16(i) => Some(i.to_string()),
        value::Int32(i) => Some(i.to_string()),
        value::Int64(i) => Some(i.to_string()),
        value::Uint8(i) => Some(i.to_string()),
        value::Uint16(i) => Some(i.to_string()),
        value::Uint32(i) => Some(i.to_string()),
        value::Uint64(i) => Some(i.to_string()),
        value::Float32(f) =>
            Some(format!("{}u32", unsafe {::std::mem::transmute::<f32, u32>(f)}.to_string())),
        value::Float64(f) =>
            Some(format!("{}u64", unsafe {::std::mem::transmute::<f64, u64>(f)}.to_string())),
        _ => {panic!()}
    }
}

pub fn getter_text (gen:&GeneratorContext,
               field : &schema_capnp::field::Reader,
               is_reader : bool)
    -> (String, FormattedText) {
    use schema_capnp::*;

    match field.which().ok().expect("unrecognized field type") {
        field::Group(group) => {
            let the_mod = gen.scope_map[&group.get_type_id()].connect("::");
            if is_reader {
                (format!("{}::Reader<'a>", the_mod),
                        Line("::capnp::traits::FromStructReader::new(self.reader)".to_string()))
            } else {
                (format!("{}::Builder<'a>", the_mod),
                        Line("::capnp::traits::FromStructBuilder::new(self.builder)".to_string()))
            }
        }
        field::Slot(reg_field) => {
            let offset = reg_field.get_offset() as usize;
            let module = if is_reader { Module::Reader } else { Module::Builder };
            let member = camel_to_snake_case(&*format!("{}", module));

            fn primitive_case<T: PartialEq + ::std::fmt::Display>(typ: &str, member:String,
                    offset: usize, default : T, zero : T) -> FormattedText {
                if default == zero {
                    Line(format!("self.{}.get_data_field::<{}>({})", member, typ, offset))
                } else {
                    Line(format!("self.{}.get_data_field_mask::<{typ}>({}, {})", member, offset, default, typ=typ))
                }
            }

            let raw_type = reg_field.get_type().unwrap();
            let typ = raw_type.type_string(gen, module, "'a");
            let default = reg_field.get_default_value().unwrap().which().unwrap();

            let result_type = match raw_type.which().unwrap() {
                type_::Enum(_) => format!("::std::result::Result<{},::capnp::NotInSchema>", typ),
                type_::AnyPointer(_) if !raw_type.is_parameterized() => typ.clone(),
                _ if raw_type.is_prim() => typ.clone(),
                _ => format!("Result<{}>", typ),
            };

            let getter_code = match (raw_type.which().unwrap(), default) {
                (type_::Void(()), value::Void(())) => Line("()".to_string()),
                (type_::Bool(()), value::Bool(b)) => {
                    if b {
                        Line(format!("self.{}.get_bool_field_mask({}, true)", member, offset))
                    } else {
                        Line(format!("self.{}.get_bool_field({})", member, offset))
                    }
                }
                (type_::Int8(()), value::Int8(i)) => primitive_case(&*typ, member, offset, i, 0),
                (type_::Int16(()), value::Int16(i)) => primitive_case(&*typ, member, offset, i, 0),
                (type_::Int32(()), value::Int32(i)) => primitive_case(&*typ, member, offset, i, 0),
                (type_::Int64(()), value::Int64(i)) => primitive_case(&*typ, member, offset, i, 0),
                (type_::Uint8(()), value::Uint8(i)) => primitive_case(&*typ, member, offset, i, 0),
                (type_::Uint16(()), value::Uint16(i)) => primitive_case(&*typ, member, offset, i, 0),
                (type_::Uint32(()), value::Uint32(i)) => primitive_case(&*typ, member, offset, i, 0),
                (type_::Uint64(()), value::Uint64(i)) => primitive_case(&*typ, member, offset, i, 0),
                (type_::Float32(()), value::Float32(f)) =>
                    primitive_case(&*typ, member, offset, unsafe { ::std::mem::transmute::<f32, u32>(f) }, 0),
                (type_::Float64(()), value::Float64(f)) =>
                    primitive_case(&*typ, member, offset, (unsafe { ::std::mem::transmute::<f64, u64>(f) }), 0),
                (type_::Text(()), _) => {
                    Line(format!("self.{}.get_pointer_field({}).get_text(::std::ptr::null(), 0)", member, offset))
                }
                (type_::Data(()), _) => {
                    Line(format!("self.{}.get_pointer_field({}).get_data(::std::ptr::null(), 0)", member, offset))
                }
                (type_::List(_), _) => {
                    if is_reader {
                        Line(format!(
                            "::capnp::traits::FromPointerReader::get_from_pointer(&self.{}.get_pointer_field({}))",
                            member, offset))
                    } else {
                        Line(format!("::capnp::traits::FromPointerBuilder::get_from_pointer(self.{}.get_pointer_field({}))",
                                     member, offset))

                    }
                }
                (type_::Enum(_), _) => {
                    Branch(vec!(
                       Line(format!("::capnp::traits::FromU16::from_u16(self.{}.get_data_field::<u16>({}))",
                                   member, offset))))
                }
                (type_::Struct(_), _) => {
                    if is_reader {
                        Line(format!("::capnp::traits::FromPointerReader::get_from_pointer(&self.{}.get_pointer_field({}))",
                                     member, offset))
                    } else {
                        Line(format!("::capnp::traits::FromPointerBuilder::get_from_pointer(self.{}.get_pointer_field({}))",
                                     member, offset))
                    }
                }
                (type_::Interface(_), _) => {
                    Line(format!("match self.{}.get_pointer_field({}).get_capability() {{ ::std::result::Result::Ok(c) => ::std::result::Result::Ok(FromClientHook::new(c)), ::std::result::Result::Err(e) => ::std::result::Result::Err(e)}}",
                                 member, offset))
                }
                (type_::AnyPointer(_), _) => {
                    if !raw_type.is_parameterized() {
                        Line(format!("::capnp::any_pointer::{}::new(self.{}.get_pointer_field({}))", module, member, offset))
                    } else {
                        if is_reader {
                            Line(format!("{}::get_from_pointer(&self.{}.get_pointer_field({}))", typ, member, offset))
                        } else {
                            Line(format!("{}::get_from_pointer(self.{}.get_pointer_field({}))", typ, member, offset))
                        }
                    }
                }
                _ => {
                    panic!("default value was of wrong type");
                }
            };
            (result_type, getter_code)
        }
    }

}

fn zero_fields_of_group(gen:&GeneratorContext, node_id : u64) -> FormattedText {
    use schema_capnp::{node, field, type_};
    match gen.node_map[&node_id].which() {
        Ok(node::Struct(st)) => {
            let mut result = Vec::new();
            if st.get_discriminant_count() != 0 {
                result.push(
                    Line(format!("self.builder.set_data_field::<u16>({}, 0);",
                                 st.get_discriminant_offset())));
            }
            let fields = st.get_fields().unwrap();
            for field in fields.iter() {
                match field.which().unwrap() {
                    field::Group(group) => {
                        result.push(zero_fields_of_group(gen, group.get_type_id()));
                    }
                    field::Slot(slot) => {
                        let typ = slot.get_type().unwrap().which().unwrap();
                        match typ {
                            type_::Void(()) => {}
                            type_::Bool(()) => {
                                let line = Line(format!("self.builder.set_bool_field({}, false);",
                                                        slot.get_offset()));
                                // PERF could dedup more efficiently
                                if !result.contains(&line) { result.push(line) }
                            }
                            type_::Int8(()) |
                            type_::Int16(()) | type_::Int32(()) | type_::Int64(()) |
                            type_::Uint8(()) | type_::Uint16(()) | type_::Uint32(()) |
                            type_::Uint64(()) | type_::Float32(()) | type_::Float64(()) |
                            type_::Enum(_) => {
                                let line = Line(format!("self.builder.set_data_field::<{0}>({1}, 0u8 as {0});",
                                                        slot.get_type().unwrap().type_string(gen, Module::Builder, "'a"),
                                                        slot.get_offset()));
                                // PERF could dedup more efficiently
                                if !result.contains(&line) { result.push(line) }
                            }
                            type_::Struct(_) | type_::List(_) | type_::Text(()) | type_::Data(()) |
                            type_::AnyPointer(_) |
                            type_::Interface(_) // Is this the right thing to do for interfaces?
                                => {
                                    let line = Line(format!("self.builder.get_pointer_field({}).clear();",
                                                            slot.get_offset()));
                                    // PERF could dedup more efficiently
                                    if !result.contains(&line) { result.push(line) }
                                }
                        }
                    }
                }
            }
            return Branch(result);
        }
        _ => { panic!("expected a struct") }
    }
}

fn generate_setter(gen:&GeneratorContext, discriminant_offset : u32,
                  styled_name : &str,
                  field :&schema_capnp::field::Reader) -> FormattedText {

    use schema_capnp::*;

    let mut setter_interior = Vec::new();
    let mut setter_param = "value".to_string();
    let mut initter_interior = Vec::new();
    let mut initter_params = Vec::new();

    let discriminant_value = field.get_discriminant_value();
    if discriminant_value != field::NO_DISCRIMINANT {
        setter_interior.push(
            Line(format!("self.builder.set_data_field::<u16>({}, {});",
                         discriminant_offset as usize,
                         discriminant_value as usize)));
        initter_interior.push(
            Line(format!("self.builder.set_data_field::<u16>({}, {});",
                         discriminant_offset as usize,
                         discriminant_value as usize)));
    }

    let mut setter_generic_param = String::new();
    let mut return_result = false;

    let (maybe_reader_type, maybe_builder_type) : (Option<String>, Option<String>) = match field.which() {
        Err(_) => panic!("unrecognized field type"),
        Ok(field::Group(group)) => {
            let scope = &gen.scope_map[&group.get_type_id()];
            let the_mod = scope.connect("::");

            initter_interior.push(zero_fields_of_group(gen, group.get_type_id()));

            initter_interior.push(Line(format!("::capnp::traits::FromStructBuilder::new(self.builder)")));

            (None, Some(format!("{}::Builder<'a>", the_mod)))
        }
        Ok(field::Slot(reg_field)) => {
            let offset = reg_field.get_offset() as usize;
            let typ = reg_field.get_type().unwrap();
            match typ.which().ok().expect("unrecognized type") {
                type_::Void(()) => {
                    setter_param = "_value".to_string();
                    (Some("()".to_string()), None)
                }
                type_::Bool(()) => {
                    match prim_default(&reg_field.get_default_value().unwrap()) {
                        None => {
                            setter_interior.push(Line(format!("self.builder.set_bool_field({}, value);", offset)));
                        }
                        Some(s) => {
                            setter_interior.push(
                                Line(format!("self.builder.set_bool_field_mask({}, value, {});", offset, s)));
                        }
                    }
                    (Some("bool".to_string()), None)
                }
                _ if typ.is_prim() => {
                    let tstr = typ.type_string(gen, Module::Reader, "'a");
                    match prim_default(&reg_field.get_default_value().unwrap()) {
                        None => {
                            setter_interior.push(Line(format!("self.builder.set_data_field::<{}>({}, value);",
                                                              tstr, offset)));
                        }
                        Some(s) => {
                            setter_interior.push(
                                Line(format!("self.builder.set_data_field_mask::<{}>({}, value, {});",
                                             tstr, offset, s)));
                        }
                    };
                    (Some(tstr), None)
                }
                type_::Text(()) => {
                    setter_interior.push(Line(format!("self.builder.get_pointer_field({}).set_text(value);",
                                                      offset)));
                    initter_interior.push(Line(format!("self.builder.get_pointer_field({}).init_text(size)",
                                                       offset)));
                    initter_params.push("size : u32");
                    (Some("text::Reader".to_string()), Some("text::Builder<'a>".to_string()))
                }
                type_::Data(()) => {
                    setter_interior.push(Line(format!("self.builder.get_pointer_field({}).set_data(value);",
                                                      offset)));
                    initter_interior.push(Line(format!("self.builder.get_pointer_field({}).init_data(size)",
                                                       offset)));
                    initter_params.push("size : u32");
                    (Some("data::Reader".to_string()), Some("data::Builder<'a>".to_string()))
                }
                type_::List(ot1) => {
                    return_result = true;
                    setter_interior.push(
                        Line(format!("::capnp::traits::SetPointerBuilder::set_pointer_builder(self.builder.get_pointer_field({}), value)",
                                     offset)));

                    initter_params.push("size : u32");
                    initter_interior.push(
                        Line(format!("::capnp::traits::FromPointerBuilder::init_pointer(self.builder.get_pointer_field({}), size)", offset)));

                    match ot1.get_element_type().unwrap().which().unwrap() {
                        type_::List(_) => {
                            setter_generic_param = "<'b>".to_string();
                            (Some(reg_field.get_type().unwrap().type_string(gen, Module::Reader, "'b")),
                             Some(reg_field.get_type().unwrap().type_string(gen, Module::Builder, "'a")))
                        }
                        _ =>
                            (Some(reg_field.get_type().unwrap().type_string(gen, Module::Reader, "'a")),
                             Some(reg_field.get_type().unwrap().type_string(gen, Module::Builder, "'a")))
                    }
                }
                type_::Enum(e) => {
                    let id = e.get_type_id();
                    let the_mod = gen.scope_map[&id].connect("::");
                    setter_interior.push(
                        Line(format!("self.builder.set_data_field::<u16>({}, value as u16)",
                                     offset)));
                    (Some(format!("{}", the_mod)), None)
                }
                type_::Struct(_) => {
                    return_result = true;
                    setter_generic_param = "<'b>".to_string();
                    initter_interior.push(
                      Line(format!("::capnp::traits::FromPointerBuilder::init_pointer(self.builder.get_pointer_field({}), 0)",
                                   offset)));
                    if typ.is_branded() {
                        setter_interior.push(
                            Line(format!("<{} as ::capnp::traits::SetPointerBuilder<{}>>::set_pointer_builder(self.builder.get_pointer_field({}), value)", typ.type_string(gen, Module::Reader, "'b"), typ.type_string(gen, Module::Builder, "'b"), offset)));
                        (Some(typ.type_string(gen, Module::Reader, "'b")),
                         Some(typ.type_string(gen, Module::Builder, "'a")))
                    } else {
                        setter_interior.push(
                            Line(format!("::capnp::traits::SetPointerBuilder::set_pointer_builder(self.builder.get_pointer_field({}), value)", offset)));
                        (Some(reg_field.get_type().unwrap().type_string(gen, Module::Reader, "'b")),
                         Some(reg_field.get_type().unwrap().type_string(gen, Module::Builder, "'a")))
                    }
                }
                type_::Interface(_) => {
                    setter_interior.push(
                        Line(format!("self.builder.get_pointer_field({}).set_capability(value.client.hook);",
                                     offset)));
                    (Some(typ.type_string(gen, Module::Client, "")), None)
                }
                type_::AnyPointer(_) => {
                    if typ.is_parameterized() {
                        initter_interior.push(Line(format!("::capnp::any_pointer::Builder::new(self.builder.get_pointer_field({})).init_as()", offset)));
                        setter_generic_param = format!("<SPB: SetPointerBuilder<{}>>", typ.type_string(gen, Module::Builder, "'a"));
                        setter_interior.push(Line(format!("SetPointerBuilder::set_pointer_builder(self.builder.get_pointer_field({}), value)", offset)));
                        return_result = true;
                        (Some("SPB".to_string()), Some(typ.type_string(gen, Module::Builder, "'a")))
                    } else {
                        initter_interior.push(Line(format!("let mut result = ::capnp::any_pointer::Builder::new(self.builder.get_pointer_field({}));",
                                                   offset)));
                        initter_interior.push(Line("result.clear();".to_string()));
                        initter_interior.push(Line("result".to_string()));
                        (None, Some("::capnp::any_pointer::Builder<'a>".to_string()))
                    }
                }
                _ => panic!("unrecognized type")
            }
        }
    };
    let mut result = Vec::new();
    match maybe_reader_type {
        Some(ref reader_type) => {
            let return_type = if return_result { "-> Result<()>" } else { "" };
            result.push(Line("#[inline]".to_string()));
            result.push(Line("#[allow(dead_code)]".to_string()));
            result.push(Line(format!("pub fn set_{}{}(&mut self, {} : {}) {} {{",
                                     styled_name, setter_generic_param, setter_param,
                                     reader_type, return_type)));
            result.push(Indent(Box::new(Branch(setter_interior))));
            result.push(Line("}".to_string()));
        }
        None => {}
    }
    match maybe_builder_type {
        Some(builder_type) => {
            result.push(Line("#[inline]".to_string()));
            result.push(Line("#[allow(dead_code)]".to_string()));
            let args = initter_params.connect(", ");
            result.push(Line(format!("pub fn init_{}(self, {}) -> {} {{",
                                     styled_name, args, builder_type)));
            result.push(Indent(Box::new(Branch(initter_interior))));
            result.push(Line("}".to_string()));
        }
        None => {}
    }
    return Branch(result);
}


// return (the 'Which' enum, the 'which()' accessor, typedef)
fn generate_union(gen:&GeneratorContext,
                  discriminant_offset : u32,
                  fields : &[schema_capnp::field::Reader],
                  is_reader : bool)
                  -> (FormattedText, FormattedText, FormattedText)
{
    use schema_capnp::*;

    fn new_ty_param(ty_params : &mut Vec<String>) -> String {
        let result = format!("A{}", ty_params.len());
        ty_params.push(result.clone());
        result
    }

    let mut getter_interior = Vec::new();
    let mut interior = Vec::new();
    let mut enum_interior = Vec::new();

    let mut ty_params = Vec::new();
    let mut ty_args = Vec::new();

    let doffset = discriminant_offset as usize;

    for field in fields.iter() {

        let dvalue = field.get_discriminant_value() as usize;

        let field_name = field.get_name().unwrap();
        let enumerant_name = capitalize_first_letter(field_name);

        let (ty, get) = getter_text(gen, field, is_reader);

        getter_interior.push(Branch(vec!(
                    Line(format!("{} => {{", dvalue)),
                    Indent(Box::new(Line(format!("return ::std::result::Result::Ok({}(", enumerant_name.clone())))),
                    Indent(Box::new(Indent(Box::new(get)))),
                    Indent(Box::new(Line("));".to_string()))),
                    Line("}".to_string())
                )));

        let ty1 = match field.which() {
            Ok(field::Group(_)) => {
                ty_args.push(ty);
                new_ty_param(&mut ty_params)
            }
            Ok(field::Slot(reg_field)) => {
                match reg_field.get_type().unwrap().which() {
                    Ok(type_::Text(())) | Ok(type_::Data(())) |
                    Ok(type_::List(_)) | Ok(type_::Struct(_)) |
                    Ok(type_::AnyPointer(_)) => {
                        ty_args.push(ty);
                        new_ty_param(&mut ty_params)
                    }
                    Ok(type_::Interface(_)) => {
                        ty
                    }
                    _ => ty
                }
            }
            _ => ty
        };

        enum_interior.push(Line(format!("{}({}),", enumerant_name, ty1)));
    }

    let enum_name = format!("Which{}",
                            if ty_params.len() > 0 { format!("<{}>", ty_params.connect(",")) }
                            else {"".to_string()} );


    getter_interior.push(Line("x => return ::std::result::Result::Err(::capnp::NotInSchema(x))".to_string()));

    interior.push(
        Branch(vec!(Line(format!("pub enum {} {{", enum_name)),
                    Indent(Box::new(Branch(enum_interior))),
                    Line("}".to_string()))));

    let result = Branch(interior);

    let field_name = if is_reader { "reader" } else { "builder" };

    let concrete_type =
            format!("Which{}{}",
                    if is_reader {"Reader"} else {"Builder"},
                    if ty_params.len() > 0 { "<'a>" } else {""});

    let typedef =
        Line(format!("pub type {} = Which{};",
                     concrete_type,
                     if ty_args.len() > 0 {format!("<{}>",
                                                   ty_args.connect(","))} else {"".to_string()}));

    let getter_result =
        Branch(vec!(Line("#[inline]".to_string()),
                    Line("#[allow(dead_code)]".to_string()),
                    Line(format!("pub fn which(self) -> ::std::result::Result<{}, ::capnp::NotInSchema> {{",
                                 concrete_type)),
                    Indent(Box::new(Branch(vec!(
                        Line(format!("match self.{}.get_data_field::<u16>({}) {{", field_name, doffset)),
                        Indent(Box::new(Branch(getter_interior))),
                        Line("}".to_string()))))),
                    Line("}".to_string())));

    // TODO set_which() for builders?

    return (result, getter_result, typedef);
}

fn generate_haser(discriminant_offset : u32,
                  styled_name : &str,
                  field :&schema_capnp::field::Reader,
                  is_reader : bool) -> FormattedText {

    use schema_capnp::*;

    let mut result = Vec::new();
    let mut interior = Vec::new();
    let member = if is_reader { "reader" } else { "builder" };

    let discriminant_value = field.get_discriminant_value();
    if discriminant_value != field::NO_DISCRIMINANT {
       interior.push(
            Line(format!("if self.{}.get_data_field::<u16>({}) != {} {{ return false; }}",
                         member,
                         discriminant_offset as usize,
                         discriminant_value as usize)));
    }
    match field.which() {
        Err(_) | Ok(field::Group(_)) => {},
        Ok(field::Slot(reg_field)) => {
            match reg_field.get_type().unwrap().which() {
                Ok(type_::Text(())) | Ok(type_::Data(())) |
                Ok(type_::List(_)) | Ok(type_::Struct(_)) |
                Ok(type_::AnyPointer(_)) => {
                    interior.push(
                        Line(format!("!self.{}.get_pointer_field({}).is_null()",
                                     member, reg_field.get_offset())));
                    result.push(
                        Line("#[allow(dead_code)]".to_string()));
                    result.push(
                        Line(format!("pub fn has_{}(&self) -> bool {{", styled_name)));
                    result.push(
                        Indent(Box::new(Branch(interior))));
                    result.push(Line("}".to_string()));
                }
                _ => {}
            }
        }
    }

    Branch(result)
}

fn generate_pipeline_getter(gen:&GeneratorContext,
                            field : schema_capnp::field::Reader) -> FormattedText {
    use schema_capnp::{field, type_};

    let name = field.get_name().unwrap();

    match field.which() {
        Err(_) => panic!("unrecognized field type"),
        Ok(field::Group(group)) => {
            let the_mod = gen.scope_map[&group.get_type_id()].connect("::");
            return Branch(vec!(
                                Line("#[allow(dead_code)]".to_string()),
                                Line(format!("pub fn get_{}(&self) -> {}::Pipeline {{",
                                            camel_to_snake_case(name),
                                            the_mod)),
                               Indent(
                                   Box::new(Line("FromTypelessPipeline::new(self._typeless.noop())".to_string()))),
                               Line("}".to_string())));
        }
        Ok(field::Slot(reg_field)) => {
            let typ = reg_field.get_type().unwrap();
            match typ.which() {
                Err(_) => panic!("unrecognized type"),
                Ok(type_::Struct(_)) => {
                    return Branch(vec!(
                        Line("#[allow(dead_code)]".to_string()),
                        Line(format!("pub fn get_{}(&self) -> {} {{",
                                     camel_to_snake_case(name),
                                     typ.type_string(gen, Module::Pipeline, ""))),
                        Indent(Box::new(Line(
                            format!("FromTypelessPipeline::new(self._typeless.get_pointer_field({}))",
                                    reg_field.get_offset())))),
                        Line("}".to_string())));
                }
                Ok(type_::Interface(_)) => {
                    return Branch(vec!(
                        Line("#[allow(dead_code)]".to_string()),
                        Line(format!("pub fn get_{}(&self) -> {} {{",
                                     camel_to_snake_case(name),
                                     typ.type_string(gen, Module::Client, ""))),
                        Indent(Box::new(Line(
                            format!("FromClientHook::new(self._typeless.get_pointer_field({}).as_cap())",
                                    reg_field.get_offset())))),
                        Line("}".to_string())));
                }
                _ => {
                    return Branch(Vec::new());
                }
            }
        }
    }
}

fn generate_node(gen:&GeneratorContext,
                 node_id : u64,
                 node_name: &str) -> FormattedText {
    use schema_capnp::*;

    let mut output: Vec<FormattedText> = Vec::new();
    let mut nested_output: Vec<FormattedText> = Vec::new();

    let node_reader = &gen.node_map[&node_id];
    let nested_nodes = node_reader.get_nested_nodes().unwrap();
    for nested_node in nested_nodes.iter() {
        let id = nested_node.get_id();
        nested_output.push(generate_node(gen, id, &gen.scope_map[&id].last().unwrap()));
    }

    match node_reader.which() {

        Ok(node::File(())) => {
            output.push(Branch(nested_output));
        }

        Ok(node::Struct(struct_reader)) => {
            let params = node_reader.parameters_texts(gen);
            output.push(BlankLine);

            let is_generic = node_reader.get_is_generic();
            if is_generic {
                output.push(Line(format!("pub mod {} {{ /* {} */", node_name, params.expanded_list.connect(","))));
            } else {
                output.push(Line(format!("pub mod {} {{", node_name)));
            }
            let bracketed_params = if params.params == "" { "".to_string() } else { format!("<{}>", params.params) };

            let mut preamble = Vec::new();
            let mut builder_members = Vec::new();
            let mut reader_members = Vec::new();
            let mut union_fields = Vec::new();
            let mut which_enums = Vec::new();
            let mut pipeline_impl_interior = Vec::new();
            let mut private_mod_interior = Vec::new();

            let data_size = struct_reader.get_data_word_count();
            let pointer_size = struct_reader.get_pointer_count();
            let discriminant_count = struct_reader.get_discriminant_count();
            let discriminant_offset = struct_reader.get_discriminant_offset();

            preamble.push(generate_import_statements());
            preamble.push(BlankLine);

            if is_generic {
                preamble.push(generate_import_statements_for_generics());
                preamble.push(BlankLine);
            }

            let fields = struct_reader.get_fields().unwrap();
            for field in fields.iter() {
                let name = field.get_name().unwrap();
                let styled_name = camel_to_snake_case(name);

                let discriminant_value = field.get_discriminant_value();
                let is_union_field = discriminant_value != field::NO_DISCRIMINANT;

                if !is_union_field {
                    pipeline_impl_interior.push(generate_pipeline_getter(gen, field));
                    let (ty, get) = getter_text(gen, &field, true);
                    reader_members.push(
                        Branch(vec!(
                            Line("#[inline]".to_string()),
                            Line("#[allow(dead_code)]".to_string()),
                            Line(format!("pub fn get_{}(self) -> {} {{", styled_name, ty)),
                            Indent(Box::new(get)),
                            Line("}".to_string()))));

                    let (ty_b, get_b) = getter_text(gen, &field, false);

                    builder_members.push(
                        Branch(vec!(
                            Line("#[inline]".to_string()),
                            Line("#[allow(dead_code)]".to_string()),
                            Line(format!("pub fn get_{}(self) -> {} {{", styled_name, ty_b)),
                            Indent(Box::new(get_b)),
                            Line("}".to_string()))));

                } else {
                    union_fields.push(field);
                }

                builder_members.push(generate_setter(gen, discriminant_offset,
                                                     &styled_name, &field));

                reader_members.push(generate_haser(discriminant_offset, &styled_name, &field, true));
                builder_members.push(generate_haser(discriminant_offset, &styled_name, &field, false));

                match field.which() {
                    Ok(field::Group(group)) => {
                        let id = group.get_type_id();
                        let text = generate_node(gen, id,
                                                 &gen.scope_map[&id].last().unwrap());
                        nested_output.push(text);
                    }
                    _ => { }
                }

            }

            if discriminant_count > 0 {
                let (which_enums1, union_getter, typedef) =
                    generate_union(gen, discriminant_offset, &union_fields, true);
                which_enums.push(which_enums1);
                which_enums.push(typedef);
                reader_members.push(union_getter);

                let (_, union_getter, typedef) =
                    generate_union(gen, discriminant_offset, &union_fields, false);
                which_enums.push(typedef);
                builder_members.push(union_getter);

                let mut reexports = String::new();
                reexports.push_str("pub use self::Which::{");
                let whichs : Vec<String> =
                    union_fields.iter().map(|f| {capitalize_first_letter(f.get_name().unwrap())}).collect();
                reexports.push_str(&whichs.connect(","));
                reexports.push_str("};");
                preamble.push(Line(reexports));
                preamble.push(BlankLine);
            }

            let builder_struct_size =
                Branch(vec!(
                    Line(format!("impl <'a,{}> ::capnp::traits::HasStructSize for Builder<'a,{}>", params.params, params.params)),
                    Line(params.where_clause.clone() + "{"),
                    Indent(Box::new(
                        Branch(vec!(Line("#[inline]".to_string()),
                                    Line("fn struct_size() -> layout::StructSize { _private::STRUCT_SIZE }".to_string()))))),
                   Line("}".to_string())));


            private_mod_interior.push(
                Line(
                    "use capnp::private::layout;".to_string()));
            private_mod_interior.push(
                Line(
                    format!("pub const STRUCT_SIZE : layout::StructSize = layout::StructSize {{ data : {}, pointers : {} }};",
                            data_size as usize, pointer_size as usize)));
            private_mod_interior.push(
                Line(
                    format!("pub const TYPE_ID: u64 = {:#x};", node_id)));


            let from_pointer_builder_impl =
                Branch(vec![
                    Line(format!("impl <'a,{}> ::capnp::traits::FromPointerBuilder<'a> for Builder<'a,{}>", params.params, params.params)),
                    Line(params.where_clause.clone() + " {"),
                    Indent(
                        Box::new(
                            Branch(vec!(
                                Line(format!("fn init_pointer(builder: ::capnp::private::layout::PointerBuilder<'a>, _size : u32) -> Builder<'a,{}> {{", params.params)),
                                Indent(Box::new(Line("::capnp::traits::FromStructBuilder::new(builder.init_struct(_private::STRUCT_SIZE))".to_string()))),
                                Line("}".to_string()),
                                Line(format!("fn get_from_pointer(builder: ::capnp::private::layout::PointerBuilder<'a>) -> Result<Builder<'a,{}>> {{", params.params)),
                                Indent(Box::new(Line("::std::result::Result::Ok(::capnp::traits::FromStructBuilder::new(try!(builder.get_struct(_private::STRUCT_SIZE, ::std::ptr::null()))))".to_string()))),
                                Line("}".to_string()))))),
                    Line("}".to_string()),
                    BlankLine]);

            let accessors = vec!(
                Branch(preamble),
                Line("#[allow(dead_code)]".to_string()),
                (if !is_generic {
                    Branch(vec!(
                        Line("pub struct Owned;".to_string()),
                        Line("impl <'a> ::capnp::traits::Owned<'a> for Owned { type Reader = Reader<'a>; type Builder = Builder<'a>; type Pipeline = Pipeline; }".to_string()),
                        Line("impl <'a> ::capnp::traits::OwnedStruct<'a> for Owned { type Reader = Reader<'a>; type Builder = Builder<'a>; type Pipeline = Pipeline; }".to_string())
                    ))
                } else {
                    Branch(vec!(
                        Line(format!("pub struct Owned<{}> {{", params.params)),
                            Indent(Box::new(Line(format!("_phantom: PhantomData<({})>", params.params)))),
                        Line("}".to_string()),
                        Line(format!("impl <'a, {}> ::capnp::traits::Owned<'a> for Owned <{}> {} {{ type Reader = Reader<'a, {}>; type Builder = Builder<'a, {}>; type Pipeline = Pipeline{}; }}",
                            params.params, params.params, params.where_clause, params.params, params.params, bracketed_params)),
                        Line(format!("impl <'a, {}> ::capnp::traits::OwnedStruct<'a> for Owned <{}> {} {{ type Reader = Reader<'a, {}>; type Builder = Builder<'a, {}>; type Pipeline = Pipeline{}; }}",
                            params.params, params.params, params.where_clause, params.params, params.params, bracketed_params)),
                    ))
                }),
                BlankLine,
                Line("#[derive(Clone, Copy)]".to_string()),
                (if !is_generic {
                    Line("pub struct Reader<'a> { reader : layout::StructReader<'a> }".to_string())
                } else {
                    Branch(vec!(
                        Line(format!("pub struct Reader<'a,{}>", params.params)),
                        Line(params.where_clause.clone() + " {"),
                        Indent(Box::new(Branch(vec!(
                            Line("reader : layout::StructReader<'a>,".to_string()),
                            Line(format!("_phantom: PhantomData<({})>", params.params)),
                        )))),
                        Line("}".to_string())
                    ))
                }),
                BlankLine,
                Branch(vec!(
                        Line(format!("impl <'a,{}> ::capnp::traits::HasTypeId for Reader<'a,{}>",
                            params.params, params.params)),
                        Line(params.where_clause.clone() + "{"),
                        Indent(Box::new(Branch(vec!(Line("#[inline]".to_string()),
                                               Line("fn type_id() -> u64 { _private::TYPE_ID }".to_string()))))),
                    Line("}".to_string()))),
                Line(format!("impl <'a,{}> ::capnp::traits::FromStructReader<'a> for Reader<'a,{}>",
                            params.params, params.params)),
                Line(params.where_clause.clone() + "{"),
                Indent(
                    Box::new(Branch(vec!(
                        Line(format!("fn new(reader: ::capnp::private::layout::StructReader<'a>) -> Reader<'a,{}> {{", params.params)),
                        Indent(Box::new(Line(format!("Reader {{ reader : reader, {} }}", params.phantom_data)))),
                        Line("}".to_string()))))),
                Line("}".to_string()),
                BlankLine,
                Line(format!("impl <'a,{}> ::capnp::traits::FromPointerReader<'a> for Reader<'a,{}>",
                    params.params, params.params)),
                Line(params.where_clause.clone() + "{"),
                Indent(
                    Box::new(Branch(vec!(
                        Line(format!("fn get_from_pointer(reader: &::capnp::private::layout::PointerReader<'a>) -> Result<Reader<'a,{}>> {{",params.params)),
                        Indent(Box::new(Line("::std::result::Result::Ok(::capnp::traits::FromStructReader::new(try!(reader.get_struct(::std::ptr::null()))))".to_string()))),
                        Line("}".to_string()))))),
                Line("}".to_string()),
                BlankLine,
                Line(format!("impl <'a,{}> Reader<'a,{}>", params.params, params.params)),
                Line(params.where_clause.clone() + "{"),
                Indent(
                    Box::new(Branch(vec![
                        Line("#[allow(dead_code)]".to_string()),
                        Line(format!("pub fn borrow<'b>(&'b self) -> Reader<'b,{}> {{",params.params)),
                        Indent(Box::new(Line("Reader { .. *self }".to_string()))),
                        Line("}".to_string()),
                        BlankLine,
                        Line("#[allow(dead_code)]".to_string()),
                        Line("pub fn total_size(&self) -> Result<::capnp::MessageSize> {".to_string()),
                        Indent(Box::new(Line("self.reader.total_size()".to_string()))),
                        Line("}".to_string())]))),
                Indent(Box::new(Branch(reader_members))),
                Line("}".to_string()),
                BlankLine,
                (if !is_generic {
                    Line("pub struct Builder<'a> { builder : ::capnp::private::layout::StructBuilder<'a> }".to_string())
                } else {
                    Branch(vec!(
                        Line(format!("pub struct Builder<'a,{}>", params.params)),
                        Line(params.where_clause.clone() + " {"),
                        Indent(Box::new(Branch(vec!(
                            Line("builder : ::capnp::private::layout::StructBuilder<'a>,".to_string()),
                            Line(format!("_phantom: PhantomData<({})>", params.params)),
                        )))),
                        Line("}".to_string())
                    ))
                }),
                builder_struct_size,
                Branch(vec!(
                        Line(format!("impl <'a,{}> ::capnp::traits::HasTypeId for Builder<'a,{}>", params.params, params.params)),
                        Line(params.where_clause.clone() + " {"),
                        Indent(Box::new(Branch(vec!(Line("#[inline]".to_string()),
                                                    Line("fn type_id() -> u64 { _private::TYPE_ID }".to_string()))))),
                               Line("}".to_string()))),
                Line(format!("impl <'a,{}> ::capnp::traits::FromStructBuilder<'a> for Builder<'a,{}>", params.params, params.params)),
                Line(params.where_clause.clone() + " {"),
                Indent(
                    Box::new(Branch(vec!(
                        Line(format!("fn new(builder : ::capnp::private::layout::StructBuilder<'a>) -> Builder<'a, {}> {{", params.params)),
                        Indent(Box::new(Line(format!("Builder {{ builder : builder, {} }}", params.phantom_data)))),
                        Line("}".to_string()))))),
                Line("}".to_string()),
                BlankLine,
                from_pointer_builder_impl,
                Line(format!("impl <'a,{}> ::capnp::traits::SetPointerBuilder<Builder<'a,{}>> for Reader<'a,{}>", params.params, params.params, params.params)),
                Line(params.where_clause.clone() + " {"),
                Indent(Box::new(Line(format!("fn set_pointer_builder<'b>(pointer : ::capnp::private::layout::PointerBuilder<'b>, value : Reader<'a,{}>) -> Result<()> {{ pointer.set_struct(&value.reader) }}", params.params)))),
                Line("}".to_string()),
                BlankLine,
                Line(format!("impl <'a,{}> Builder<'a,{}>", params.params, params.params)),
                Line(params.where_clause + " {"),
                Indent(
                    Box::new(Branch(vec![
                        Line("#[allow(dead_code)]".to_string()),
                        Line(format!("pub fn as_reader(self) -> Reader<'a,{}> {{", params.params)),
                        Indent(Box::new(Line("::capnp::traits::FromStructReader::new(self.builder.as_reader())".to_string()))),
                        Line("}".to_string()),
                        Line("#[allow(dead_code)]".to_string()),
                        Line(format!("pub fn borrow<'b>(&'b mut self) -> Builder<'b,{}> {{", params.params)),
                        Indent(Box::new(Line("Builder { .. *self }".to_string()))),
                        Line("}".to_string()),

                        BlankLine,
                        Line("#[allow(dead_code)]".to_string()),
                        Line("pub fn total_size(&self) -> Result<::capnp::MessageSize> {".to_string()),
                        Indent(Box::new(Line("self.builder.as_reader().total_size()".to_string()))),
                        Line("}".to_string())
                        ]))),
                Indent(Box::new(Branch(builder_members))),
                Line("}".to_string()),
                BlankLine,
                (if is_generic {
                    Branch(vec![
                        Line(format!("pub struct Pipeline{} {{", bracketed_params)),
                        Indent(Box::new(Branch(vec![
                            Line("_typeless : ::capnp::any_pointer::Pipeline,".to_string()),
                            Line(format!("_phantom: PhantomData<({})>", params.params)),
                        ]))),
                        Line("}".to_string())
                    ])
                } else {
                    Line("pub struct Pipeline { _typeless : ::capnp::any_pointer::Pipeline }".to_string())
                }),
                Line(format!("impl{} FromTypelessPipeline for Pipeline{} {{", bracketed_params, bracketed_params)),
                Indent(
                    Box::new(Branch(vec!(
                        Line(format!("fn new(typeless : ::capnp::any_pointer::Pipeline) -> Pipeline{} {{", bracketed_params)),
                        Indent(Box::new(Line(format!("Pipeline {{ _typeless : typeless, {} }}", params.phantom_data)))),
                        Line("}".to_string()))))),
                Line("}".to_string()),
                Line(format!("impl{} Pipeline{} {{", bracketed_params, bracketed_params)),
                Indent(Box::new(Branch(pipeline_impl_interior))),
                Line("}".to_string()),
                Line("mod _private {".to_string()),
                Indent(Box::new(Branch(private_mod_interior))),
                Line("}".to_string()),
                );

            output.push(Indent(Box::new(Branch(vec!(Branch(accessors),
                                                    Branch(which_enums),
                                                    Branch(nested_output))))));
            output.push(Line("}".to_string()));

        }

        Ok(node::Enum(enum_reader)) => {
            let names = &gen.scope_map[&node_id];
            output.push(BlankLine);

            let mut members = Vec::new();
            let mut match_branches = Vec::new();
            let enumerants = enum_reader.get_enumerants().unwrap();
            for ii in 0..enumerants.len() {
                let enumerant = capitalize_first_letter(enumerants.get(ii).get_name().unwrap());
                members.push(Line(format!("{} = {},", enumerant, ii)));
                match_branches.push(Line(format!("{} => ::std::result::Result::Ok({}::{}),", ii, *names.last().unwrap(), enumerant)));
            }
            match_branches.push(Line("n => ::std::result::Result::Err(::capnp::NotInSchema(n)),".to_string()));

            output.push(Branch(vec!(
                Line("#[repr(u16)]".to_string()),
                Line("#[derive(Clone, Copy, PartialEq)]".to_string()),
                Line(format!("pub enum {} {{", *names.last().unwrap())),
                Indent(Box::new(Branch(members))),
                Line("}".to_string()))));

            output.push(
                Branch(vec!(
                    Line(format!("impl ::capnp::traits::FromU16 for {} {{", *names.last().unwrap())),
                    Indent(Box::new(Line("#[inline]".to_string()))),
                    Indent(
                        Box::new(Branch(vec![
                            Line(format!(
                                "fn from_u16(value : u16) -> ::std::result::Result<{}, ::capnp::NotInSchema> {{",
                                *names.last().unwrap())),
                            Indent(
                                Box::new(Branch(vec![
                                    Line("match value {".to_string()),
                                    Indent(Box::new(Branch(match_branches))),
                                    Line("}".to_string())
                                        ]))),
                            Line("}".to_string())]))),
                    Line("}".to_string()),
                    Line(format!("impl ::capnp::traits::ToU16 for {} {{", *names.last().unwrap())),
                    Indent(Box::new(Line("#[inline]".to_string()))),
                    Indent(
                        Box::new(Line("fn to_u16(self) -> u16 { self as u16 }".to_string()))),
                    Line("}".to_string()))));

            output.push(
                Branch(vec!(
                    Line(format!("impl ::capnp::traits::HasTypeId for {} {{", *names.last().unwrap())),
                    Indent(Box::new(Line("#[inline]".to_string()))),
                    Indent(
                        Box::new(Line(format!("fn type_id() -> u64 {{ {:#x}u64 }}", node_id).to_string()))),
                    Line("}".to_string()))));
        }

        Ok(node::Interface(interface)) => {
            let params = node_reader.parameters_texts(gen);
            output.push(BlankLine);

            let is_generic = node_reader.get_is_generic();

            let names = &gen.scope_map[&node_id];
            let mut client_impl_interior = Vec::new();
            let mut server_interior = Vec::new();
            let mut mod_interior = Vec::new();
            let mut dispatch_arms = Vec::new();
            let mut private_mod_interior = Vec::new();

            let bracketed_params = if params.params == "" { "".to_string() } else { format!("<{}>", params.params) };

            private_mod_interior.push(Line(format!("pub const TYPE_ID: u64 = {:#x};", node_id)));

            mod_interior.push(Line ("#![allow(unused_variables)]".to_string()));
            mod_interior.push(generate_import_statements());
            if is_generic {
                mod_interior.push(generate_import_statements_for_generics())
            }
            mod_interior.push(
                Line("use capnp::capability::Request;".to_string()));
            mod_interior.push(
                Line("use capnp::private::capability::{ClientHook, ServerHook};".to_string()));
            mod_interior.push(Line("use capnp::capability;".to_string()));
            mod_interior.push(BlankLine);

            let methods = interface.get_methods().unwrap();
            for ordinal in 0..methods.len() {
                let method = methods.get(ordinal);
                let name = method.get_name().unwrap();

                method.get_code_order();
                let param_id = method.get_param_struct_type();
                let param_node = &gen.node_map[&param_id];
                let param_scopes = if param_node.get_scope_id() == 0 {
                    let mut names = names.clone();
                    let local_name = module_name(&format!("{}Params", name));
                    nested_output.push(generate_node(gen, param_id, &*local_name));
                    names.push(local_name);
                    names
                } else {
                    gen.scope_map[&param_node.get_id()].clone()
                };
                let param_type = param_node.type_string(&gen, &method.get_param_brand().unwrap(), Some(&param_scopes), Module::Owned, "'a");

                let result_id = method.get_result_struct_type();
                let result_node = &gen.node_map[&result_id];
                let result_scopes = if result_node.get_scope_id() == 0 {
                    let mut names = names.clone();
                    let local_name = module_name(&format!("{}Results", name));
                    nested_output.push(generate_node(gen, result_id, &*local_name));
                    names.push(local_name);
                    names
                } else {
                    gen.scope_map[&result_node.get_id()].clone()
                };
                let result_type = result_node.type_string(&gen, &method.get_result_brand().unwrap(), Some(&result_scopes), Module::Owned, "'a");

                dispatch_arms.push(
                    Line(format!(
                            "{} => server.{}(::capnp::private::capability::internal_get_typed_context(context)),",
                            ordinal, camel_to_snake_case(name))));
                mod_interior.push(
                    Line(format!(
                            "pub type {}Context<'a,{}> = capability::CallContext<{}, {}>;",
                            capitalize_first_letter(name), params.params, param_type, result_type)));
                server_interior.push(
                    Line(format!(
                            "fn {}<'a>(&mut self, {}Context<'a,{}>);",
                            camel_to_snake_case(name), capitalize_first_letter(name), params.params
                            )));

                client_impl_interior.push(
                    Line(format!("pub fn {}_request<'a>(&self) -> Request<{},{}> {{",
                                 camel_to_snake_case(name), param_type, result_type)));

                client_impl_interior.push(Indent(
                    Box::new(Line(format!("self.client.new_call(_private::TYPE_ID, {}, None)", ordinal)))));
                client_impl_interior.push(Line("}".to_string()));

                method.get_annotations().unwrap();
            }

            let mut base_dispatch_arms = Vec::new();
            let server_base = {
                let mut base_traits = Vec::new();
                let extends = interface.get_superclasses().unwrap();
                for ii in 0..extends.len() {
                    let base_id = extends.get(ii).get_id();
                    let the_mod = gen.scope_map[&base_id].connect("::");
                    base_dispatch_arms.push(
                        Line(format!(
                                "0x{:x} => {}::ServerDispatch::<T>::dispatch_call_internal(&mut *self.server, method_id, context),",
                                base_id, the_mod)));
                    base_traits.push(format!("{}::Server", the_mod));
                }
                if extends.len() > 0 { format!(": {}", base_traits.connect(" + ")) }
                else { "".to_string() }
            };

            mod_interior.push(BlankLine);
            mod_interior.push(Line(format!("pub struct Client{} {{", bracketed_params)));
            mod_interior.push(Indent(Box::new(Line("pub client : ::capnp::private::capability::Client,".to_string()))));
            if is_generic {
                mod_interior.push(Indent(Box::new(Line(format!("_phantom: PhantomData<({})>", params.params)))))
            }
            mod_interior.push(Line("}".to_string()));
            mod_interior.push(
                Branch(vec!(
                    Line(format!("impl {} FromClientHook for Client{} {{", bracketed_params, bracketed_params)),
                    Indent(Box::new(Line(format!("fn new(hook : Box<ClientHook+Send>) -> Client{} {{", bracketed_params)))),
                    Indent(Box::new(Indent(Box::new(Line(format!("Client {{ client : ::capnp::private::capability::Client::new(hook), {} }}", params.phantom_data)))))),
                    Indent(Box::new(Line("}".to_string()))),
                    Line("}".to_string()))));


            mod_interior.push(
                Branch(vec!(
                    (if is_generic {
                        Branch(vec!(
                            Line(format!("pub struct ToClient<U,{}> {{", params.params)),
                            Indent(Box::new(Branch(vec!(
                                Line("pub u: U,".to_string()),
                                Line(format!("_phantom: PhantomData<({})>", params.params))
                            )))),
                            Line("}".to_string()),
                            Line(format!("impl <{}, U : Server<{}> + Send + 'static> ToClient<U,{}>", params.params, params.params, params.params)),
                            Line(params.where_clause_with_send.clone() + "{"),
                        ))
                    } else {
                        Branch(vec!(
                            Line("pub struct ToClient<U>(pub U);".to_string()),
                            Line("impl <U : Server + Send + 'static> ToClient<U> {".to_string())
                        ))
                    }),
                    Indent(Box::new(Branch( vec!(
                        Line("#[allow(dead_code)]".to_string()),
                        Line(format!("pub fn from_server<T: ServerHook>(self) -> Client{} {{", bracketed_params)),
                        Indent(
                            Box::new(Line(format!("Client {{ client : T::new_client(::std::boxed::Box::new(ServerDispatch {{ server : ::std::boxed::Box::new(self.u), {} }})), {} }}", params.phantom_data, params.phantom_data)))),
                        Line("}".to_string()))))),
                    Line("}".to_string()))));


            mod_interior.push(
                    Branch(vec!(
                        Line(format!("impl {} ::capnp::traits::HasTypeId for Client{} {{", bracketed_params, bracketed_params)),
                        Indent(Box::new(Line("#[inline]".to_string()))),
                        Indent(Box::new(Line("fn type_id() -> u64 { _private::TYPE_ID }".to_string()))),
                        Line("}".to_string()))));


            mod_interior.push(
                    Branch(vec!(
                        Line(format!("impl {} Clone for Client{} {{", bracketed_params, bracketed_params)),
                        Indent(Box::new(Line(format!("fn clone(&self) -> Client{} {{", bracketed_params)))),
                        Indent(Box::new(Indent(Box::new(Line("Client { client : ::capnp::private::capability::Client::new(self.client.hook.copy()) }".to_string()))))),
                        Indent(Box::new(Line("}".to_string()))),
                        Line("}".to_string()))));


            mod_interior.push(
                Branch(vec!(Line(format!("impl {} Client{} {{", bracketed_params, bracketed_params)),
                            Indent(Box::new(Branch(client_impl_interior))),
                            Line("}".to_string()))));

            mod_interior.push(Branch(vec!(Line(format!("pub trait Server<{}> {} {{", params.params, server_base)),
                                          Indent(Box::new(Branch(server_interior))),
                                          Line("}".to_string()))));

            mod_interior.push(Branch(vec!(Line(format!("pub struct ServerDispatch<T,{}> {{", params.params)),
                                          Indent(Box::new(Line("pub server : Box<T>,".to_string()))),
                                          Indent(Box::new(Branch(if is_generic {
                                            vec!(Line(format!("_phantom: PhantomData<({})>", params.params))) } else { vec!() } ))),
                                          Line("}".to_string()))));

            mod_interior.push(
                Branch(vec!(
                    (if is_generic {
                        Line(format!("impl <{}, T : Server{}> ::capnp::capability::Server for ServerDispatch<T,{}> {{", params.params, bracketed_params, params.params))
                    } else {
                        Line("impl <T : Server> ::capnp::capability::Server for ServerDispatch<T> {".to_string())
                    }),
                    Indent(Box::new(Line("fn dispatch_call(&mut self, interface_id : u64, method_id : u16, context : capability::CallContext<::capnp::any_pointer::Reader, ::capnp::any_pointer::Builder>) {".to_string()))),
                    Indent(Box::new(Indent(Box::new(Line("match interface_id {".to_string()))))),
                    Indent(Box::new(Indent(Box::new(Indent(
                        Box::new(Line("_private::TYPE_ID => ServerDispatch::<T>::dispatch_call_internal(&mut *self.server, method_id, context),".to_string()))))))),
                    Indent(Box::new(Indent(Box::new(Indent(Box::new(Branch(base_dispatch_arms))))))),
                    Indent(Box::new(Indent(Box::new(Indent(Box::new(Line("_ => {}".to_string()))))))),
                    Indent(Box::new(Indent(Box::new(Line("}".to_string()))))),
                    Indent(Box::new(Line("}".to_string()))),
                    Line("}".to_string()))));

            mod_interior.push(
                Branch(vec!(
                    (if is_generic {
                        Line(format!("impl <{}, T : Server{}> ServerDispatch<T,{}> {{", params.params, bracketed_params, params.params))
                    } else {
                        Line("impl <T : Server> ServerDispatch<T> {".to_string())
                    }),
                    Line("#[allow(dead_code)]".to_string()),
                    Indent(Box::new(Line("pub fn dispatch_call_internal(server :&mut T, method_id : u16, context : capability::CallContext<::capnp::any_pointer::Reader, ::capnp::any_pointer::Builder>) {".to_string()))),
                    Indent(Box::new(Indent(Box::new(Line("match method_id {".to_string()))))),
                    Indent(Box::new(Indent(Box::new(Indent(Box::new(Branch(dispatch_arms))))))),
                    Indent(Box::new(Indent(Box::new(Indent(Box::new(Line("_ => {}".to_string()))))))),
                    Indent(Box::new(Indent(Box::new(Line("}".to_string()))))),
                    Indent(Box::new(Line("}".to_string()))),
                    Line("}".to_string()))));

            mod_interior.push(
                Branch(vec!(
                    Line("pub mod _private {".to_string()),
                    Indent(Box::new(Branch(private_mod_interior))),
                    Line("}".to_string()),
                    )));


            mod_interior.push(Branch(vec!(Branch(nested_output))));


            output.push(BlankLine);
            if is_generic {
                output.push(Line(format!("pub mod {} {{ /* ({}) */", node_name, params.expanded_list.connect(","))));
            } else {
                output.push(Line(format!("pub mod {} {{", node_name)));
            }
            output.push(Indent(Box::new(Branch(mod_interior))));
            output.push(Line("}".to_string()));
        }

        Ok(node::Const(c)) => {
            let names = &gen.scope_map[&node_id];
            let styled_name = snake_to_upper_case(&names.last().unwrap());

            let (typ, txt) = match tuple_result(c.get_type().unwrap().which(), c.get_value().unwrap().which()) {
                Ok((type_::Void(()), value::Void(()))) => ("()".to_string(), "()".to_string()),
                Ok((type_::Bool(()), value::Bool(b))) => ("bool".to_string(), b.to_string()),
                Ok((type_::Int8(()), value::Int8(i))) => ("i8".to_string(), i.to_string()),
                Ok((type_::Int16(()), value::Int16(i))) => ("i16".to_string(), i.to_string()),
                Ok((type_::Int32(()), value::Int32(i))) => ("i32".to_string(), i.to_string()),
                Ok((type_::Int64(()), value::Int64(i))) => ("i64".to_string(), i.to_string()),
                Ok((type_::Uint8(()), value::Uint8(i))) => ("u8".to_string(), i.to_string()),
                Ok((type_::Uint16(()), value::Uint16(i))) => ("u16".to_string(), i.to_string()),
                Ok((type_::Uint32(()), value::Uint32(i))) => ("u32".to_string(), i.to_string()),
                Ok((type_::Uint64(()), value::Uint64(i))) => ("u64".to_string(), i.to_string()),

                // float string formatting appears to be a bit broken currently, in Rust.
                Ok((type_::Float32(()), value::Float32(f))) => ("f32".to_string(), format!("{}f32", f.to_string())),
                Ok((type_::Float64(()), value::Float64(f))) => ("f64".to_string(), format!("{}f64", f.to_string())),

                Ok((type_::Text(()), value::Text(_t))) => { panic!() }
                Ok((type_::Data(()), value::Data(_d))) => { panic!() }
                Ok((type_::List(_t), value::List(_p))) => { panic!() }
                Ok((type_::Struct(_t), value::Struct(_p))) => { panic!() }
                Ok((type_::Interface(_t), value::Interface(()))) => { panic!() }
                Ok((type_::AnyPointer(_), value::AnyPointer(_pr))) => { panic!() }
                Err(_) => { panic!("unrecognized type") }
                _ => { panic!("type does not match value") }
            };

            output.push(
                Line(format!("pub const {} : {} = {};", styled_name, typ, txt)));
        }

        Ok(node::Annotation( annotation_reader )) => {
            println!("  annotation node:");
            if annotation_reader.get_targets_file() {
                println!("  targets file");
            }
            if annotation_reader.get_targets_const() {
                println!("  targets const");
            }
            // ...
            if annotation_reader.get_targets_annotation() {
                println!("  targets annotation");
            }
        }

        Err(_) => ()
    }

    Branch(output)
}



pub fn main<T : ::std::io::Read>(mut inp : T, out_dir : &::std::path::Path) -> ::capnp::Result<()> {
    //! Generate Rust code according to a `schema_capnp::code_generator_request` read from `inp`.

    use capnp::serialize;
    use std::borrow::ToOwned;
    use std::io::Write;

    let message = try!(serialize::read_message(&mut inp, capnp::message::ReaderOptions::new()));

    let gen = try!(GeneratorContext::new(&message));

    for requested_file in try!(gen.request.get_requested_files()).iter() {
        let id = requested_file.get_id();
        let mut filepath = out_dir.to_path_buf();
        let root_name : String = format!("{}_capnp",
                                         ::std::path::PathBuf::from(requested_file.get_filename().unwrap()).
                                         file_stem().unwrap().to_owned().
                                         into_string().unwrap().replace("-", "_"));
        filepath.push(&format!("{}.rs", root_name));

        let lines = Branch(vec!(
            Line("// Generated by the capnpc-rust plugin to the Cap'n Proto schema compiler.".to_string()),
            Line("// DO NOT EDIT.".to_string()),
            Line(format!("// source: {}", try!(requested_file.get_filename()))),
            BlankLine,
            generate_node(&gen, id, &root_name)));

        let text = stringify(&lines);

        // It would be simpler to use try! instead of a pattern match, but then the error message
        // would not include `filepath`.
        match ::std::fs::File::create(&filepath) {
            Ok(ref mut writer) => {
                try!(writer.write_all(text.as_bytes()));
            }
            Err(e) => {
                let _ = writeln!(&mut ::std::io::stderr(),
                                 "could not open file {:?} for writing: {}", filepath, e);
                return Err(::capnp::Error::Io(e));
            }
        }
    }
    Ok(())
}

