use std::io;
use std::io::Write;

pub fn write_gnuplot_normal_distribution<S: Write>(
    writer: &mut S,
    xmin: f64,
    xmax: f64,
    mu: f64,
    sigma: f64,
) -> io::Result<()> {
    writeln!(writer, "set key left box")?;
    writeln!(writer, "set autoscale")?;
    writeln!(writer, "set samples 800")?;

    writeln!(writer, "set xrange [{}:{}]", xmin, xmax)?;
    writeln!(writer, "set ytics nomirror")?;
    writeln!(writer, "set autoscale y")?;

    writeln!(writer, "invsqrt2pi = 0.398942280401433")?;
    writeln!(writer, "normal(x,mu,sigma) = sigma<=0 ? 1/0 : invsqrt2pi / sigma * exp(-0.5 * ((x - mu) / sigma) ** 2)")?;
    writeln!(
        writer,
        "plot normal(x,{},{}) with lines lw 2 notitle",
        mu, sigma
    )?;

    Ok(())
}
