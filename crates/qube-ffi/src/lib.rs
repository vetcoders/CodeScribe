uniffi::setup_scaffolding!();

#[uniffi::export]
pub fn init_qube_engine() {
    let _ = std::any::type_name::<qube_stt::stt::adapter::WhisperSingletonAdapter>();
}
