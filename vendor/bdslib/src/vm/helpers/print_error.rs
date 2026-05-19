use easy_error::Error;

use crate::vm::stdlib::debug_fun::debug_display_stack;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::*;
use fancy_regex::Regex;
use rust_multistackvm::multistackvm::VM;
use yansi::Paint;

pub fn parse_msg(msg: String) -> Option<Vec<String>> {
    let mut res: Vec<String> = Vec::new();
    match Regex::new(r"^(?<msg>.*)?\((?<loc>.*)\)$") {
        Ok(re) => match re.captures(&msg) {
            Ok(cap) => match cap {
                Some(groups) => {
                    let msg = match groups.get(1) {
                        Some(msg) => msg.as_str().to_string(),
                        None => return None,
                    };
                    let loc = match groups.get(2) {
                        Some(loc) => loc.as_str().to_string(),
                        None => return None,
                    };
                    res.push(msg);
                    res.push(loc);
                }
                None => {
                    log::error!("Error msg grouping");
                    return None;
                }
            },
            Err(err) => {
                log::error!("Error msg parsing: {}", err);
            }
        },
        Err(err) => {
            log::error!("Error compiling regular exression for msg parsing: {}", err);
            return None;
        }
    }
    Some(res)
}

pub fn print_error_from_str(err: String, nocolor: bool) {
    if nocolor {
        match parse_msg(err.clone()) {
            Some(msg_parsed) => {
                let mut table = Table::new();
                table
                    .load_preset(UTF8_FULL)
                    .apply_modifier(UTF8_ROUND_CORNERS)
                    .set_content_arrangement(ContentArrangement::Dynamic)
                    .add_row(vec![Cell::new("Error"), Cell::new(msg_parsed[0].clone())])
                    .add_row(vec![
                        Cell::new("Location"),
                        Cell::new(msg_parsed[1].clone()),
                    ]);
                println!("{table}");
            }
            None => {
                println!("Error occured:");
                println!("   {}", &err);
            }
        }
    } else {
        match parse_msg(err.clone()) {
            Some(msg_parsed) => {
                let mut table = Table::new();
                table
                    .load_preset(UTF8_FULL)
                    .apply_modifier(UTF8_ROUND_CORNERS)
                    .set_content_arrangement(ContentArrangement::Dynamic)
                    .add_row(vec![
                        Cell::new("Error").fg(Color::Red),
                        Cell::new(msg_parsed[0].clone()).fg(Color::White),
                    ])
                    .add_row(vec![
                        Cell::new("Location").fg(Color::Blue),
                        Cell::new(msg_parsed[1].clone()).fg(Color::Yellow),
                    ]);
                println!("{table}");
            }
            None => {
                println!("{}:", Paint::yellow("Error occured"));
                println!("   {}", Paint::red(&err));
            }
        }
    }
}

pub fn print_error(the_err: Error, nocolor: bool) {
    let err = format!("{}", the_err.ctx);
    print_error_from_str(err, nocolor);
}

pub fn print_error_with_vm(vm: &mut VM, err: Error, nocolor: bool) {
    print_error_from_str_with_vm(vm, format!("{}", err.ctx), nocolor)
}

pub fn print_error_from_str_with_vm(vm: &mut VM, err: String, nocolor: bool) {
    if nocolor {
        match parse_msg(err.clone()) {
            Some(msg_parsed) => {
                let mut table = Table::new();
                table
                    .load_preset(UTF8_FULL)
                    .apply_modifier(UTF8_ROUND_CORNERS)
                    .set_content_arrangement(ContentArrangement::Dynamic)
                    .add_row(vec![Cell::new("Error"), Cell::new(msg_parsed[0].clone())])
                    .add_row(vec![
                        Cell::new("Location"),
                        Cell::new(msg_parsed[1].clone()),
                    ]);
                println!("{table}");
            }
            None => {
                println!("Error occured:");
                println!("   {}", &err);
            }
        }
        println!("[BUND] Content of the stack");
        match debug_display_stack::stdlib_debug_display_stack(vm) {
            Ok(_) => {}
            Err(err) => {
                print_error(err, nocolor);
            }
        }
    } else {
        let bund = format!(
            "{}{}{}{}{}{} ",
            Paint::yellow("["),
            Paint::red("B"),
            Paint::blue("U").bold(),
            Paint::white("N"),
            Paint::cyan("D"),
            Paint::green("]").bold()
        );
        match parse_msg(err.clone()) {
            Some(msg_parsed) => {
                let mut table = Table::new();
                table
                    .load_preset(UTF8_FULL)
                    .apply_modifier(UTF8_ROUND_CORNERS)
                    .set_content_arrangement(ContentArrangement::Dynamic)
                    .add_row(vec![
                        Cell::new("Error").fg(Color::Red),
                        Cell::new(msg_parsed[0].clone()).fg(Color::White),
                    ])
                    .add_row(vec![
                        Cell::new("Location").fg(Color::Blue),
                        Cell::new(msg_parsed[1].clone()).fg(Color::Yellow),
                    ]);
                println!("{table}");
            }
            None => {
                println!("{}:", Paint::yellow("Error occured"));
                println!("   {}", Paint::red(&err));
            }
        }
        println!("{} {}", &bund, Paint::green("Content of the stack"));
        match debug_display_stack::stdlib_debug_display_stack(vm) {
            Ok(_) => {}
            Err(err) => {
                print_error(err, nocolor);
            }
        }
    }
}

pub fn print_error_from_str_plain(err: String) {
    match parse_msg(err.clone()) {
        Some(msg_parsed) => {
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .add_row(vec![Cell::new("Error"), Cell::new(msg_parsed[0].clone())])
                .add_row(vec![
                    Cell::new("Location"),
                    Cell::new(msg_parsed[1].clone()),
                ]);
            println!("{table}");
        }
        None => {
            println!("Error occured:");
            println!("   {}", &err);
        }
    }
}

pub fn format_error_from_str_plain(err: String) -> String {
    match parse_msg(err.clone()) {
        Some(msg_parsed) => {
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .add_row(vec![Cell::new("Error"), Cell::new(msg_parsed[0].clone())])
                .add_row(vec![
                    Cell::new("Location"),
                    Cell::new(msg_parsed[1].clone()),
                ]);
            return format!("{table}");
        }
        None => {
            return format!("{}", &err);
        }
    }
}

pub fn print_error_plain(the_err: Error) {
    let err = format!("{}", the_err.ctx);
    print_error_from_str_plain(err);
}

pub fn format_error_plain(the_err: Error) -> String {
    let err = format!("{}", the_err.ctx);
    format_error_from_str_plain(err)
}
