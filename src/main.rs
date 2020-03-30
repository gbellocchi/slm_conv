// Copyright 2018-2020 ETH Zurich
// Andreas Kurth <akurth@iis.ee.ethz.ch>
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

fn mem_from_file(path: &str, swap_endianness: bool) -> Result<HashMap<usize, String>> {
    let file = File::open(path)?;
    let mut mem = HashMap::new();
    for line in BufReader::new(file).lines() {
        let l = line.unwrap();
        let v = l.split(' ').collect::<Vec<&str>>();
        let data_word = v[1].trim_start_matches("0x").to_string();
        let addr = if v[0].chars().nth(0) == Some('@') {
            let idx_str = &v[0][1..];
            let idx = usize::from_str_radix(idx_str, 16).unwrap();
            assert_eq!(data_word.len(), 8, "incorrect word length at index {} of file {}", idx, path);
            idx * 4
        } else {
            let addr_str = &v[0].trim_start_matches("0x");
            assert_eq!(data_word.len(), 8, "incorrect word length at address {} of file {}", addr_str, path);
            usize::from_str_radix(addr_str, 16).unwrap()
        };
        let data = if swap_endianness {
            // TODO: faster and less copies?
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
        assert_eq!(mem.insert(addr, data), None, "duplicate key for address {:x} of file {}", addr, path);
    }
    Ok(mem)
}

fn main() -> Result<()> {
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
        .get_matches();

    let swap_endianness = matches.is_present("swap_endianness");
    let mem = match matches.value_of("input_file") {
        Some(path) => mem_from_file(path, swap_endianness),
        None => Ok(HashMap::new()),
    }?;

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

    let n_rows = arg_usize("n_rows");
    let n_serial = arg_usize("serial_banks");
    let n_parallel = arg_usize("parallel_banks");
    let start_addr = arg_addr("start_addr");
    let word_width = arg_usize("word_width");
    assert!(word_width % 32 == 0);
    let word_bytes = word_width / 8;
    let words_per_line = word_bytes / 4;
    let words_in_parallel = words_per_line * n_parallel;
    let format = {
        let escaped = matches.value_of("format").unwrap().replace("{}", "{{}}");
        let re = Regex::new(r"%(?P<n>0\d+)?(?P<f>[SP])").unwrap();
        re.replace_all(&escaped, "{$f:$n}").into_owned()
    };

    let mem_val = |idx: usize| {
        let addr = start_addr + idx * 4;
        match mem.get(&addr) {
            Some(s) => s.as_str(),
            None => "00000000"
        }
    };

    for i_ser in 0..n_serial {
        for i_par in 0..n_parallel {
            let mut vars = HashMap::new();
            vars.insert("S".to_string(), i_ser);
            vars.insert("P".to_string(), i_par);
            let mut file = File::create(format.format(&vars).unwrap()).unwrap();
            for i_word in 0..n_rows {
                let idx = i_par * words_per_line + words_in_parallel * i_word
                            + words_in_parallel * n_rows * i_ser;
                write!(file, "@{:08X} ", i_word).unwrap();
                for i_sw in (0..words_per_line).rev() {
                    write!(file, "{}", mem_val(idx+i_sw)).unwrap();
                }
                write!(file, "\n").unwrap();
            }
        }
    }
    Ok(())
}
