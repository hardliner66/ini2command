extern crate ini;
use ini::Ini;

use clap::{App, AppSettings, Arg};
use encoding_rs::*;
use std::{
    io::{Read, Write},
    process::Command,
};
use string_error::{into_err, static_err};

struct Args {
    ini: String,
    section: Option<String>,
    property: String,
    command: String,
    search_string: String,
    dry: bool,
}

const DEFAULT_SEARCH_STRING: &str = "{}";

fn get_args() -> Args {
    let matches = App::new("ini2command")
        .setting(AppSettings::TrailingVarArg)
        .version("1.0")
        .about("Create a command from a value of an INI-file and execute it.")
        .arg(
            Arg::with_name("ini")
                .short("i")
                .long("ini")
                .value_name("FILE")
                .help("The path to the ini file")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("section")
                .short("s")
                .long("section")
                .value_name("NAME")
                .help("The section to use")
                .takes_value(true)
                .required(false),
        )
        .arg(
            Arg::with_name("property")
                .short("p")
                .long("property")
                .value_name("NAME")
                .help("The property to use")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("dry")
                .short("d")
                .long("dry")
                .help("Print the command instead of executing it.")
                .required(false),
        )
        .arg(
            Arg::with_name("search_string")
                .short("r")
                .long("search_string")
                .value_name("STRING")
                .help("The search string for use in the template")
                .default_value(DEFAULT_SEARCH_STRING)
                .takes_value(true)
                .required(false),
        )
        .arg(
            Arg::with_name("command")
                .value_name("COMMAND")
                .help("The command template")
                .required(true)
                .multiple(true)
                .last(true),
        )
        .get_matches();

    Args {
        ini: matches.value_of("ini").unwrap().to_string(),
        section: matches.value_of("section").map(|s| s.to_string()),
        property: matches.value_of("property").unwrap().to_string(),
        command: matches
            .values_of("command")
            .unwrap()
            .collect::<Vec<_>>()
            .join(" "),
        dry: matches.is_present("dry"),
        search_string: matches
            .value_of("search_string")
            .unwrap_or(DEFAULT_SEARCH_STRING)
            .to_string(),
    }
}

fn convert(
    decoder: &mut Decoder,
    encoder: &mut Encoder,
    read: &mut dyn Read,
    write: &mut dyn Write,
    last: bool,
) {
    let mut input_buffer = [0u8; 2048];
    let mut intermediate_buffer = [0u16; 2048];
    let mut output_buffer = [0u8; 4096];
    let mut current_input_ended = false;
    while !current_input_ended {
        match read.read(&mut input_buffer) {
            Err(_) => {
                print!("Error reading input.");
                std::process::exit(-5);
            }
            Ok(decoder_input_end) => {
                current_input_ended = decoder_input_end == 0;
                let input_ended = last && current_input_ended;
                let mut decoder_input_start = 0usize;
                loop {
                    let (decoder_result, decoder_read, decoder_written, _) = decoder
                        .decode_to_utf16(
                            &input_buffer[decoder_input_start..decoder_input_end],
                            &mut intermediate_buffer,
                            input_ended,
                        );
                    decoder_input_start += decoder_read;

                    let last_output = if input_ended {
                        match decoder_result {
                            CoderResult::InputEmpty => true,
                            CoderResult::OutputFull => false,
                        }
                    } else {
                        false
                    };

                    // Regardless of whether the intermediate buffer got full
                    // or the input buffer was exhausted, let's process what's
                    // in the intermediate buffer.

                    let mut encoder_input_start = 0usize;
                    loop {
                        let (encoder_result, encoder_read, encoder_written, _) = encoder
                            .encode_from_utf16(
                                &intermediate_buffer[encoder_input_start..decoder_written],
                                &mut output_buffer,
                                last_output,
                            );
                        encoder_input_start += encoder_read;
                        match write.write_all(&output_buffer[..encoder_written]) {
                            Err(_) => {
                                print!("Error writing output.");
                                std::process::exit(-6);
                            }
                            Ok(_) => {}
                        }
                        match encoder_result {
                            CoderResult::InputEmpty => {
                                break;
                            }
                            CoderResult::OutputFull => {
                                continue;
                            }
                        }
                    }

                    // Now let's see if we should read again or process the
                    // rest of the current input buffer.
                    match decoder_result {
                        CoderResult::InputEmpty => {
                            break;
                        }
                        CoderResult::OutputFull => {
                            continue;
                        }
                    }
                }
            }
        }
    }
}

fn get_encoding(cp: u32) -> Option<&'static Encoding> {
    match cp as u16 {
        850 => Some(IBM866),
        cp => codepage::to_encoding(cp),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = get_args();

    let conf = Ini::load_from_file(&args.ini)?;

    let section = conf.section(args.section.clone()).ok_or(into_err(format!(
        "Section \"{}\" not found!",
        args.section.unwrap_or_default()
    )))?;

    let value = section.get(&args.property).ok_or(into_err(format!(
        "Property \"{}\" not found!",
        args.property
    )))?;

    let command = args.command.replace(&args.search_string, value);

    if args.dry {
        println!("{}", command);
    } else {
        let mut parts = command.split(" ");

        let command = parts.nth(0).ok_or(static_err("Template string is empty"))?;

        match command {
            "echo" => println!("{}", parts.collect::<Vec<_>>().join(" ")),
            _ => {
                let output = Command::new(command).args(parts).output()?;

                if cfg!(feature = "unstable_try_decode") && cfg!(target_family = "windows") {
                    let in_cp = unsafe {
                        // winapi::um::consoleapi::GetConsoleOutputCP()
                        winapi::um::winnls::GetACP()
                    };

                    let input_encoding = get_encoding(in_cp).unwrap();
                    let mut decoder = input_encoding.new_decoder();

                    let output_encoding = UTF_8;
                    let mut encoder = output_encoding.new_encoder();

                    if output.stdout.len() > 0 {
                        let mut outp = std::io::Cursor::new(output.stdout);
                        convert(
                            &mut decoder,
                            &mut encoder,
                            &mut outp,
                            &mut std::io::stdout(),
                            false,
                        );
                    }

                    if output.stderr.len() > 0 {
                        let mut inp = std::io::Cursor::new(output.stderr);
                        convert(
                            &mut decoder,
                            &mut encoder,
                            &mut inp,
                            &mut std::io::stderr(),
                            true,
                        );
                    }
                } else {
                    if output.stdout.len() > 0 {
                        println!("{}", String::from_utf8_lossy(&output.stdout));
                    }
                    if output.stderr.len() > 0 {
                        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                    }
                }
            }
        }
    }

    Ok(())
}
