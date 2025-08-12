// Copyright 2018-2020 ETH Zurich
// Andreas Kurth <akurth@iis.ee.ethz.ch>
// Gianluca Bellocchi <gianluca.bellocchi@unimore.it>
//
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

use std::fs::File;
use std::io::{BufRead, BufReader, Result};
use std::io::prelude::*;
use std::vec::Vec;
use std::collections::HashMap;

#[macro_use]
extern crate clap;
use clap::{Arg, App};

extern crate regex;
use regex::Regex;

use strfmt::Format;

/**********************************************************************************************
*  mem_from_file
*  ------------------------------------------------------------------------------------------
*  Reads an SLM file and returns a memory map (address -> data word).
*  Optionally swaps endianness for each 32-bit word.
**********************************************************************************************/
fn mem_from_file(path: &str, swap_endianness: bool) -> Result<HashMap<usize, String>> {
    let file = File::open(path)?;
    let mut mem = HashMap::new();
    for line in BufReader::new(file).lines() {
        let l = line.unwrap();
        // Split line into address and data word
        let v = l.split(' ').collect::<Vec<&str>>();
        let data_word = v[1].trim_start_matches("0x").to_string();
        // Parse address, supporting both @index and 0xaddress formats
        let (addr, indexed) = if v[0].chars().nth(0) == Some('@') {
            let idx_str = &v[0][1..];
            let idx = usize::from_str_radix(idx_str, 16).unwrap();
            (idx * 4, true)
        } else {
            let addr_str = &v[0].trim_start_matches("0x");
            (usize::from_str_radix(addr_str, 16).unwrap(), false)
        };
        let key_str = || if indexed {
            format!("index @{:x}", addr/4)
        } else {
            format!("address 0x{:x}", addr)
        };
        // Ensure data word is 8 hex digits (32 bits)
        assert_eq!(data_word.len(), 8, "incorrect word length for {} of file {}", key_str(), path);
        // Optionally swap endianness
        let data = if swap_endianness {
            // TODO: faster and less copies?
            // Swap bytes in the 32-bit word
            let inp: Vec<char> = data_word.chars().collect();
            let mut oup = String::with_capacity(8);
            for i in 0..4 {
                oup.push(inp[6-2*i]);
                oup.push(inp[7-2*i]);
            };
            oup
        } else {
            data_word
        };
        // Assert that the SLM line does not overwrite an existing entry.
        assert_eq!(mem.insert(addr, data), None, "duplicate entry for {} of file {}", key_str(), path);
    }
    Ok(mem)
}

/**********************************************************************************************
*  print_help_log
*  ------------------------------------------------------------------------------------------
*  Prints a detailed help log for the command-line parameters and usage examples.
**********************************************************************************************/
fn print_help_log() {
    println!("SLM Converter - Parameter Help");
    println!("----------------------------------------");
    println!("--num-oup-rows, -n <N>      : Number of rows in each output SLM file (required)");
    println!("--start, -s <ADDR>          : First address; hexadecimal with or without 0x prefix (required)");
    println!("--word-width, -w <BITS>     : Number of bits per memory word; must be a multiple of 32 (required)");
    println!("--serial-banks, -S <N>      : Number of memory banks in series (default: 1)");
    println!("--parallel-banks, -P <N>    : Number of memory banks in parallel (default: 1)");
    println!("--file, -f <FILE>           : Input SLM file with 32-bit words; if omitted, memory is initialized to zero");
    println!("--format, -F <STR>          : Output filename format string. Use %S and %P for serial and parallel index (default: %S_%P.slm)");
    println!("--swap-endianness           : Swap endianness for every 32-bit data word");
    println!("--help-log                  : Show this detailed parameter help log");
    println!("----------------------------------------");
    println!("Example:");
    println!("  slm_conv --swap-endianness -f input.slm -w 32 -P 4 -S 8 -n 1024 -s 0x1c000000 -F l2_%01S_%01P.slm");
    println!();
}

/**********************************************************************************************
*  preview_output_files
*  ------------------------------------------------------------------------------------------
*  Prints a preview of the output files and the number of memory lines each will contain,
*  based on the current configuration.
**********************************************************************************************/
fn preview_output_files(
    n_serial: usize,
    n_parallel: usize,
    n_rows: usize,
    format: &str,
) {
    use std::collections::HashMap;
    use strfmt::strfmt;

    println!("Preview of output files:");
    println!("------------------------");
    for i_ser in 0..n_serial {
        for i_par in 0..n_parallel {
            let mut vars = HashMap::new();
            vars.insert("S".to_string(), i_ser);
            vars.insert("P".to_string(), i_par);
            let filename = strfmt(format, &vars).unwrap_or_else(|_| "<format error>".to_string());
            println!("File: {}  |  Memory lines: {}", filename, n_rows);
        }
    }
    println!("------------------------");
    println!(
        "Total files: {} ({} serial × {} parallel), each with {} memory lines.",
        n_serial * n_parallel,
        n_serial,
        n_parallel,
        n_rows
    );
    println!();
}

/**********************************************************************************************
*  main
*  ------------------------------------------------------------------------------------------
*  Main entry point: parses arguments, reads input, and writes output SLM files or prints help.
**********************************************************************************************/
fn main() -> Result<()> {
    // Set up command-line argument parsing
    let matches = App::new("SLM Converter")
        .version(crate_version!())
        .author(crate_authors!(", "))
        .about("Converts SLM files")
        .arg(Arg::with_name("n_rows")
            .short("n")
            .long("num-oup-rows")
            .help("Number of rows in each output SLM file")
            .takes_value(true)
            .required(true)
        )
        .arg(Arg::with_name("start_addr")
            .short("s")
            .long("start")
            .help("First address; hexadecimal with or without 0x prefix")
            .takes_value(true)
            .required(true)
        )
        .arg(Arg::with_name("word_width")
            .short("w")
            .long("word-width")
            .help("Number of bits per memory word; must be a multiple of 32")
            .takes_value(true)
            .required(true)
        )
        .arg(Arg::with_name("serial_banks")
            .short("S")
            .long("serial-banks")
            .help("Number of memory banks in series")
            .takes_value(true)
            .default_value("1")
        )
        .arg(Arg::with_name("parallel_banks")
            .short("P")
            .long("parallel-banks")
            .help("Number of memory banks in parallel")
            .takes_value(true)
            .default_value("1")
        )
        .arg(Arg::with_name("input_file")
            .short("f")
            .long("file")
            .help("Input SLM file with 32-bit words; if omitted, the memory is initialized to zero")
            // TODO: Add support for SLM files with different word width.
            // TODO: Add support for input ELF files.
            .takes_value(true)
        )
        .arg(Arg::with_name("format")
            .short("F")
            .long("format")
            .help("Format string.  %S and %P for serial and parallel index, respectively.  Put `0' \
                followed by a number between `%' and `S' or `P' for zero-padding to that number of \
                digits, e.g., `%02S_%02P.slm'.")
            .takes_value(true)
            .default_value("%S_%P.slm")
        )
        .arg(Arg::with_name("swap_endianness")
             .long("swap-endianness")
             .help("Swap endianness for every 32-bit data word")
        )
        .arg(Arg::with_name("help")
            .long("help")
            .help("Describe parameters and provide examples of usage.")
        )
        .arg(Arg::with_name("preview")
            .long("preview")
            .help("Preview the names of output files and the number of memory lines each will contain.")
        )
        .get_matches();

    // Parse swap endianness flag and input file
    let swap_endianness = matches.is_present("swap_endianness");
    let mem = match matches.value_of("input_file") {
        Some(path) => mem_from_file(path, swap_endianness),
        None => Ok(HashMap::new()),
    }?;

    // Helper closures for argument parsing
    let arg_usize = |arg: &str| -> usize {
        matches
            .value_of(arg).expect(&format!("Expected value for argument {}!", arg))
            .parse::<usize>().expect(&format!("Expected unsigned integer for argument {}!", arg))
    };
    let arg_addr = |arg: &str| -> usize {
        usize::from_str_radix(matches
            .value_of(arg).expect(&format!("Expected value for argument {}!", arg))
            .trim_start_matches("0x"), 16).expect(&format!("Expected hexadecimal number for argument {}!", arg))
    };

    // Extract parameters from arguments
    let n_rows = arg_usize("n_rows");
    let n_serial = arg_usize("serial_banks");
    let n_parallel = arg_usize("parallel_banks");
    let start_addr = arg_addr("start_addr");
    let word_width = arg_usize("word_width");
    assert!(word_width % 32 == 0);
    let word_bytes = word_width / 8;
    let words_per_line = word_bytes / 4;
    let words_in_parallel = words_per_line * n_parallel;

    // Prepare output filename format string
    let format = {
        let escaped = matches.value_of("format").unwrap().replace("{}", "{{}}");
        let re = Regex::new(r"%(?P<n>0\d+)?(?P<f>[SP])").unwrap();
        re.replace_all(&escaped, "{$f:$n}").into_owned()
    };

    // Helper closure to get memory value for a given index
    let mem_val = |idx: usize| {
        let addr = start_addr + idx * 4;
        match mem.get(&addr) {
            Some(s) => s.as_str(),
            None => "00000000"
        }
    };

    // Print help log and exit if requested
    if matches.is_present("help") {
        print_help_log();
        return Ok(());
    }

    // Print preview of output files and exit if requested
    if matches.is_present("preview") {
        preview_output_files(n_serial, n_parallel, n_rows, &format);
        return Ok(());
    }

    // Main output loop: generate SLM files for each bank
    for i_ser in 0..n_serial {
        for i_par in 0..n_parallel {
            let mut vars = HashMap::new();
            vars.insert("S".to_string(), i_ser);
            vars.insert("P".to_string(), i_par);
            let mut file = File::create(format.format(&vars).unwrap()).unwrap();
            for i_word in 0..n_rows {
                // Calculate memory index for this word
                let idx = i_par * words_per_line + words_in_parallel * i_word
                            + words_in_parallel * n_rows * i_ser;
                write!(file, "@{:08X} ", i_word).unwrap();
                // Write words for this line in reverse order
                for i_sw in (0..words_per_line).rev() {
                    write!(file, "{}", mem_val(idx+i_sw)).unwrap();
                }
                write!(file, "\n").unwrap();
            }
        }
    }
    Ok(())
}