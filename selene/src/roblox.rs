use chrono::Local;
use std::{collections::BTreeMap, fmt, io::Write};

mod api;

use api::*;
use selene_lib::standard_library::*;

const API_DUMP: &str =
    "https://raw.githubusercontent.com/CloneTrooper1019/Roblox-Client-Tracker/roblox/API-Dump.json";

pub struct RobloxGenerator {
    pub std: StandardLibrary,
    pub show_deprecated: bool,
}

pub enum GenerateError {
    Http(reqwest::Error),
    Io(std::io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

impl fmt::Display for GenerateError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GenerateError::Http(error) => write!(formatter, "HTTP error: {}", error),
            GenerateError::Io(error) => write!(formatter, "IO error: {}", error),
            GenerateError::TomlDe(error) => {
                write!(formatter, "TOML deserialization error: {}", error)
            }
            GenerateError::TomlSer(error) => {
                write!(formatter, "TOML serialization error: {}", error)
            }
        }
    }
}

impl RobloxGenerator {
    pub fn generate(mut self) -> Result<(Vec<u8>, StandardLibrary), GenerateError> {
        let api: ApiDump = reqwest::get(API_DUMP)
            .and_then(|mut response| response.json())
            .map_err(GenerateError::Http)?;

        self.write_class(&api, "game", "DataModel");
        self.write_class(&api, "plugin", "Plugin");
        self.write_class(&api, "script", "Script");
        self.write_class(&api, "workspace", "Workspace");

        self.write_enums(&api);
        self.write_instance_new(&api);
        self.write_get_service(&api);

        self.deprecated_event_methods();

        let mut bytes = Vec::new();

        writeln!(bytes, "# This file was @generated by generate-roblox-std at {}", Local::now())
            .map_err(GenerateError::Io)?;

        write!(
            bytes,
            "{}",
            toml::to_string(&self.std).map_err(GenerateError::TomlSer)?
        )
        .map_err(GenerateError::Io)?;

        self.std.extend(
            StandardLibrary::from_name(&self.std.meta.as_ref().unwrap().base.as_ref().unwrap())
                .unwrap(),
        );

        self.std.inflate();

        Ok((bytes, self.std))
    }

    pub fn base_std() -> Result<StandardLibrary, GenerateError> {
        toml::from_str(include_str!("./roblox/base.toml")).map_err(GenerateError::TomlDe)
    }

    fn write_class(&mut self, api: &api::ApiDump, global_name: &str, class_name: &str) {
        self.write_class_struct(api, class_name);
        self.std
            .globals
            .insert(global_name.to_owned(), Field::Struct(class_name.to_owned()));
    }

    fn write_class_struct(&mut self, api: &api::ApiDump, class_name: &str) {
        let structs = self.std.meta.as_mut().unwrap().structs.as_mut().unwrap();
        if structs.contains_key(class_name) {
            return;
        }
        structs.insert(class_name.to_owned(), BTreeMap::new());

        let mut table = BTreeMap::new();
        table.insert("*".to_owned(), Field::Struct("Instance".to_owned()));
        self.write_class_members(api, &mut table, class_name);

        let structs = self.std.meta.as_mut().unwrap().structs.as_mut().unwrap();
        structs.insert(class_name.to_owned(), table);
    }

    fn write_class_members(
        &mut self,
        api: &api::ApiDump,
        table: &mut BTreeMap<String, Field>,
        class_name: &str,
    ) {
        let class = api.classes.iter().find(|c| c.name == class_name).unwrap();

        for member in &class.members {
            let (name, tags, field) = match &member {
                ApiMember::Callback { name, tags } => (
                    name,
                    tags,
                    Some(Field::Property {
                        writable: Some(Writable::Overridden),
                    }),
                ),

                ApiMember::Event { name, tags } => {
                    (name, tags, Some(Field::Struct("Event".to_owned())))
                }

                ApiMember::Function {
                    name,
                    tags,
                    parameters,
                } => (
                    name,
                    tags,
                    Some(Field::Function {
                        // TODO: Roblox doesn't tell us which parameters are nillable or not
                        // So results from these are regularly wrong
                        // The best solution is a manual patch for every method we *know* is nillable
                        // e.g. WaitForChild
                        // We can also let some parameters be required in the middle, and fix unused_variable to accept them

                        // arguments: parameters
                        // .iter()
                        // .map(|param| Argument {
                        // required: if param.default.is_some() {
                        // Required::NotRequired
                        // } else {
                        // Required::Required(None)
                        // },
                        // argument_type: match &param.parameter_type {
                        // ApiValueType::Class { name } => {
                        // ArgumentType::Display(name.to_owned())
                        // }
                        //
                        // ApiValueType::DataType { value } => match value {
                        // ApiDataType::Content => ArgumentType::String,
                        // ApiDataType::Other(other) => {
                        // ArgumentType::Display(other.to_owned())
                        // }
                        // },
                        //
                        // ApiValueType::Group { value } => match value {
                        // ApiGroupType::Table => ArgumentType::Table,
                        // ApiGroupType::Tuple => ArgumentType::Vararg,
                        // ApiGroupType::Variant => ArgumentType::Any,
                        // },
                        //
                        // ApiValueType::Primitive { value } => match value {
                        // ApiPrimitiveType::Bool => ArgumentType::Bool,
                        // ApiPrimitiveType::Double
                        // | ApiPrimitiveType::Float
                        // | ApiPrimitiveType::Int
                        // | ApiPrimitiveType::Int64 => ArgumentType::Number,
                        // ApiPrimitiveType::String => ArgumentType::String,
                        // },
                        //
                        // ApiValueType::Other { name } => {
                        // ArgumentType::Display(name.to_owned())
                        // }
                        // },
                        // })
                        // .collect(),
                        arguments: parameters
                            .iter()
                            .map(|_| Argument {
                                argument_type: ArgumentType::Any,
                                required: Required::NotRequired,
                            })
                            .collect(),
                        method: true,
                    }),
                ),

                ApiMember::Property {
                    name,
                    tags,
                    security,
                    value_type,
                } => (name, tags, {
                    if *security == ApiPropertySecurity::default() {
                        let empty = Vec::new();
                        let tags: &Vec<String> = match tags {
                            Some(tags) => tags,
                            None => &empty,
                        };

                        if let ApiValueType::Class { name } = value_type {
                            self.write_class_struct(api, name);
                            Some(Field::Struct(name.to_owned()))
                        } else {
                            Some(Field::Property {
                                writable: if tags.contains(&"ReadOnly".to_string()) {
                                    None
                                } else {
                                    Some(Writable::Overridden)
                                },
                            })
                        }
                    } else {
                        None
                    }
                }),
            };

            let empty = Vec::new();
            let tags: &Vec<String> = match tags {
                Some(tags) => tags,
                None => &empty,
            };

            if !self.show_deprecated && tags.contains(&"Deprecated".to_owned()) {
                continue;
            }

            if let Some(field) = field {
                table.insert(name.to_owned(), field);
            }
        }

        if class.superclass != "<<<ROOT>>>" {
            self.write_class_members(api, table, &class.superclass);
        }
    }

    fn write_enums(&mut self, api: &api::ApiDump) {
        let mut children = BTreeMap::new();

        for enuhm in &api.enums {
            let mut enum_table = BTreeMap::new();
            enum_table.insert(
                "GetEnumItems".to_owned(),
                Field::Function {
                    arguments: vec![],
                    method: true,
                },
            );

            for item in &enuhm.items {
                enum_table.insert(item.name.to_owned(), Field::Struct("EnumItem".to_owned()));
            }

            children.insert(enuhm.name.to_owned(), Field::Table(enum_table));
        }

        self.std
            .globals
            .insert("Enum".to_owned(), Field::Table(children));
    }

    fn write_instance_new(&mut self, api: &api::ApiDump) {
        let instance_names = api
            .classes
            .iter()
            .filter_map(|class| {
                if !class.tags.contains(&"NotCreatable".to_owned()) {
                    Some(class.name.to_owned())
                } else {
                    None
                }
            })
            .collect();

        let mut instance = self.std.globals.get_mut("Instance").unwrap();

        if let Field::Table(table) = &mut instance {
            *table.get_mut("new").unwrap() = Field::Function {
                arguments: vec![Argument {
                    argument_type: ArgumentType::Constant(instance_names),
                    required: Required::Required(None),
                }],
                method: false,
            };
        } else {
            unreachable!()
        }
    }

    fn write_get_service(&mut self, api: &api::ApiDump) {
        let service_names = api
            .classes
            .iter()
            .filter_map(|class| {
                if class.tags.contains(&"Service".to_owned()) {
                    Some(class.name.to_owned())
                } else {
                    None
                }
            })
            .collect();

        let meta = self.std.meta.as_mut().unwrap();
        let structs = meta.structs.as_mut().unwrap();
        let data_model = structs.get_mut("DataModel").unwrap();

        *data_model.get_mut("GetService").unwrap() = Field::Function {
            arguments: vec![Argument {
                argument_type: ArgumentType::Constant(service_names),
                required: Required::Required(None),
            }],
            method: true,
        };
    }

    fn deprecated_event_methods(&mut self) {
        if !self.show_deprecated {
            return;
        }

        let structs = self.std.meta.as_mut().unwrap().structs.as_mut().unwrap();
        let event_struct = structs.get_mut("Event").unwrap();
        let (connect, wait) = (
            event_struct["Connect"].clone(),
            event_struct["Wait"].clone(),
        );

        event_struct.insert("connect".to_owned(), connect);
        event_struct.insert("wait".to_owned(), wait);
    }
}
