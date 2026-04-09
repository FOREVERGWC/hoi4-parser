use std::time::{Duration, Instant};

use crate::{compat, generator, parser, tokenizer, Document, Hoi4ParserError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchReport {
    pub iterations: usize,
    pub parse_generate_total: Duration,
    pub avg_per_iteration: Duration,
    pub tokenize_total: Duration,
    pub parse_total: Duration,
    pub generate_total: Duration,
    pub post_process_total: Duration,
    pub input_bytes: usize,
}

pub fn benchmark_round_trip(
    input: &str,
    iterations: usize,
) -> Result<BenchReport, Hoi4ParserError> {
    if iterations == 0 {
        return Err(Hoi4ParserError::Generate {
            message: "迭代次数必须大于 0".to_string(),
        });
    }

    let start = Instant::now();
    let mut tokenize_total = Duration::ZERO;
    let mut parse_total = Duration::ZERO;
    let mut generate_total = Duration::ZERO;
    let mut post_process_total = Duration::ZERO;

    for _ in 0..iterations {
        let t0 = Instant::now();
        let tokens = tokenizer::tokenize(input)?;
        tokenize_total += t0.elapsed();

        let t1 = Instant::now();
        let root = parser::parse_root(&tokens)?;
        parse_total += t1.elapsed();

        let doc = Document::new(root, input);
        let t2 = Instant::now();
        let rendered = generator::generate_document(&doc)?;
        generate_total += t2.elapsed();

        let t3 = Instant::now();
        let _ = compat::restore_compat_operators(&rendered);
        post_process_total += t3.elapsed();
    }
    let total = start.elapsed();

    Ok(BenchReport {
        iterations,
        parse_generate_total: total,
        avg_per_iteration: total / iterations as u32,
        tokenize_total,
        parse_total,
        generate_total,
        post_process_total,
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
        assert!(report.parse_generate_total >= report.tokenize_total);
        assert!(report.parse_generate_total >= report.parse_total);
        assert!(report.parse_generate_total >= report.generate_total);
        assert!(report.parse_generate_total >= report.post_process_total);
    }
}
