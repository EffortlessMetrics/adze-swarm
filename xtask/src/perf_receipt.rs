use anyhow::Result;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    ProductSmoke,
}

struct ReceiptCommand {
    surface: &'static str,
    command: &'static str,
    evidence: &'static str,
}

const PRODUCT_SMOKE_COMMANDS: &[ReceiptCommand] = &[
    ReceiptCommand {
        surface: "fixture family inventory",
        command: "cargo test -p adze-benchmarks --test verify_fixture_parsing verify_benchmark_fixture_families_are_documented -- --exact --nocapture",
        evidence: "benchmark fixture families are registered and documented",
    },
    ReceiptCommand {
        surface: "benchmark inventory",
        command: "cargo test -p adze-benchmarks --test verify_fixture_parsing verify_benchmark_inventory_is_exhaustive -- --exact --nocapture",
        evidence: "every registered benchmark has classification metadata and README inventory",
    },
    ReceiptCommand {
        surface: "parse benchmark compile health",
        command: "cargo bench -p adze-benchmarks --bench parse_bench --no-run",
        evidence: "parse-only benchmark fixture compiles without running Criterion measurement",
    },
    ReceiptCommand {
        surface: "document projection benchmark compile health",
        command: "cargo bench -p adze-benchmarks --bench document_projection --no-run",
        evidence: "parse_document, typed AST projection, and JSON projection bench compiles",
    },
];

pub fn run(profile: Profile) -> Result<()> {
    match profile {
        Profile::ProductSmoke => print_product_smoke(),
    }

    Ok(())
}

fn print_product_smoke() {
    println!("perf-receipt profile: product-smoke");
    println!("status: advisory");
    println!("blocking: false");
    println!("runner policy: local/manual/scheduled evidence only");
    println!();
    println!("surfaces:");
    println!("- parse only: parse_bench compile receipt");
    println!("- parse_document: document_projection compile receipt");
    println!("- typed AST projection: document_projection compile receipt");
    println!("- JSON projection: document_projection compile receipt");
    println!("- benchmark inventory: fixture and classification receipts");
    println!();
    println!("proof commands:");
    for receipt in PRODUCT_SMOKE_COMMANDS {
        println!("- {}:", receipt.surface);
        println!("  command: {}", receipt.command);
        println!("  evidence: {}", receipt.evidence);
    }
    println!();
    println!("non-claims:");
    println!("- no stable throughput claim");
    println!("- no stable memory-use claim");
    println!("- no Tree-sitter performance parity claim");
    println!("- no incremental parsing performance claim");
    println!("- no release-blocking regression threshold");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_smoke_profile_names_expected_commands() {
        let commands = PRODUCT_SMOKE_COMMANDS
            .iter()
            .map(|receipt| receipt.command)
            .collect::<Vec<_>>();

        assert!(
            commands
                .iter()
                .any(|command| command.contains("parse_bench --no-run")),
            "product-smoke receipt must include parse benchmark compile health"
        );
        assert!(
            commands
                .iter()
                .any(|command| command.contains("document_projection --no-run")),
            "product-smoke receipt must include document projection compile health"
        );
        assert!(
            commands.iter().any(|command| command.contains(
                "verify_benchmark_fixture_families_are_documented"
            )),
            "product-smoke receipt must include fixture family inventory proof"
        );
    }
}
