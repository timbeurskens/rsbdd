use std::{fmt::Display, num::NonZeroUsize};

use clap::Parser;
use itertools::Itertools;

// todo: implement selftest (prove multiplication is implemented correctly):
// forall a, b: 
//   a * b == b * a (commutativity)
//   x * 1 == x (identity)
//   x * 0 == 0 (zero)
//   x * (y + z) == (x * y) + (x * z) (distributivity)
//   (x * y) * z == x * (y * z) (associativity)
// or:
//   x * 0 == 0
//   x * (y + 1) == (x * y) + x

// todo: implement primality check:
// forall port_a, port_b: if multiplication holds, either port_a == 1 or port_b == 1

// bug: multiplication is not commutative if #bits is even


#[derive(Debug, Parser)]
struct Args {
    #[clap(long, short, default_value = "8")]
    /// the width of of the multiplier
    bits: NonZeroUsize,

    #[clap(long)]
    a: Option<usize>,
    #[clap(long)]
    b: Option<usize>,
    #[clap(long)]
    out: Option<usize>,

    #[clap(long)]
    hide_known: bool,

    #[clap(long)]
    is_prime: bool,
}

fn check_bits(value: usize, bits: usize) -> bool {
    let bits_required = f64::log2(value as f64).floor() as usize + 1;

    if bits_required > bits {
        eprintln!(
            "{bits} bits is not enough to represent {value}, needs at least {bits_required} bits"
        );
        false
    } else {
        true
    }
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

fn produce_multiplier(bits_a: impl ExactSizeIterator<Item = impl Display>, bits_b: impl ExactSizeIterator<Item = impl Display>) -> Vec<AdderOutput> {
    assert_eq!(bits_a.len(), bits_b.len(), "input sizes should match");

    let input_width = bits_a.len();
    let output_width = input_width * 2;

    todo!()
}

fn main() {
    let args = Args::parse();

    // increase the bit-width of the adders by one, to accomodate for carry-out in the next addition.

    let output_width = args.bits.get() * 2;
    let input_width = args.bits.get();

    let mut outputs: Vec<AdderOutput> = Vec::new();
    let mut last_adder: Vec<AdderOutput> = Vec::new();
    let mut assignments: Vec<String> = Vec::new();

    for (i, j) in (0..input_width).tuple_windows() {
        let port_a: Vec<String> = if i == 0 {
            (0..input_width)
                .map(|bit| format!("(port_a_{bit} & port_b_{i})"))
                .chain(["false".to_string()])
                .collect()
        } else {
            let carry_out = last_adder.last().unwrap().carry_out.clone();
            assignments.extend(
                last_adder
                    .into_iter()
                    .skip(1)
                    .map(|adder| adder.sum)
                    .chain([carry_out])
                    .enumerate()
                    .map(|(bit, value)| format!("(stage_{i}_{bit} <=> {value})")),
            );

            (0..input_width)
                .map(|bit| format!("stage_{i}_{bit}"))
                .collect()
        };

        let port_b = ["false".to_string()]
            .into_iter()
            .chain((0..input_width).map(|bit| format!("(port_a_{bit} & port_b_{j})")));

        let adder = produce_adder_array(port_a, port_b);
        last_adder = adder.clone();

        outputs.push(adder.first().unwrap().clone())
    }

    outputs.extend(last_adder.into_iter().skip(1));

    let mut known_ports = Vec::new();

    if let Some(a) = args.a {
        if !check_bits(a, input_width) {
            return;
        }
        known_ports.extend((0..input_width).map(|bit| format!("port_a_{bit}")));
    }

    if let Some(b) = args.b {
        if !check_bits(b, input_width) {
            return;
        }
        known_ports.extend((0..input_width).map(|bit| format!("port_b_{bit}")));
    }

    if let Some(out) = args.out {
        if !check_bits(out, output_width) {
            return;
        }
        known_ports.extend((0..output_width).map(|bit| format!("port_out_{bit}")));
    }

    known_ports.extend(
        (0..input_width - 1)
            .flat_map(|stage| (0..input_width + 1).map(move |bit| format!("stage_{stage}_{bit}"))),
    );

    println!("any {} #", known_ports.join(","));

    for assignment in assignments {
        println!("{} &", assignment);
    }

    for (i, adder) in outputs.iter().enumerate() {
        println!("(port_out_{i} <=> {}) &", adder.sum);
    }

    if let Some(last_output) = outputs.last() {
        println!(
            "(port_out_{} <=> {}) &",
            outputs.len(),
            last_output.carry_out
        );
    }

    // assign output values
    args.a
        .inspect(|value| produce_port_value("port_a_", to_bits(*value, input_width)));
    args.b
        .inspect(|value| produce_port_value("port_b_", to_bits(*value, input_width)));
    args.out
        .inspect(|value| produce_port_value("port_out_", to_bits(*value, output_width)));

    println!("true");
}
