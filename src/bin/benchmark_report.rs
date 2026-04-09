use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use hoi4_parser::benchmark_round_trip;

const DEFAULT_INPUT: &str = "tests/fixtures/multiline_condition_block.txt";
const DEFAULT_ITERATIONS: usize = 300;
const DEFAULT_WARMUP_ROUNDS: usize = 2;
const DEFAULT_MEASURE_ROUNDS: usize = 8;

fn main() {
    if let Err(err) = run() {
        eprintln!("benchmark 失败: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let input_path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_INPUT));
    let iterations = args
        .get(2)
        .map(|s| {
            s.parse::<usize>()
                .map_err(|e| format!("迭代次数解析失败: {e}"))
        })
        .transpose()?
        .unwrap_or(DEFAULT_ITERATIONS);
    let measure_rounds = args
        .get(3)
        .map(|s| {
            s.parse::<usize>()
                .map_err(|e| format!("测量轮数解析失败: {e}"))
        })
        .transpose()?
        .unwrap_or(DEFAULT_MEASURE_ROUNDS);
    let warmup_rounds = args
        .get(4)
        .map(|s| {
            s.parse::<usize>()
                .map_err(|e| format!("预热轮数解析失败: {e}"))
        })
        .transpose()?
        .unwrap_or(DEFAULT_WARMUP_ROUNDS);

    if !input_path.exists() {
        return Err(format!("输入文件不存在: {}", input_path.display()));
    }
    if iterations == 0 {
        return Err("迭代次数必须大于 0".to_string());
    }
    if measure_rounds == 0 {
        return Err("测量轮数必须大于 0".to_string());
    }

    let input = fs::read_to_string(&input_path)
        .map_err(|e| format!("读取文件失败 {}: {e}", input_path.display()))?;
    for _ in 0..warmup_rounds {
        benchmark_round_trip(&input, iterations).map_err(|e| format!("warmup 失败: {e}"))?;
    }

    let mut reports = Vec::with_capacity(measure_rounds);
    for _ in 0..measure_rounds {
        let report = benchmark_round_trip(&input, iterations)
            .map_err(|e| format!("benchmark 执行失败: {e}"))?;
        reports.push(report);
    }

    let mut total_sum = Duration::ZERO;
    let mut tokenize_sum = Duration::ZERO;
    let mut parse_sum = Duration::ZERO;
    let mut generate_sum = Duration::ZERO;
    let mut post_sum = Duration::ZERO;
    let mut total_samples = Vec::with_capacity(measure_rounds);
    let mut per_iter_samples = Vec::with_capacity(measure_rounds);

    for report in &reports {
        total_sum += report.parse_generate_total;
        tokenize_sum += report.tokenize_total;
        parse_sum += report.parse_total;
        generate_sum += report.generate_total;
        post_sum += report.post_process_total;
        total_samples.push(report.parse_generate_total);
        per_iter_samples.push(report.avg_per_iteration);
    }

    let round_count = measure_rounds as u32;
    let avg_total = total_sum / round_count;
    let avg_tokenize = tokenize_sum / round_count;
    let avg_parse = parse_sum / round_count;
    let avg_generate = generate_sum / round_count;
    let avg_post = post_sum / round_count;
    let avg_per_iter = avg_total / iterations as u32;

    println!("Benchmark 完成");
    println!("文件: {}", input_path.display());
    println!("输入大小: {} bytes", reports[0].input_bytes);
    println!("迭代/轮: {}", iterations);
    println!("预热轮数: {}", warmup_rounds);
    println!("测量轮数: {}", measure_rounds);
    println!(
        "平均总耗时: {} | 平均每次迭代: {}",
        fmt_duration(avg_total),
        fmt_duration(avg_per_iter)
    );
    println!("\n分段耗时(按测量轮平均):");
    print_stage("tokenize", avg_tokenize, avg_total);
    print_stage("parse", avg_parse, avg_total);
    print_stage("generate", avg_generate, avg_total);
    print_stage("post_process", avg_post, avg_total);
    println!("\n分布统计:");
    print_distribution("总耗时/轮", &mut total_samples);
    print_distribution("单次迭代", &mut per_iter_samples);

    Ok(())
}

fn print_stage(name: &str, stage: Duration, total: Duration) {
    let pct = if total.is_zero() {
        0.0
    } else {
        (stage.as_secs_f64() / total.as_secs_f64()) * 100.0
    };
    println!(" - {name:<12} {} ({pct:.2}%)", fmt_duration(stage));
}

fn print_distribution(label: &str, samples: &mut [Duration]) {
    samples.sort_unstable();
    let min = samples.first().copied().unwrap_or(Duration::ZERO);
    let median = percentile(samples, 50.0);
    let p95 = percentile(samples, 95.0);
    let max = samples.last().copied().unwrap_or(Duration::ZERO);
    println!(
        " - {label:<12} min={} median={} p95={} max={}",
        fmt_duration(min),
        fmt_duration(median),
        fmt_duration(p95),
        fmt_duration(max)
    );
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let n = sorted.len();
    let rank = ((p / 100.0) * (n as f64 - 1.0)).round() as usize;
    sorted[rank.min(n - 1)]
}

fn fmt_duration(d: Duration) -> String {
    if d.as_secs() >= 1 {
        format!("{:.3}s", d.as_secs_f64())
    } else if d.as_millis() >= 1 {
        format!("{:.3}ms", d.as_secs_f64() * 1_000.0)
    } else {
        format!("{:.3}us", d.as_secs_f64() * 1_000_000.0)
    }
}
