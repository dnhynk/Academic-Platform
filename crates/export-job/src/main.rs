use academic_policy::ProcessClass;

const PROCESS_CLASS: ProcessClass = ProcessClass::ExportJob;

fn main() {
    let _capability_set = PROCESS_CLASS.capabilities();
}
