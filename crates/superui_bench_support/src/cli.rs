//! Shared `--key value` / `--flag` parsing.
//!
//! `backend` is deliberately left as a `String`: horde has a Native backend that
//! citadel and rows do not, so mapping the string to an enum belongs to the caller.

/// Per-example parser defaults. `cap_flags` are the flag spellings that set the
/// size knob (horde: `--enemy-cap`, citadel: `--building-count`, rows: `--rows`).
#[derive(Clone, Copy, Debug)]
pub struct ArgDefaults {
    pub frames: usize,
    pub warmup: usize,
    pub cap_flags: &'static [&'static str],
}

#[derive(Clone, Debug)]
pub struct BenchArgs {
    pub backend: Option<String>,
    pub caps: Vec<usize>,
    pub frames: usize,
    pub warmup: usize,
    pub seed: u64,
    pub json: bool,
    pub dhat: bool,
    pub profile: bool,
}

/// Minimal `--key value` / `--flag` parser.
pub fn parse_args(argv: &[String], defaults: ArgDefaults) -> Result<BenchArgs, String> {
    let mut backend: Option<String> = None;
    let mut caps: Vec<usize> = Vec::new();
    let mut frames = defaults.frames;
    let mut warmup = defaults.warmup;
    let mut seed = 0u64;
    let mut json = false;
    let mut dhat = false;
    let mut profile = false;

    let mut i = 0;
    while i < argv.len() {
        let key = argv[i].as_str();
        let advance = |i: &mut usize| -> Result<&str, String> {
            *i += 1;
            argv.get(*i).map(|s| s.as_str()).ok_or_else(|| format!("missing value for {key}"))
        };
        if defaults.cap_flags.contains(&key) {
            let v = advance(&mut i)?
                .parse()
                .map_err(|_| format!("bad {key}"))?;
            caps = vec![v];
            i += 1;
            continue;
        }
        match key {
            "--backend" => backend = Some(advance(&mut i)?.to_string()),
            // Accepted and ignored, for script compatibility across the examples.
            "--preset" => {
                advance(&mut i)?;
            }
            "--sweep" => {
                caps = advance(&mut i)?
                    .split(',')
                    .map(|s| s.trim().parse::<usize>().map_err(|_| "bad --sweep list".to_string()))
                    .collect::<Result<_, _>>()?;
            }
            "--frames" | "--reps" => frames = advance(&mut i)?.parse().map_err(|_| "bad --frames/--reps".to_string())?,
            "--warmup" => warmup = advance(&mut i)?.parse().map_err(|_| "bad --warmup".to_string())?,
            "--seed" => seed = advance(&mut i)?.parse().map_err(|_| "bad --seed".to_string())?,
            "--format" => json = advance(&mut i)? == "json",
            "--dhat" => dhat = true,
            "--profile" => profile = true,
            other => return Err(format!("unknown arg '{other}'")),
        }
        i += 1;
    }

    Ok(BenchArgs { backend, caps, frames, warmup, seed, json, dhat, profile })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn defaults() -> ArgDefaults {
        ArgDefaults { frames: 1000, warmup: 100, cap_flags: &["--building-count", "--enemy-cap"] }
    }

    #[test]
    fn parses_flags_and_leaves_backend_a_string() {
        let a = parse_args(
            &args(&[
                "--backend", "supersolid", "--sweep", "60,120", "--frames", "500",
                "--warmup", "50", "--seed", "7", "--format", "json",
            ]),
            defaults(),
        )
        .unwrap();
        // Backend stays a String: the variant sets differ per example, so mapping
        // it is the caller's job.
        assert_eq!(a.backend.as_deref(), Some("supersolid"));
        assert_eq!(a.caps, vec![60, 120]);
        assert_eq!(a.frames, 500);
        assert_eq!(a.warmup, 50);
        assert_eq!(a.seed, 7);
        assert!(a.json);
    }

    #[test]
    fn cap_flag_aliases_are_accepted() {
        let a = parse_args(&args(&["--backend", "null", "--enemy-cap", "400"]), defaults()).unwrap();
        assert_eq!(a.caps, vec![400]);
        let b = parse_args(&args(&["--backend", "null", "--building-count", "120"]), defaults()).unwrap();
        assert_eq!(b.caps, vec![120]);
    }

    #[test]
    fn preset_is_tolerated_and_unknown_args_are_not() {
        assert!(parse_args(&args(&["--backend", "null", "--preset", "stress"]), defaults()).is_ok());
        assert!(parse_args(&args(&["--nope"]), defaults()).is_err());
    }

    #[test]
    fn reps_is_an_alias_for_frames() {
        let a = parse_args(&args(&["--backend", "x", "--reps", "25"]), defaults()).unwrap();
        assert_eq!(a.frames, 25);
    }

    #[test]
    fn defaults_apply_when_unset() {
        let a = parse_args(&args(&["--backend", "null"]), defaults()).unwrap();
        assert_eq!(a.frames, 1000);
        assert_eq!(a.warmup, 100);
        assert!(a.caps.is_empty(), "caps stay empty so the caller can apply its own default");
    }
}
