use academic_policy::ProcessClass;

const PROCESS_CLASS: ProcessClass = ProcessClass::Connector;

fn main() {
    let _capability_set = PROCESS_CLASS.capabilities();
}
