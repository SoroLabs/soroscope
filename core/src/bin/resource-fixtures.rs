use serde::Serialize;
use soroscope_core::comparison::{build_report, RegressionReport};
use soroscope_core::resource_fixtures::{check_fixture_resources, record_fixture, ResourceFixture};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct FixtureReport {
    fixture: String,
    contract: String,
    function: String,
    report: RegressionReport,
    check_error: Option<String>,
    refreshed: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("resource fixture gate failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (fixtures_dir, report_path, refresh) = parse_args()?;
    let mut reports = Vec::new();
    let mut failed = false;

    for path in fixture_paths(&fixtures_dir)? {
        let mut fixture: ResourceFixture = serde_json::from_slice(&fs::read(&path)?)?;
        let wasm_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&fixture.wasm_path);
        let wasm = fs::read(&wasm_path)?;
        let baseline = fixture.expected.clone();

        if refresh {
            record_fixture(&mut fixture, &wasm)?;
            fs::write(&path, serde_json::to_vec_pretty(&fixture)?)?;
        }

        let actual = match soroscope_core::resource_fixtures::run_fixture_locally(&fixture, &wasm) {
            Ok(actual) => actual,
            Err(error) => {
                failed = true;
                reports.push(FixtureReport {
                    fixture: path.display().to_string(),
                    contract: fixture.contract_id.clone(),
                    function: fixture.function.clone(),
                    report: build_report(baseline, baseline.clone()),
                    check_error: Some(error.to_string()),
                    refreshed: refresh,
                });
                continue;
            }
        };

        let regression = build_report(actual.clone(), baseline);
        let check_error = check_fixture_resources(&fixture, &fixture.expected, &actual)
            .err()
            .map(|error| error.to_string());
        if check_error.is_some() || !regression.regression_flags.is_empty() {
            failed = true;
        }
        reports.push(FixtureReport {
            fixture: path.display().to_string(),
            contract: fixture.contract_id.clone(),
            function: fixture.function.clone(),
            report: regression,
            check_error,
            refreshed: refresh,
        });
    }

    if reports.is_empty() {
        return Err(format!("no fixture JSON files found in {}", fixtures_dir.display()).into());
    }

    fs::write(&report_path, serde_json::to_vec_pretty(&reports)?)?;
    for item in &reports {
        for flag in &item.report.regression_flags {
            println!(
                "{} {} {} base={} current={} percent={:+.1}%",
                item.contract,
                item.function,
                flag.resource,
                meter_value(&item.report, &flag.resource, false),
                meter_value(&item.report, &flag.resource, true),
                flag.change_percent
            );
        }
        if let Some(error) = &item.check_error {
            println!(
                "{} {} check failed: {}",
                item.contract, item.function, error
            );
        }
        if item.report.regression_flags.is_empty() && item.check_error.is_none() {
            println!("{} {} passed", item.contract, item.function);
        }
    }

    if failed && !refresh {
        return Err("one or more fixture checks failed".into());
    }
    Ok(())
}

fn parse_args() -> Result<(PathBuf, PathBuf, bool), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_default();
    if command != "check" {
        return Err(
            "usage: resource-fixtures check [--fixtures DIR] [--report FILE] [--refresh]".into(),
        );
    }
    let mut fixtures = PathBuf::from("core/fixtures");
    let mut report = PathBuf::from("resource-fixture-report.json");
    let mut refresh = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fixtures" => {
                fixtures = PathBuf::from(args.next().ok_or("missing --fixtures value")?)
            }
            "--report" => report = PathBuf::from(args.next().ok_or("missing --report value")?),
            "--refresh" => refresh = true,
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    if refresh && env::var("BASELINE_UPDATE").as_deref() != Ok("true") {
        return Err("--refresh requires BASELINE_UPDATE=true (baseline-update label)".into());
    }
    Ok((fixtures, report, refresh))
}

fn fixture_paths(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut paths = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn meter_value(report: &RegressionReport, resource: &str, current: bool) -> u64 {
    let resources = if current {
        &report.current
    } else {
        &report.base
    };
    match resource {
        "cpu_instructions" => resources.cpu_instructions,
        "ram_bytes" => resources.ram_bytes,
        "ledger_read_bytes" => resources.ledger_read_bytes,
        "ledger_write_bytes" => resources.ledger_write_bytes,
        "transaction_size_bytes" => resources.transaction_size_bytes,
        _ => 0,
    }
}
