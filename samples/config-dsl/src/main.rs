use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: adze-config-dsl <policy-file>");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("usage: adze-config-dsl <policy-file>");
        return ExitCode::from(2);
    }
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("cannot read {}: {error}", std::path::Path::new(&path).display());
            return ExitCode::from(2);
        }
    };
    match adze_config_dsl::grammar::parse(&source) {
        Ok(policy) => {
            println!("retries={}", policy.retries);
            println!("targets={}", policy.targets.join(","));
            match policy.timeout {
                Some(timeout) => println!("timeout_seconds={}", timeout.seconds),
                None => println!("timeout_seconds=unset"),
            }
            ExitCode::SUCCESS
        }
        Err(errors) => {
            if errors.is_empty() {
                eprintln!("parser returned failure without a diagnostic");
            }
            for error in errors {
                eprintln!("{}", error.display_with_source(&source));
            }
            ExitCode::from(1)
        }
    }
}
