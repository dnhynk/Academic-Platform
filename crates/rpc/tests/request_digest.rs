//! Fixed SHA-256 vectors independently assembled with Node Buffer/crypto.
use academic_rpc::{
    digest::mutable_request_digest,
    generated::{
        MutableRequest, SyntheticBackupCommand, SyntheticIngestCommand, SyntheticRestoreCommand,
        mutable_request::Command,
    },
};

#[test]
fn request_digest_preserves_independent_wire_preimages() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "synthetic-ingest",
            Command::SyntheticIngest(SyntheticIngestCommand {
                synthetic_fixture_id: "phase0-synthetic-bitemporal-ledger-v2".to_owned(),
            }),
            [
                "6c13ee8ad245660c7885f44460ddd571b18b4738d5d54daa5ec64953fceb0927",
                "32d3b6fd5669b44de605ce475db036476031bd192b6815841e431bdb7882cadb",
            ],
        ),
        (
            "synthetic-backup",
            Command::SyntheticBackup(SyntheticBackupCommand {}),
            [
                "ebefd0cafa3c2c2d34295a33e63188a9fcd83aa1af4ee41df8a5d8d07f29dcf5",
                "b7e58956b8c113917abaeddece9df634fbfefec3808f87d8095697faae04778c",
            ],
        ),
        (
            "synthetic-restore",
            Command::SyntheticRestore(SyntheticRestoreCommand {
                backup_receipt_id: vec![7; 16],
            }),
            [
                "e866c99edc2be815203ef22e81553bd0c6b2e0646c258d026f2cc94db7bc82e8",
                "70974de81736168ac1dc347ab5b7fea085a53a95fdaba551737a5488b8dc4f42",
            ],
        ),
    ];
    for (capability, command, expected) in cases {
        for (index, revision) in [None, Some(513)].into_iter().enumerate() {
            let mut request = MutableRequest {
                request_id: vec![1; 16],
                client_instance_id: vec![2; 16],
                idempotency_key: vec![3; 32],
                request_digest: vec![0; 32],
                expected_profile_revision: revision,
                capability_id: format!("learning-platform.local.{capability}.v1"),
                command: Some(command.clone()),
            };
            let digest = mutable_request_digest(&request)?;
            let actual: String = digest
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(actual, expected[index]);
            request.request_digest.fill(255);
            assert_eq!(
                mutable_request_digest(&request)?,
                digest,
                "digest excludes its own field"
            );
            request.idempotency_key[0] ^= 1;
            assert_ne!(
                mutable_request_digest(&request)?,
                digest,
                "submission binding is included"
            );
        }
    }
    Ok(())
}
