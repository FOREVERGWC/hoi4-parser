use std::time::{Duration, Instant};

use crate::{Hoi4ParserError, generate, parse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchReport {
    pub iterations: usize,
    pub parse_generate_total: Duration,
    pub avg_per_iteration: Duration,
    pub input_bytes: usize,
}

pub fn benchmark_round_trip(input: &str, iterations: usize) -> Result<BenchReport, Hoi4ParserError> {
    if iterations == 0 {
        return Err(Hoi4ParserError::Generate {
            message: "迭代次数必须大于 0".to_string(),
        });
    }

    let start = Instant::now();
    for _ in 0..iterations {
        let doc = parse(input)?;
        let _ = generate(&doc)?;
    }
    let total = start.elapsed();

    Ok(BenchReport {
        iterations,
        parse_generate_total: total,
        avg_per_iteration: total / iterations as u32,
        input_bytes: input.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::benchmark_round_trip;

    #[test]
    fn benchmark_api_should_return_report() {
        let input = "country = { name = \"China\" name = \"PRC\" effect = \"set_var = { key = \\\"x\\\" value = 1 }\" }";
        let report = benchmark_round_trip(input, 200).expect("benchmark should succeed");
        assert_eq!(report.iterations, 200);
        assert!(report.input_bytes > 0);
        assert!(report.parse_generate_total >= report.avg_per_iteration);
    }
}
