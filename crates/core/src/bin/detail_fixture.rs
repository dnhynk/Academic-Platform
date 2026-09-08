//! Explicit local synthetic fixture tool, never enabled in the ordinary runtime.
use std::{io::Read, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let profile_path = PathBuf::from(args.next().ok_or("Usage: academic-detail-fixture <new-profile> <synthetic-corpus.json> [<lecture-id> <synthetic.wav>]")?);
    let corpus_path = PathBuf::from(args.next().ok_or("missing synthetic corpus")?);
    let audio = match args.next() {
        None => None,
        Some(lecture) => Some((
            lecture
                .into_string()
                .map_err(|_| "lecture ID is not UTF-8")?,
            PathBuf::from(
                args.next()
                    .ok_or("audio lecture requires a synthetic WAV path")?,
            ),
        )),
    };
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    println!("{}", academic_rpc::PHASE1_POLICY_BANNER);
    let mut bytes = Vec::new();
    std::fs::File::open(corpus_path)?
        .take(1_048_577)
        .read_to_end(&mut bytes)?;
    if bytes.len() > academic_rpc::details::MAX_DETAIL_BYTES {
        return Err("synthetic corpus exceeds bound".into());
    }
    let corpus: academic_rpc::details::DetailCorpus = serde_json::from_slice(&bytes)?;
    corpus.validate()?;
    let profile = academic_store::profile::create_synthetic_profile(
        &profile_path,
        &academic_store::path_policy::NativePathProbe::default(),
        *academic_domain::ContentDigest::sha256(b"academic.explicit-detail-fixture.v1").as_bytes(),
    )?;
    let mut media = std::collections::BTreeMap::new();
    if let Some((lecture, path)) = audio {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(4_194_305)
            .read_to_end(&mut bytes)?;
        media.insert(lecture, bytes);
    }
    academic_core::details::fixture::import_synthetic_corpus_with_audio(&profile, corpus, media)?;
    println!(
        "Synthetic detail corpus durably accepted into {}",
        profile_path.display()
    );
    Ok(())
}
