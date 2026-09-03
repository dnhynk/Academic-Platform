use academic_capture_gate::QuarantinedArtifact;

fn bytes_of(artifact: &QuarantinedArtifact) -> &[u8] {
    artifact.bytes()
}

fn main() {
    let _reader: fn(&QuarantinedArtifact) -> &[u8] = bytes_of;
}
