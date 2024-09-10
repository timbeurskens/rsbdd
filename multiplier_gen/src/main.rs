use std::{fmt::Display, num::NonZeroUsize};

use clap::Parser;
use itertools::Itertools;

#[derive(Debug, Parser)]
struct Args {
    #[clap(long, short, default_value = "8")]
    /// the width of of the multiplier
    bits: NonZeroUsize,

    value_a: Option<usize>,
    value_b: Option<usize>,
    value_out: Option<usize>,
}

fn to_bits(value: usize, bits: usize) -> Vec<bool> {
    (0..bits).map(|bit| (value & (1 << bit)) != 0).collect()
}

fn produce_port_value(port_prefix: &str, value: impl IntoIterator<Item = bool>) {
    value.into_iter().enumerate().for_each(|(i, v)| {
        let opt_negate = if v { " " } else { "-" };
        println!("{opt_negate}{port_prefix}{i} &");
    })
}

#[derive(Clone, Debug)]
struct AdderOutput {
    carry_out: String,
    sum: String,
}

fn produce_half_adder(bit_a: impl Display, bit_b: impl Display) -> AdderOutput {
    AdderOutput {
        carry_out: format!("({bit_a} & {bit_b})"),
        sum: format!("({bit_a} xor {bit_b})"),
    }
}

fn produce_full_adder(
    bit_a: impl Display,
    bit_b: impl Display,
    carry_in: impl Display,
) -> AdderOutput {
    let half_adder_1 = produce_half_adder(bit_a, bit_b);
    let half_adder_2 = produce_half_adder(half_adder_1.sum, carry_in);

    AdderOutput {
        carry_out: format!("({} | {})", half_adder_1.carry_out, half_adder_2.carry_out),
        sum: half_adder_2.sum,
    }
}

fn produce_adder_array(
    bits_a: impl IntoIterator<Item = impl Display>,
    bits_b: impl IntoIterator<Item = impl Display>,
) -> Vec<AdderOutput> {
    let mut last_bit: Option<AdderOutput> = None;

    bits_a
        .into_iter()
        .zip(bits_b.into_iter())
        .map(|(a, b)| {
            let carry_in = if let Some(last_bit) = last_bit.as_ref() {
                &last_bit.carry_out
            } else {
                "false"
            };

            let adder = produce_full_adder(a, b, carry_in);

            last_bit = Some(adder.clone());

            adder
        })
        .collect()
}

fn main() {
    let args = Args::parse();

    let output_width = args.bits.get() * 2 - 1;
    let input_width = args.bits.get();

    let mut outputs:  Vec<AdderOutput> = Vec::new();
    let mut last_adder: Vec<AdderOutput> = Vec::new();

    for (i, j) in (0..input_width).tuple_windows() {
        let port_a: Vec<String> = if i == 0 {
            (0..input_width).map(|bit| format!("(port_a_{bit} & port_b_{i})")).collect()
        } else {
            let carry_out = last_adder.last().unwrap().carry_out.clone();
            last_adder.into_iter().skip(1).map(|adder| {
                adder.sum
            }).chain([carry_out]).collect()
        };
        
        let port_b = (0..input_width).map(|bit| format!("(port_a_{bit} & port_b_{j})"));
        
        let adder = produce_adder_array(port_a, port_b);
        last_adder = adder.clone();

        outputs.push(adder.first().unwrap().clone())
    }

    outputs.extend(last_adder.into_iter().skip(1));

    for (i, adder) in outputs.iter().enumerate() {
        println!("(port_out_{i} <=> {}) &", adder.sum);
    }

    if let Some(last_output) = outputs.last() {
        println!("(port_out_{} <=> {}) &", outputs.len(), last_output.carry_out);
    }

    // assign output values
    args.value_a
        .inspect(|value| produce_port_value("port_a_", to_bits(*value, input_width)));
    args.value_b
        .inspect(|value| produce_port_value("port_b_", to_bits(*value, input_width)));
    args.value_out
        .inspect(|value| produce_port_value("port_out_", to_bits(*value, output_width)));

    println!("true");
}
